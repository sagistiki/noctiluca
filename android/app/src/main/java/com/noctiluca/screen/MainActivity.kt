package com.noctiluca.screen

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.net.ConnectivityManager
import android.net.LinkProperties
import android.os.Bundle
import android.view.Gravity
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.View
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.WindowManager
import android.widget.FrameLayout
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.TextView
import java.net.Inet4Address

/**
 * The whole app: a full-screen picture from Noctiluca, and while nothing is
 * coming, a calm screen that says which box this is and that it waits.
 */
class MainActivity : Activity(), Receiver.Listener {

    private lateinit var receiver: Receiver
    private lateinit var idle: View
    private lateinit var status: TextView
    private lateinit var address: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        receiver = Receiver(this, this)

        val root = FrameLayout(this).apply { setBackgroundColor(Color.BLACK) }
        val picture = SurfaceView(this)
        picture.holder.addCallback(object : SurfaceHolder.Callback {
            override fun surfaceCreated(holder: SurfaceHolder) {}
            override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
                receiver.setSurface(holder.surface, width, height)
            }
            override fun surfaceDestroyed(holder: SurfaceHolder) = receiver.setSurface(null, 0, 0)
        })
        root.addView(picture, FrameLayout.LayoutParams(-1, -1))
        idle = idleScreen()
        root.addView(idle, FrameLayout.LayoutParams(-1, -1))
        setContentView(root)
        hideSystemBars()
    }

    override fun onResume() {
        super.onResume()
        hideSystemBars()
        address.text = "${receiver.name}  ·  ${ipAddress() ?: "no network"}"
        receiver.start()
    }

    override fun onPause() {
        // The screen is let go when the app is left (Home): the Mac sees it close.
        receiver.stop()
        onState(Receiver.State.WAITING, null)
        super.onPause()
    }

    override fun onState(state: Receiver.State, mac: String?) {
        runOnUiThread {
            when (state) {
                Receiver.State.WAITING -> {
                    status.text = "Waiting for Noctiluca on the Mac"
                    idle.visibility = View.VISIBLE
                }
                Receiver.State.CONNECTED -> {
                    status.text = "Connected to Noctiluca${mac?.let { " ($it)" } ?: ""}"
                    idle.visibility = View.VISIBLE
                }
                Receiver.State.SHOWING -> idle.visibility = View.GONE
            }
        }
    }

    override fun onGameMode(on: Boolean) {
        // HDMI ALLM: the TV drops its picture processing and its delay with it.
        if (android.os.Build.VERSION.SDK_INT >= 30) runOnUiThread { window.setPreferMinimalPostProcessing(on) }
    }

    override fun hdrTypes(): List<String> {
        @Suppress("DEPRECATION")
        val caps = windowManager.defaultDisplay.hdrCapabilities ?: return emptyList()
        return caps.supportedHdrTypes.map {
            when (it) {
                android.view.Display.HdrCapabilities.HDR_TYPE_HDR10 -> "HDR10"
                android.view.Display.HdrCapabilities.HDR_TYPE_HLG -> "HLG"
                android.view.Display.HdrCapabilities.HDR_TYPE_DOLBY_VISION -> "DOLBY_VISION"
                android.view.Display.HdrCapabilities.HDR_TYPE_HDR10_PLUS -> "HDR10_PLUS"
                else -> "HDR$it"
            }
        }
    }

    private fun idleScreen(): View {
        val column = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            gravity = Gravity.CENTER
            setBackgroundColor(Color.rgb(0x0a, 0x0a, 0x10))
        }
        val logo = ImageView(this).apply { setImageResource(R.mipmap.ic_launcher) }
        column.addView(logo, LinearLayout.LayoutParams(dp(120), dp(120)).apply { bottomMargin = dp(24) })
        column.addView(TextView(this).apply {
            text = "Noctiluca Screen"
            setTextColor(Color.WHITE)
            textSize = 34f
            typeface = Typeface.create("sans-serif", Typeface.BOLD)
            gravity = Gravity.CENTER
        })
        status = TextView(this).apply {
            setTextColor(Color.rgb(0xff, 0x2b, 0xd6))
            textSize = 18f
            gravity = Gravity.CENTER
            setPadding(0, dp(14), 0, 0)
        }
        column.addView(status)
        address = TextView(this).apply {
            setTextColor(Color.rgb(0x9a, 0x9a, 0xb0))
            textSize = 15f
            gravity = Gravity.CENTER
            setPadding(0, dp(10), 0, 0)
        }
        column.addView(address)
        column.addView(TextView(this).apply {
            text = "Choose this screen in Noctiluca → Mapping → Output"
            setTextColor(Color.rgb(0x6a, 0x6a, 0x80))
            textSize = 13f
            gravity = Gravity.CENTER
            setPadding(0, dp(36), 0, 0)
        })
        status.text = "Waiting for Noctiluca on the Mac"
        return column
    }

    private fun ipAddress(): String? {
        val cm = getSystemService(ConnectivityManager::class.java) ?: return null
        val props: LinkProperties = cm.getLinkProperties(cm.activeNetwork) ?: return null
        return props.linkAddresses.map { it.address }.firstOrNull { it is Inet4Address }?.hostAddress
    }

    private fun hideSystemBars() {
        if (android.os.Build.VERSION.SDK_INT >= 30) {
            window.insetsController?.let {
                it.hide(WindowInsets.Type.systemBars())
                it.systemBarsBehavior = WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            }
        } else {
            @Suppress("DEPRECATION")
            window.decorView.systemUiVisibility = (View.SYSTEM_UI_FLAG_FULLSCREEN
                or View.SYSTEM_UI_FLAG_HIDE_NAVIGATION or View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY)
        }
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()
}
