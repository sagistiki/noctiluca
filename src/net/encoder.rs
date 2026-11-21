//! H.264 with the Mac's own video encoder (VideoToolbox, in hardware, so
//! the CPU stays free for the DJ software), set for live streaming: no
//! frame reordering, low-latency rate control, a key frame on demand.
//! Called straight through the C API; macOS only.

#![allow(non_upper_case_globals, non_snake_case)]

use std::ffi::c_void;
use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};

type CFTypeRef = *const c_void;
type CFStringRef = *const c_void;
type CFDictionaryRef = *const c_void;
type CFArrayRef = *const c_void;
type CFAllocatorRef = *const c_void;
type OSStatus = i32;
type VTCompressionSessionRef = *mut c_void;
type CVPixelBufferRef = *mut c_void;
type CVPixelBufferPoolRef = *mut c_void;
type CMSampleBufferRef = *mut c_void;
type CMBlockBufferRef = *mut c_void;
type CMFormatDescriptionRef = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

const kCMTimeFlags_Valid: u32 = 1;
const INVALID_TIME: CMTime = CMTime { value: 0, timescale: 0, flags: 0, epoch: 0 };
const kCMVideoCodecType_H264: u32 = u32::from_be_bytes(*b"avc1");
const kCVPixelFormatType_32BGRA: u32 = u32::from_be_bytes(*b"BGRA");
const kCFNumberSInt32Type: isize = 3;
const kCVPixelBufferLock_ReadOnly: u64 = 1;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: CFTypeRef;
    static kCFBooleanFalse: CFTypeRef;
    static kCFTypeDictionaryKeyCallBacks: [usize; 6];
    static kCFTypeDictionaryValueCallBacks: [usize; 5];
    fn CFNumberCreate(alloc: CFAllocatorRef, kind: isize, value: *const c_void) -> CFTypeRef;
    fn CFDictionaryCreate(alloc: CFAllocatorRef, keys: *const CFTypeRef, values: *const CFTypeRef, count: isize, kc: *const c_void, vc: *const c_void) -> CFDictionaryRef;
    fn CFDictionaryContainsKey(dict: CFDictionaryRef, key: CFTypeRef) -> u8;
    fn CFDictionaryGetValue(dict: CFDictionaryRef, key: CFTypeRef) -> CFTypeRef;
    fn CFArrayGetCount(array: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, index: isize) -> CFTypeRef;
    fn CFRelease(cf: CFTypeRef);
}

