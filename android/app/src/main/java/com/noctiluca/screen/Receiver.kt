package com.noctiluca.screen

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.view.Surface
import org.json.JSONObject
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.DataInputStream
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketException

/**
 * Waits for Noctiluca on the Mac, announces this screen on the network, and
 * plays what the Mac sends. One Mac at a time: a new connection takes over.
 */
class Receiver(private val context: Context, private val listener: Listener) {

    interface Listener {
        /** Waiting, connected (with the Mac's address) or showing. */
        fun onState(state: State, mac: String?)
        /** The Mac asks for the TV's game mode (least delay) on or off. */
        fun onGameMode(on: Boolean)
        /** The HDR kinds the TV takes, as named in the protocol. */
        fun hdrTypes(): List<String>
    }

    enum class State { WAITING, CONNECTED, SHOWING }

    val name: String = Settings.Global.getString(context.contentResolver, "device_name")
        ?.takeIf { it.isNotBlank() } ?: Build.MODEL

    @Volatile private var surface: Surface? = null
    @Volatile private var screen = Pair(1920, 1080)
    @Volatile private var running = false
    private var server: ServerSocket? = null
    private var client: Socket? = null
    private var decoder: Decoder? = null
    private var nsd: NsdManager? = null
    private var registration: NsdManager.RegistrationListener? = null

    fun setSurface(surface: Surface?, width: Int, height: Int) {
        synchronized(this) {
            this.surface = surface
            screen = Pair(width, height)
            if (surface == null) stopDecoder()
        }
    }

    fun start() {
        if (running) return
        running = true
        Thread({ serve() }, "receiver").start()
        announce()
    }

    fun stop() {
        running = false
        nsd?.let { n -> registration?.let { runCatching { n.unregisterService(it) } } }
        registration = null
        runCatching { client?.close() }
        runCatching { server?.close() }
        synchronized(this) { stopDecoder() }
    }

    private fun announce() {
        val info = NsdServiceInfo().apply {
            serviceName = name
            serviceType = Protocol.SERVICE_TYPE
            port = Protocol.PORT
            setAttribute("model", Build.MODEL)
            setAttribute("w", screen.first.toString())
            setAttribute("h", screen.second.toString())
            setAttribute("v", Protocol.VERSION.toString())
            setAttribute("app", BuildConfigVersion.NAME)
        }
        val listener = object : NsdManager.RegistrationListener {
            override fun onServiceRegistered(info: NsdServiceInfo) {
                Log.i(TAG, "announced as ${info.serviceName}")
            }
            override fun onRegistrationFailed(info: NsdServiceInfo, code: Int) {
                Log.w(TAG, "announce failed $code")
            }
            override fun onServiceUnregistered(info: NsdServiceInfo) {}
            override fun onUnregistrationFailed(info: NsdServiceInfo, code: Int) {}
        }
        nsd = context.getSystemService(Context.NSD_SERVICE) as NsdManager
        registration = listener
        nsd?.registerService(info, NsdManager.PROTOCOL_DNS_SD, listener)
    }

    private fun serve() {
        try {
            val s = ServerSocket()
            s.reuseAddress = true
            s.bind(InetSocketAddress(Protocol.PORT))
            server = s
            while (running) {
                val socket = s.accept()
                // A new Mac takes over from the old one.
                runCatching { client?.close() }
                client = socket
                Thread({ session(socket) }, "session").start()
            }
        } catch (e: SocketException) {
            // Closed on stop.
        } catch (e: Exception) {
            Log.e(TAG, "server", e)
        }
    }

    private fun session(socket: Socket) {
        val mac = socket.inetAddress.hostAddress
        socket.tcpNoDelay = true
        socket.receiveBufferSize = 1 shl 20
        socket.soTimeout = 0
        listener.onState(State.CONNECTED, mac)
        Log.i(TAG, "Mac connected from $mac")
        val input = DataInputStream(BufferedInputStream(socket.getInputStream(), 1 shl 16))
        val out = BufferedOutputStream(socket.getOutputStream())
        var stats: Thread? = null
        try {
            Protocol.write(out, Protocol.HELLO, hello().toByteArray())
            stats = Thread({ sendStats(socket, out) }, "stats").apply { start() }
            while (running && !socket.isClosed) {
                val m = Protocol.read(input)
                when (m.type) {
                    Protocol.PING -> {
                        val macNs = Protocol.le(m.payload).long
                        Protocol.write(out, Protocol.PONG, Protocol.longs(macNs, System.nanoTime()))
                    }
                    Protocol.CONFIG -> configure(JSONObject(String(m.payload)))
                    Protocol.FRAME -> {
                        val b = Protocol.le(m.payload)
                        val showAt = b.long
                        val flags = b.get().toInt()
                        val data = ByteArray(b.remaining())
                        b.get(data)
                        synchronized(this) { decoder }?.offer(Decoder.Frame(showAt, flags and Protocol.FLAG_KEY != 0, data))
                    }
                    Protocol.IDLE -> {
                        synchronized(this) { stopDecoder() }
                        listener.onState(State.CONNECTED, mac)
                    }
                }
            }
        } catch (e: Exception) {
            Log.i(TAG, "Mac gone: ${e.message}")
        } finally {
            stats?.interrupt()
            runCatching { socket.close() }
            if (client === socket) {
                synchronized(this) { stopDecoder() }
                listener.onState(State.WAITING, null)
            }
        }
    }

    private fun configure(config: JSONObject) {
        val w = config.optInt("width", screen.first)
        val h = config.optInt("height", screen.second)
        listener.onGameMode(config.optBoolean("game_mode", true))
        synchronized(this) {
            stopDecoder()
            val s = surface ?: return
            decoder = Decoder(s, w, h).also { d ->
                d.onFirstFrame = { listener.onState(State.SHOWING, client?.inetAddress?.hostAddress) }
            }
        }
    }

    private fun stopDecoder() {
        decoder?.release()
        decoder = null
    }

    private fun hello(): String = JSONObject().apply {
        put("name", name)
        put("model", Build.MODEL)
        put("maker", Build.MANUFACTURER)
        put("android", Build.VERSION.RELEASE)
        put("width", screen.first)
        put("height", screen.second)
        put("version", Protocol.VERSION)
        put("app", BuildConfigVersion.NAME)
        put("hdr", org.json.JSONArray(listener.hdrTypes()))
    }.toString()

    private fun sendStats(socket: Socket, out: BufferedOutputStream) {
        try {
            while (!socket.isClosed) {
                Thread.sleep(1000)
                val d = synchronized(this) { decoder }
                val json = JSONObject().apply {
                    put("decoded", d?.decoded?.get() ?: 0)
                    put("shown", d?.shown?.get() ?: 0)
                    put("late", d?.late?.get() ?: 0)
                    put("dropped", d?.dropped?.get() ?: 0)
                    put("decode_ms", d?.decodeMs ?: 0.0)
                }
                Protocol.write(out, Protocol.STATS, json.toString().toByteArray())
            }
        } catch (_: Exception) {
        }
    }

    companion object {
        private const val TAG = "Noctiluca"
    }
}

/** The app's version, without generating BuildConfig. */
object BuildConfigVersion {
    const val NAME = "1.2"
}
