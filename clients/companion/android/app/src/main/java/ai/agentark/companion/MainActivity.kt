package ai.agentark.companion

import android.Manifest
import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView

class MainActivity : Activity() {
    private val notificationChannelId = "agentark_companion_notifications"
    private lateinit var store: SecureTokenStore
    private var client: CompanionClient? = null
    private lateinit var status: TextView
    private lateinit var wsUrl: EditText
    private lateinit var sessionId: EditText
    private lateinit var code: EditText

    private val capabilities = setOf(
        "approval_prompt",
        "notifications",
        "sms",
        "whatsapp_handoff",
        "camera",
        "photos",
        "location"
    )

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = SecureTokenStore(this)

        val layout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(32, 32, 32, 32)
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        wsUrl = EditText(this).apply {
            hint = "ws://localhost:8990/companion/ws"
            setText("ws://10.0.2.2:8990/companion/ws")
        }
        sessionId = EditText(this).apply { hint = "pairing session id" }
        code = EditText(this).apply { hint = "pairing code" }
        status = TextView(this).apply { text = "Not connected" }

        val connect = Button(this).apply {
            text = "Connect"
            setOnClickListener { connectClient() }
        }
        val claim = Button(this).apply {
            text = "Claim pairing"
            setOnClickListener {
                ensureNotificationPermission()
                client?.claimPairing(sessionId.text.toString(), code.text.toString())
            }
        }
        val pulse = Button(this).apply {
            text = "Pulse"
            setOnClickListener { client?.pulse() }
        }
        val clear = Button(this).apply {
            text = "Clear token"
            setOnClickListener {
                store.clear()
                status.text = "Stored token cleared"
            }
        }

        listOf(wsUrl, sessionId, code, connect, claim, pulse, clear, status).forEach(layout::addView)
        setContentView(layout)
    }

    override fun onDestroy() {
        client?.disconnect()
        super.onDestroy()
    }

    private fun connectClient() {
        ensureNotificationPermission()
        createNotificationChannel()
        client?.disconnect()
        client = CompanionClient(
            wsUrl = wsUrl.text.toString(),
            tokenStore = store,
            capabilities = capabilities,
            onStatus = { runOnUiThread { status.text = it } },
            onLocalNotification = { title, body -> showLocalNotification(title, body) }
        )
        client?.connect()
    }

    private fun ensureNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1001)
        }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val channel = NotificationChannel(
            notificationChannelId,
            "AgentArk Companion",
            NotificationManager.IMPORTANCE_DEFAULT
        ).apply {
            description = "Local AgentArk companion notifications"
        }
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.createNotificationChannel(channel)
    }

    private fun showLocalNotification(title: String, body: String): Boolean {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            return false
        }
        createNotificationChannel()
        val intent = Intent(this, MainActivity::class.java)
        val flags = PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        val pendingIntent = PendingIntent.getActivity(this, 0, intent, flags)
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(this, notificationChannelId)
        } else {
            Notification.Builder(this)
        }
        val notification = builder
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(Notification.BigTextStyle().bigText(body))
            .setContentIntent(pendingIntent)
            .setAutoCancel(true)
            .build()
        val manager = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        manager.notify((System.currentTimeMillis() % Int.MAX_VALUE).toInt(), notification)
        return true
    }
}