#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    static kCVPixelBufferPixelFormatTypeKey: CFStringRef;
    static kCVPixelBufferWidthKey: CFStringRef;
    static kCVPixelBufferHeightKey: CFStringRef;
    static kCVPixelBufferIOSurfacePropertiesKey: CFStringRef;
    fn CVPixelBufferPoolCreate(alloc: CFAllocatorRef, pool_attrs: CFDictionaryRef, buffer_attrs: CFDictionaryRef, out: *mut CVPixelBufferPoolRef) -> i32;
    fn CVPixelBufferPoolCreatePixelBuffer(alloc: CFAllocatorRef, pool: CVPixelBufferPoolRef, out: *mut CVPixelBufferRef) -> i32;
    fn CVPixelBufferLockBaseAddress(pb: CVPixelBufferRef, flags: u64) -> i32;
    fn CVPixelBufferUnlockBaseAddress(pb: CVPixelBufferRef, flags: u64) -> i32;
    fn CVPixelBufferGetBaseAddress(pb: CVPixelBufferRef) -> *mut u8;
    fn CVPixelBufferGetBytesPerRow(pb: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetDataSize(pb: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetPixelFormatType(pb: CVPixelBufferRef) -> u32;
    fn CVPixelBufferGetWidth(pb: CVPixelBufferRef) -> usize;
    fn CVPixelBufferGetHeight(pb: CVPixelBufferRef) -> usize;
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    static kCMSampleAttachmentKey_NotSync: CFStringRef;
    fn CMSampleBufferGetDataBuffer(sb: CMSampleBufferRef) -> CMBlockBufferRef;
    fn CMSampleBufferGetFormatDescription(sb: CMSampleBufferRef) -> CMFormatDescriptionRef;
    fn CMSampleBufferGetSampleAttachmentsArray(sb: CMSampleBufferRef, create: u8) -> CFArrayRef;
    fn CMSampleBufferGetPresentationTimeStamp(sb: CMSampleBufferRef) -> CMTime;
    fn CMBlockBufferGetDataLength(bb: CMBlockBufferRef) -> usize;
    fn CMBlockBufferCopyDataBytes(bb: CMBlockBufferRef, offset: usize, length: usize, dest: *mut c_void) -> OSStatus;
    fn CMVideoFormatDescriptionGetH264ParameterSetAtIndex(desc: CMFormatDescriptionRef, index: usize, ptr: *mut *const u8, size: *mut usize, count: *mut usize, nal_header_len: *mut i32) -> OSStatus;
}

type OutputCallback = extern "C" fn(*mut c_void, *mut c_void, OSStatus, u32, CMSampleBufferRef);

#[link(name = "VideoToolbox", kind = "framework")]
unsafe extern "C" {
    static kVTCompressionPropertyKey_RealTime: CFStringRef;
    static kVTCompressionPropertyKey_AllowFrameReordering: CFStringRef;
    static kVTCompressionPropertyKey_ProfileLevel: CFStringRef;
    static kVTCompressionPropertyKey_AverageBitRate: CFStringRef;
    static kVTCompressionPropertyKey_MaxKeyFrameInterval: CFStringRef;
    static kVTCompressionPropertyKey_ExpectedFrameRate: CFStringRef;
    static kVTCompressionPropertyKey_ColorPrimaries: CFStringRef;
    static kVTCompressionPropertyKey_TransferFunction: CFStringRef;
    static kVTCompressionPropertyKey_YCbCrMatrix: CFStringRef;
    static kVTProfileLevel_H264_High_AutoLevel: CFStringRef;
    static kVTEncodeFrameOptionKey_ForceKeyFrame: CFStringRef;
    static kVTVideoEncoderSpecification_EnableLowLatencyRateControl: CFStringRef;
    static kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder: CFStringRef;
    fn VTCompressionSessionCreate(
        alloc: CFAllocatorRef,
        width: i32,
        height: i32,
        codec: u32,
        encoder_spec: CFDictionaryRef,
        source_attrs: CFDictionaryRef,
        data_alloc: CFAllocatorRef,
        callback: OutputCallback,
        refcon: *mut c_void,
        out: *mut VTCompressionSessionRef,
    ) -> OSStatus;
    fn VTSessionSetProperty(session: VTCompressionSessionRef, key: CFStringRef, value: CFTypeRef) -> OSStatus;
    fn VTCompressionSessionPrepareToEncodeFrames(session: VTCompressionSessionRef) -> OSStatus;
    fn VTCompressionSessionEncodeFrame(
        session: VTCompressionSessionRef,
        image: CVPixelBufferRef,
        pts: CMTime,
        duration: CMTime,
        props: CFDictionaryRef,
        refcon: *mut c_void,
        flags_out: *mut u32,
    ) -> OSStatus;
    fn VTCompressionSessionCompleteFrames(session: VTCompressionSessionRef, until: CMTime) -> OSStatus;
    fn VTCompressionSessionInvalidate(session: VTCompressionSessionRef);
}

#[link(name = "CoreMedia", kind = "framework")]
unsafe extern "C" {
    static kCMFormatDescriptionColorPrimaries_ITU_R_709_2: CFStringRef;
    static kCMFormatDescriptionTransferFunction_ITU_R_709_2: CFStringRef;
    static kCMFormatDescriptionYCbCrMatrix_ITU_R_709_2: CFStringRef;
}

/// One encoded picture: Annex B (start codes), parameter sets in front of
/// key frames, and the time it was given with.
#[derive(Clone, Debug)]
pub struct Encoded {
    pub time_us: i64,
    pub key: bool,
    pub data: Vec<u8>,
}

type Sink = Arc<Mutex<Vec<Encoded>>>;

pub struct Encoder {
    session: VTCompressionSessionRef,
    pool: CVPixelBufferPoolRef,
    sink: *mut Sink,
    out: Sink,
    pub width: u32,
    pub height: u32,
}

// The session is used from one thread at a time (the stream's).
unsafe impl Send for Encoder {}

fn number(v: i32) -> CFTypeRef {
    unsafe { CFNumberCreate(std::ptr::null(), kCFNumberSInt32Type, (&v as *const i32).cast()) }
}

fn dictionary(pairs: &[(CFStringRef, CFTypeRef)]) -> CFDictionaryRef {
    let keys: Vec<CFTypeRef> = pairs.iter().map(|p| p.0).collect();
    let values: Vec<CFTypeRef> = pairs.iter().map(|p| p.1).collect();
    unsafe {
        CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            pairs.len() as isize,
            (&raw const kCFTypeDictionaryKeyCallBacks).cast(),
            (&raw const kCFTypeDictionaryValueCallBacks).cast(),
        )
    }
}

