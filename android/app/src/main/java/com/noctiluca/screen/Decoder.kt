package com.noctiluca.screen

import android.media.MediaCodec
import android.media.MediaFormat
import android.os.Build
import android.util.Log
import android.view.Surface
import java.nio.ByteBuffer
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong

/**
 * Decodes the Mac's H.264 with the box's hardware decoder straight onto the
 * screen, and shows each frame at the moment the Mac asked for (in this
 * box's System.nanoTime clock). Frames that come too late are dropped, so
 * the picture never falls behind the music.
 */
class Decoder(private val surface: Surface, private val width: Int, private val height: Int) {

    class Frame(val showAtNs: Long, val key: Boolean, val data: ByteArray)

    val decoded = AtomicLong()
    val shown = AtomicLong()
    val late = AtomicLong()
    val dropped = AtomicLong()
    /** How long frames waited in the decoder, averaged (ms). */
    @Volatile var decodeMs = 0.0
    /** Called on the first frame shown. */
    @Volatile var onFirstFrame: (() -> Unit)? = null

    private val queue = ArrayBlockingQueue<Frame>(12)
    private var codec: MediaCodec? = null
    @Volatile private var running = true
    private var inputThread: Thread? = null
    private var outputThread: Thread? = null
    @Volatile private var waitingForKey = true
    private val queuedAt = HashMap<Long, Long>()

    /** Hands a frame over; never blocks the network. */
    fun offer(frame: Frame) {
        if (!queue.offer(frame)) {
            // The decoder is behind: start again from the next key frame.
            queue.clear()
            dropped.incrementAndGet()
            waitingForKey = true
        }
    }

    fun release() {
        running = false
        inputThread?.interrupt()
        inputThread?.join(500)
        outputThread?.join(500)
        try {
            codec?.stop()
        } catch (_: Exception) {
        }
        codec?.release()
        codec = null
    }

    init {
        inputThread = Thread({ feed() }, "decoder-in").apply { start() }
    }

    private fun start(key: Frame): MediaCodec {
        val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, width, height)
        // The parameter sets travel in the key frame: give them to the decoder up front.
        val (sps, pps) = parameterSets(key.data)
        if (sps != null) format.setByteBuffer("csd-0", ByteBuffer.wrap(sps))
        if (pps != null) format.setByteBuffer("csd-1", ByteBuffer.wrap(pps))
        if (Build.VERSION.SDK_INT >= 30) format.setInteger(MediaFormat.KEY_LOW_LATENCY, 1)
        format.setInteger("vendor.low-latency.enable", 1)
        format.setInteger(MediaFormat.KEY_PRIORITY, 0) // real time
        val c = MediaCodec.createDecoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
        c.configure(format, surface, null, 0)
        c.start()
        outputThread = Thread({ drain(c) }, "decoder-out").apply { start() }
        Log.i(TAG, "decoder ${c.name} ${width}x$height")
        return c
    }

    private fun feed() {
        try {
            while (running) {
                val frame = queue.poll(100, TimeUnit.MILLISECONDS) ?: continue
                if (waitingForKey) {
                    if (!frame.key) {
                        dropped.incrementAndGet()
                        continue
                    }
                    waitingForKey = false
                }
                val c = codec ?: start(frame).also { codec = it }
                var index = -1
                while (running && index < 0) index = c.dequeueInputBuffer(10_000)
                if (index < 0) break
                val buffer = c.getInputBuffer(index) ?: continue
                buffer.clear()
                if (frame.data.size > buffer.capacity()) {
                    c.queueInputBuffer(index, 0, 0, 0, 0)
                    dropped.incrementAndGet()
                    waitingForKey = true
                    continue
                }
                buffer.put(frame.data)
                val pts = frame.showAtNs / 1000
                synchronized(queuedAt) { queuedAt[pts] = System.nanoTime() }
                c.queueInputBuffer(index, 0, frame.data.size, pts, if (frame.key) MediaCodec.BUFFER_FLAG_KEY_FRAME else 0)
            }
        } catch (e: InterruptedException) {
            // Stopping.
        } catch (e: Exception) {
            Log.e(TAG, "decoder input", e)
        }
    }

    private fun drain(c: MediaCodec) {
        val info = MediaCodec.BufferInfo()
        try {
            while (running) {
                val index = c.dequeueOutputBuffer(info, 10_000)
                if (index < 0) continue
                decoded.incrementAndGet()
                val showAt = info.presentationTimeUs * 1000
                val now = System.nanoTime()
                synchronized(queuedAt) {
                    queuedAt.remove(info.presentationTimeUs)?.let { decodeMs = decodeMs * 0.95 + (now - it) / 1e6 * 0.05 }
                    if (queuedAt.size > 64) queuedAt.clear()
                }
                if (showAt < now - LATE_NS) {
                    // Too late to be on the beat: not shown.
                    c.releaseOutputBuffer(index, false)
                    late.incrementAndGet()
                } else {
                    // Shown at that moment (the nearest refresh), at most a second ahead.
                    c.releaseOutputBuffer(index, minOf(showAt, now + 1_000_000_000L))
                    if (shown.incrementAndGet() == 1L) onFirstFrame?.invoke()
                }
            }
        } catch (e: IllegalStateException) {
            // Released while waiting.
        } catch (e: Exception) {
            Log.e(TAG, "decoder output", e)
        }
    }

    companion object {
        private const val TAG = "Noctiluca"
        /** A frame this late (or more) is dropped. */
        private const val LATE_NS = 12_000_000L

        /** The SPS and PPS of an Annex B key frame, each with its start code. */
        fun parameterSets(data: ByteArray): Pair<ByteArray?, ByteArray?> {
            var sps: ByteArray? = null
            var pps: ByteArray? = null
            val starts = ArrayList<Int>()
            var i = 0
            while (i + 3 < data.size) {
                if (data[i].toInt() == 0 && data[i + 1].toInt() == 0 && data[i + 2].toInt() == 0 && data[i + 3].toInt() == 1) {
                    starts.add(i)
                    i += 4
                } else {
                    i++
                }
            }
            for ((n, s) in starts.withIndex()) {
                val end = if (n + 1 < starts.size) starts[n + 1] else data.size
                when (data[s + 4].toInt() and 0x1f) {
                    7 -> sps = data.copyOfRange(s, end)
                    8 -> pps = data.copyOfRange(s, end)
                }
            }
            return Pair(sps, pps)
        }
    }
}
