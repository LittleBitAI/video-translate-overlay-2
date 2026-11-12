package dev.vto.overlay

// Keeps the process in the foreground while a job runs (A5): Android stops
// the long work of an app that has left the screen, but not of one with a
// foreground service. Its ongoing notification shows the job's progress (R24).

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat

class JobService : Service() {
    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val n = notice(this, intent?.getStringExtra("title") ?: "", intent?.getIntExtra("percent", 0) ?: 0)
        if (Build.VERSION.SDK_INT >= 29) {
            startForeground(ID, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC)
        } else {
            startForeground(ID, n)
        }
        synchronized(Companion) {
            started = true
            // The job ended before this ran: stopping it then would have crashed the app.
            if (stopAsked) {
                stopAsked = false
                stopSelf()
            }
        }
        // A killed process resumes its job on the next start (pipeline.rs), not from here.
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        synchronized(Companion) { started = false }
        super.onDestroy()
    }

    companion object {
        const val CHANNEL = "vto-job"
        const val ID = 1

        // Whether onStartCommand has run (startForeground called), and a stop that came before it.
        private var started = false
        private var stopAsked = false

        fun show(ctx: Context, state: String, title: String, percent: Int) = synchronized(Companion) {
            val i = Intent(ctx, JobService::class.java).putExtra("title", title).putExtra("percent", percent)
            when (state) {
                "shown" -> {
                    stopAsked = false
                    ContextCompat.startForegroundService(ctx, i)
                }
                "updated" -> ctx.getSystemService(NotificationManager::class.java).notify(ID, notice(ctx, title, percent))
                // A service asked to start must call startForeground before it may stop
                // (Android kills the app otherwise): a job that ends first leaves the stop to it.
                else -> if (started) {
                    ctx.stopService(i)
                } else {
                    stopAsked = true
                    // Progress posted while the service never started (refused) goes too.
                    ctx.getSystemService(NotificationManager::class.java).cancel(ID)
                }
            }
        }

        fun notice(ctx: Context, title: String, percent: Int): Notification {
            val nm = ctx.getSystemService(NotificationManager::class.java)
            if (Build.VERSION.SDK_INT >= 26) {
                nm.createNotificationChannel(NotificationChannel(CHANNEL, "자막 만드는 중", NotificationManager.IMPORTANCE_LOW))
            }
            val open = Intent(ctx, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
            val tap = PendingIntent.getActivity(ctx, 0, open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
            return NotificationCompat.Builder(ctx, CHANNEL)
                .setSmallIcon(android.R.drawable.stat_sys_download)
                .setContentTitle("자막 만드는 중 $percent%")
                .setContentText(title)
                .setProgress(100, percent.coerceIn(0, 100), false)
                .setOngoing(true)
                .setOnlyAlertOnce(true)
                .setContentIntent(tap)
                .build()
        }
    }
}