extern "C" fn on_encoded(refcon: *mut c_void, _frame: *mut c_void, status: OSStatus, _flags: u32, sb: CMSampleBufferRef) {
    if status != 0 || sb.is_null() || refcon.is_null() {
        return;
    }
    let sink = unsafe { &*(refcon as *const Sink) };
    unsafe {
        let attachments = CMSampleBufferGetSampleAttachmentsArray(sb, 0);
        let key = attachments.is_null()
            || CFArrayGetCount(attachments) == 0
            || CFDictionaryContainsKey(CFArrayGetValueAtIndex(attachments, 0), kCMSampleAttachmentKey_NotSync) == 0
            || CFDictionaryGetValue(CFArrayGetValueAtIndex(attachments, 0), kCMSampleAttachmentKey_NotSync) == kCFBooleanFalse;
        let mut data = Vec::new();
        if key {
            // SPS and PPS first, so the screen can start here.
            let desc = CMSampleBufferGetFormatDescription(sb);
            let mut count = 0usize;
            let mut index = 0usize;
            loop {
                let (mut ptr, mut size, mut header) = (std::ptr::null(), 0usize, 0i32);
                if CMVideoFormatDescriptionGetH264ParameterSetAtIndex(desc, index, &mut ptr, &mut size, &mut count, &mut header) != 0 {
                    break;
                }
                data.extend_from_slice(&[0, 0, 0, 1]);
                data.extend_from_slice(std::slice::from_raw_parts(ptr, size));
                index += 1;
                if index >= count {
                    break;
                }
            }
        }
        let block = CMSampleBufferGetDataBuffer(sb);
        let len = CMBlockBufferGetDataLength(block);
        let mut avcc = vec![0u8; len];
        if CMBlockBufferCopyDataBytes(block, 0, len, avcc.as_mut_ptr().cast()) != 0 {
            return;
        }
        data.extend_from_slice(&avcc_to_annex_b(&avcc));
        let pts = CMSampleBufferGetPresentationTimeStamp(sb);
        if let Ok(mut out) = sink.lock() {
            out.push(Encoded { time_us: pts.value, key, data });
        }
    }
}

