package com.noctiluca.screen

import java.io.DataInputStream
import java.io.IOException
import java.io.OutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * The messages between Noctiluca on the Mac and this screen. Must match
 * src/net/protocol.rs. Each message: length (u32, of everything after it),
 * type (u8), payload; numbers little endian.
 */
object Protocol {
    const val PORT = 47800
    const val SERVICE_TYPE = "_noctiluca._tcp"
    const val VERSION = 1

    const val HELLO: Int = 1   // screen -> Mac: JSON about this screen
    const val PING: Int = 2    // Mac -> screen: i64 Mac time (ns)
    const val PONG: Int = 3    // screen -> Mac: i64 Mac time, i64 screen time (ns)
    const val CONFIG: Int = 4  // Mac -> screen: JSON {width, height, fps}
    const val FRAME: Int = 5   // Mac -> screen: i64 show at (screen ns), u8 flags, H.264 Annex B
    const val STATS: Int = 6   // screen -> Mac: JSON, once a second
    const val IDLE: Int = 7    // Mac -> screen: stop showing, back to the waiting screen

    const val FLAG_KEY = 1

    /** Frames bigger than this are refused as a broken stream. */
    const val MAX_MESSAGE = 16 * 1024 * 1024

    class Message(val type: Int, val payload: ByteArray)

    fun read(input: DataInputStream): Message {
        val header = ByteArray(4)
        input.readFully(header)
        val length = ByteBuffer.wrap(header).order(ByteOrder.LITTLE_ENDIAN).int
        if (length < 1 || length > MAX_MESSAGE) throw IOException("bad message length $length")
        val type = input.readUnsignedByte()
        val payload = ByteArray(length - 1)
        input.readFully(payload)
        return Message(type, payload)
    }

    fun write(out: OutputStream, type: Int, payload: ByteArray) {
        val header = ByteBuffer.allocate(5).order(ByteOrder.LITTLE_ENDIAN)
        header.putInt(payload.size + 1)
        header.put(type.toByte())
        synchronized(out) {
            out.write(header.array())
            out.write(payload)
            out.flush()
        }
    }

    fun longs(vararg values: Long): ByteArray {
        val b = ByteBuffer.allocate(8 * values.size).order(ByteOrder.LITTLE_ENDIAN)
        values.forEach { b.putLong(it) }
        return b.array()
    }

    fun le(payload: ByteArray): ByteBuffer = ByteBuffer.wrap(payload).order(ByteOrder.LITTLE_ENDIAN)
}
