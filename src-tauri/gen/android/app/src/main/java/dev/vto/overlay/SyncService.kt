package dev.vto.overlay

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat

// The native sync controller owns this lease; it never restarts itself from a killed process.
class SyncService : Service() {
    private var multicast: WifiManager.MulticastLock? = null
    private var serviceOwner = ""

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        synchronized(Companion) {
            if (intent?.getStringExtra("owner") != owner || intent.getLongExtra("generation", -1) != generation || !desired) {
                pending = false
                stopSelf(startId)
                return START_NOT_STICKY
            }
            serviceOwner = owner
            val manager = getSystemService(NotificationManager::class.java)
            if (Build.VERSION.SDK_INT >= 26) {
                manager.createNotificationChannel(NotificationChannel(CHANNEL, "기기간 보관함 연결", NotificationManager.IMPORTANCE_LOW))
            }
            val open = PendingIntent.getActivity(this, ID, Intent(this, MainActivity::class.java), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
            val notice = NotificationCompat.Builder(this, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_notify_sync)
                .setContentTitle("기기간 보관함 연결")
                .setContentText("같은 네트워크의 PC 연결을 확인합니다. Google 앱 계정에서 연결을 중지할 수 있습니다.")
                .setContentIntent(open).setOngoing(true).setOnlyAlertOnce(true).build()
            if (Build.VERSION.SDK_INT >= 29) startForeground(ID, notice, ServiceInfo.FOREGROUND_SERVICE_TYPE_CONNECTED_DEVICE)
            else startForeground(ID, notice)
            try {
                if (multicast == null) {
                    val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
                    multicast = wifi.createMulticastLock("vto:local-sync").apply { setReferenceCounted(false); acquire() }
                }
                activeOwner = serviceOwner
                started = true
                pending = false
            } catch (_: RuntimeException) {
                pending = false
                stopSelf(startId)
            }
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        multicast?.takeIf { it.isHeld }?.release()
        multicast = null
        synchronized(Companion) {
            if (activeOwner == serviceOwner) { started = false; activeOwner = "" }
            pending = false
        }
        super.onDestroy()
    }

    override fun onTimeout(startId: Int, fgsType: Int) {
        synchronized(Companion) { pending = false }
        stopSelf()
    }

    companion object {
        private const val CHANNEL = "vto-local-sync"
        private const val ID = 2002
        private var owner = ""
        private var activeOwner = ""
        private var generation = 0L
        private var desired = false
        private var visible = false
        private var started = false
        private var pending = false

        fun register(ctx: Context, token: String) = synchronized(Companion) {
            require(Regex("[0-9a-f]{32}").matches(token))
            desired = false
            owner = token
            generation = 0
            pending = false
            ctx.stopService(Intent(ctx, SyncService::class.java))
            Unit
        }

        fun set(ctx: Context, token: String, next: Long, enabled: Boolean): Boolean = synchronized(Companion) {
            if (token != owner || next < generation || next < 0) return false
            generation = next
            desired = enabled
            if (!enabled) {
                pending = false
                ctx.stopService(Intent(ctx, SyncService::class.java))
                return false
            }
            if (started && activeOwner == token) return true
            if (!visible || !NotificationManagerCompat.from(ctx).areNotificationsEnabled()) return false
            if (!pending) {
                try {
                    ContextCompat.startForegroundService(ctx, Intent(ctx, SyncService::class.java).putExtra("owner", token).putExtra("generation", next))
                    pending = true
                } catch (_: RuntimeException) { pending = false; return false }
            }
            started && activeOwner == token
        }

        fun foreground(ctx: Context, foreground: Boolean) = synchronized(Companion) {
            visible = foreground
            if (foreground && desired) set(ctx, owner, generation, true)
            Unit
        }
    }
}