/// Length-prefixed NAL units (as the encoder gives them) to start codes (as
/// decoders take them from a stream).
pub fn avcc_to_annex_b(avcc: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(avcc.len() + 16);
    let mut i = 0;
    while i + 4 <= avcc.len() {
        let n = u32::from_be_bytes(avcc[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        if n == 0 || i + n > avcc.len() {
            break;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&avcc[i..i + n]);
        i += n;
    }
    out
}

impl Encoder {
    /// A hardware H.264 encoder for `width` x `height` BGRA pictures.
    pub fn new(width: u32, height: u32, fps: u32, bitrate: u32) -> Result<Self> {
        let out: Sink = Arc::new(Mutex::new(Vec::new()));
        let sink = Box::into_raw(Box::new(out.clone()));
        let mut session: VTCompressionSessionRef = std::ptr::null_mut();
        unsafe {
            let spec = dictionary(&[
                (kVTVideoEncoderSpecification_RequireHardwareAcceleratedVideoEncoder, kCFBooleanTrue),
                (kVTVideoEncoderSpecification_EnableLowLatencyRateControl, kCFBooleanTrue),
            ]);
            let (fmt, w, h) = (number(kCVPixelFormatType_32BGRA as i32), number(width as i32), number(height as i32));
            let io = dictionary(&[]);
            let attrs = dictionary(&[
                (kCVPixelBufferPixelFormatTypeKey, fmt),
                (kCVPixelBufferWidthKey, w),
                (kCVPixelBufferHeightKey, h),
                (kCVPixelBufferIOSurfacePropertiesKey, io),
            ]);
            let status = VTCompressionSessionCreate(
                std::ptr::null(),
                width as i32,
                height as i32,
                kCMVideoCodecType_H264,
                spec,
                attrs,
                std::ptr::null(),
                on_encoded,
                sink.cast(),
                &mut session,
            );
            // Our own pool of BGRA buffers: the session's own pool changes to
            // the encoder's native format once it is running.
            let mut pool: CVPixelBufferPoolRef = std::ptr::null_mut();
            let pool_status = CVPixelBufferPoolCreate(std::ptr::null(), std::ptr::null(), attrs, &mut pool);
            for cf in [spec, fmt, w, h, io, attrs] {
                CFRelease(cf);
            }
            if status != 0 || session.is_null() || pool_status != 0 || pool.is_null() {
                if !session.is_null() {
                    VTCompressionSessionInvalidate(session);
                    CFRelease(session);
                }
                if !pool.is_null() {
                    CFRelease(pool);
                }
                drop(Box::from_raw(sink));
                bail!("the video encoder would not start (status {status}, {pool_status})");
            }
            let set = |key: CFStringRef, value: CFTypeRef| VTSessionSetProperty(session, key, value);
            set(kVTCompressionPropertyKey_RealTime, kCFBooleanTrue);
            set(kVTCompressionPropertyKey_AllowFrameReordering, kCFBooleanFalse);
            set(kVTCompressionPropertyKey_ProfileLevel, kVTProfileLevel_H264_High_AutoLevel);
            for (key, v) in [
                (kVTCompressionPropertyKey_AverageBitRate, bitrate as i32),
                (kVTCompressionPropertyKey_MaxKeyFrameInterval, (fps * 2) as i32),
                (kVTCompressionPropertyKey_ExpectedFrameRate, fps as i32),
            ] {
                let n = number(v);
                set(key, n);
                CFRelease(n);
            }
            // SDR, BT.709: what the picture is, said in the stream, so the
            // TV shows its colours and blacks as meant, in any picture mode.
            set(kVTCompressionPropertyKey_ColorPrimaries, kCMFormatDescriptionColorPrimaries_ITU_R_709_2);
            set(kVTCompressionPropertyKey_TransferFunction, kCMFormatDescriptionTransferFunction_ITU_R_709_2);
            set(kVTCompressionPropertyKey_YCbCrMatrix, kCMFormatDescriptionYCbCrMatrix_ITU_R_709_2);
            VTCompressionSessionPrepareToEncodeFrames(session);
            Ok(Encoder { session, pool, sink, out, width, height })
        }
    }

    /// Encodes one BGRA picture (`stride` bytes a row, top row first),
    /// tagged with `time_us`. The result arrives in `take`.
    pub fn encode(&mut self, bgra: &[u8], stride: usize, time_us: i64, force_key: bool) -> Result<()> {
        let (w, h) = (self.width as usize, self.height as usize);
        if bgra.len() < stride * (h - 1) + w * 4 {
            bail!("picture too small");
        }
        unsafe {
            let mut pb: CVPixelBufferRef = std::ptr::null_mut();
            if CVPixelBufferPoolCreatePixelBuffer(std::ptr::null(), self.pool, &mut pb) != 0 || pb.is_null() {
                bail!("no picture buffer free");
            }
            if CVPixelBufferLockBaseAddress(pb, 0) != 0 {
                CFRelease(pb);
                bail!("picture buffer not writable");
            }
            let dst = CVPixelBufferGetBaseAddress(pb);
            let dst_stride = CVPixelBufferGetBytesPerRow(pb);
            if std::env::var_os("TV_ENC_DEBUG").is_some() {
                eprintln!(
                    "pb fmt {:08x} {}x{} stride {} size {} base {:p}",
                    CVPixelBufferGetPixelFormatType(pb),
                    CVPixelBufferGetWidth(pb),
                    CVPixelBufferGetHeight(pb),
                    dst_stride,
                    CVPixelBufferGetDataSize(pb),
                    dst
                );
            }
            // Only a BGRA buffer of the right size is written into.
            if dst.is_null()
                || CVPixelBufferGetPixelFormatType(pb) != kCVPixelFormatType_32BGRA
                || CVPixelBufferGetWidth(pb) != w
                || CVPixelBufferGetHeight(pb) != h
                || dst_stride < w * 4
            {
                CVPixelBufferUnlockBaseAddress(pb, 0);
                CFRelease(pb);
                bail!("the encoder's picture buffer is not BGRA {w} x {h}");
            }
            for y in 0..h {
                std::ptr::copy_nonoverlapping(bgra.as_ptr().add(y * stride), dst.add(y * dst_stride), w * 4);
            }
            CVPixelBufferUnlockBaseAddress(pb, 0);
            let _ = kCVPixelBufferLock_ReadOnly;
            let props = force_key.then(|| dictionary(&[(kVTEncodeFrameOptionKey_ForceKeyFrame, kCFBooleanTrue)]));
            let pts = CMTime { value: time_us, timescale: 1_000_000, flags: kCMTimeFlags_Valid, epoch: 0 };
            let status = VTCompressionSessionEncodeFrame(
                self.session,
                pb,
                pts,
                INVALID_TIME,
                props.unwrap_or(std::ptr::null()),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            if let Some(p) = props {
                CFRelease(p);
            }
            CFRelease(pb);
            if status != 0 {
                bail!("encoding failed (status {status})");
            }
        }
        Ok(())
    }

    /// Waits until every picture given is encoded.
    pub fn flush(&mut self) {
        unsafe {
            VTCompressionSessionCompleteFrames(self.session, INVALID_TIME);
        }
    }

    /// The pictures encoded so far, oldest first.
    pub fn take(&mut self) -> Vec<Encoded> {
        self.out.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default()
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            VTCompressionSessionCompleteFrames(self.session, INVALID_TIME);
            VTCompressionSessionInvalidate(self.session);
            CFRelease(self.session);
            CFRelease(self.pool);
            drop(Box::from_raw(self.sink));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_prefixed_units_become_start_codes() {
        let avcc = [0, 0, 0, 2, 0x65, 0xaa, 0, 0, 0, 1, 0x41];
        assert_eq!(avcc_to_annex_b(&avcc), vec![0, 0, 0, 1, 0x65, 0xaa, 0, 0, 0, 1, 0x41]);
        // A broken length stops, nothing past the end is read.
        assert_eq!(avcc_to_annex_b(&[0, 0, 0, 9, 1]), Vec::<u8>::new());
    }

    /// The real encoder: a few frames of a moving picture come out as H.264
    /// that starts with a key frame carrying its SPS and PPS.
    #[test]
    fn the_mac_encodes_h264_with_parameter_sets() {
        let (w, h) = (640u32, 360u32);
        let Ok(mut enc) = Encoder::new(w, h, 60, 4_000_000) else {
            eprintln!("no hardware encoder here; skipped");
            return;
        };
        let mut frames = Vec::new();
        for n in 0..10i64 {
            let mut px = vec![0u8; (w * h * 4) as usize];
            for (i, p) in px.chunks_mut(4).enumerate() {
                let x = (i as u32 % w) as i64;
                p.copy_from_slice(&[((x + n * 8) % 256) as u8, 40, 200, 255]);
            }
            enc.encode(&px, (w * 4) as usize, n * 16_667, n == 5).unwrap();
            enc.flush();
            frames.extend(enc.take());
        }
        assert_eq!(frames.len(), 10);
        assert!(frames[0].key);
        let nal_types = |d: &[u8]| -> Vec<u8> { d.windows(5).filter(|w| w[..4] == [0, 0, 0, 1]).map(|w| w[4] & 0x1f).collect() };
        let first = nal_types(&frames[0].data);
        assert!(first.contains(&7) && first.contains(&8) && first.contains(&5), "{first:?}");
        assert!(frames[5].key, "a key frame on demand");
        assert!(!frames[3].key);
        assert_eq!(frames[3].time_us, 3 * 16_667);
    }
}
