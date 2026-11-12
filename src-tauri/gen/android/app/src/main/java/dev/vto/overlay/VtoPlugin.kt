package dev.vto.overlay

// The Android half of src-tauri/src/android.rs: the Gemini key sealed by the
// Android Keystore, the running job's foreground service, and the ready notice.
// Rust calls these through run_mobile_plugin; site pages cannot, because the
// app's ACL (commands::authority) grants no plugin command to any page.

import android.Manifest
import android.app.Activity
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.core.app.ActivityCompat
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class KeyArgs {
    var key: String = ""
}

@InvokeArg
class JobArgs {
    var state: String = ""
    var title: String = ""
    var percent: Int = 0
}

@InvokeArg
class ReadyArgs {
    var jobId: String = ""
    var text: String = ""
}

@InvokeArg
class WatchArgs {
    lateinit var channel: Channel
}

@TauriPlugin
class VtoPlugin(private val activity: Activity) : Plugin(activity) {
    private val prefs get() = activity.getSharedPreferences("vto-key", Context.MODE_PRIVATE)
    private var opener: Channel? = null

    init {
        // The ready notice needs this on Android 13 and later.
        if (Build.VERSION.SDK_INT >= 33 &&
            ContextCompat.checkSelfPermission(activity, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            ActivityCompat.requestPermissions(activity, arrayOf(Manifest.permission.POST_NOTIFICATIONS), 7)
        }
    }

    /** An AES-GCM key that never leaves the Keystore; it seals the Gemini key in app-private storage. */
    private fun secret(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return gen.generateKey()
    }

    private fun guard(invoke: Invoke, f: () -> Unit) {
        try {
            f()
        } catch (e: Exception) {
            invoke.reject(e.javaClass.simpleName + ": " + (e.message ?: ""))
        }
    }

    @Command
    fun getKey(invoke: Invoke) = guard(invoke) {
        val ret = JSObject()
        prefs.getString(SEALED, null)?.let { sealed ->
            val (iv, body) = sealed.split(':').map { Base64.decode(it, Base64.NO_WRAP) }
            val c = Cipher.getInstance("AES/GCM/NoPadding")
            c.init(Cipher.DECRYPT_MODE, secret(), GCMParameterSpec(128, iv))
            ret.put("key", String(c.doFinal(body), Charsets.UTF_8))
        }
        invoke.resolve(ret)
    }

    @Command
    fun setKey(invoke: Invoke) = guard(invoke) {
        val a = invoke.parseArgs(KeyArgs::class.java)
        val c = Cipher.getInstance("AES/GCM/NoPadding")
        c.init(Cipher.ENCRYPT_MODE, secret())
        val body = c.doFinal(a.key.toByteArray(Charsets.UTF_8))
        val sealed = Base64.encodeToString(c.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(body, Base64.NO_WRAP)
        check(prefs.edit().putString(SEALED, sealed).commit()) { "could not write the sealed key" }
        invoke.resolve()
    }

    @Command
    fun deleteKey(invoke: Invoke) = guard(invoke) {
        check(prefs.edit().remove(SEALED).commit()) { "could not delete the sealed key" }
        invoke.resolve()
    }

    /** `shown` starts the foreground service, `updated` moves its progress, `hidden` stops it. */
    @Command
    fun job(invoke: Invoke) = guard(invoke) {
        val a = invoke.parseArgs(JobArgs::class.java)
        JobService.show(activity, a.state, a.title, a.percent)
        invoke.resolve()
    }

    /** The ready notice; tapping it brings the app back with `vto-open:<job id>`. */
    @Command
    fun ready(invoke: Invoke) = guard(invoke) {
        val a = invoke.parseArgs(ReadyArgs::class.java)
        val nm = activity.getSystemService(NotificationManager::class.java)
        // A job resumed after a crash between showing its notice and recording it
        // sends again: while that job's notice is still up, it gets no second one (E2).
        if (nm.activeNotifications.any { it.tag == READY_CHANNEL && it.id == a.jobId.hashCode() }) {
            invoke.resolve()
            return@guard
        }
        if (Build.VERSION.SDK_INT >= 26) {
            nm.createNotificationChannel(NotificationChannel(READY_CHANNEL, "자막 준비 완료", NotificationManager.IMPORTANCE_DEFAULT))
        }
        val open = Intent(activity, MainActivity::class.java)
            .setAction(Intent.ACTION_VIEW)
            .setData(Uri.parse("vto-open:" + a.jobId))
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP)
        val tap = PendingIntent.getActivity(activity, a.jobId.hashCode(), open, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val n = NotificationCompat.Builder(activity, READY_CHANNEL)
            .setSmallIcon(android.R.drawable.stat_sys_download_done)
            .setContentTitle("자막 준비 완료")
            .setContentText(a.text)
            .setContentIntent(tap)
            .setAutoCancel(true)
            .build()
        nm.notify(READY_CHANNEL, a.jobId.hashCode(), n)
        invoke.resolve()
    }

    /** Rust's channel for tapped ready notices, including the one that started this activity. */
    @Command
    fun watchOpen(invoke: Invoke) = guard(invoke) {
        opener = invoke.parseArgs(WatchArgs::class.java).channel
        deliver(activity.intent)
        invoke.resolve()
    }

    override fun onNewIntent(intent: Intent) = deliver(intent)

    private fun deliver(intent: Intent?) {
        if (intent == null) return
        val data = intent.data ?: return
        if (data.scheme != "vto-open") return
        val ch = opener ?: return
        ch.send(JSObject().put("jobId", data.schemeSpecificPart))
        intent.data = null // once
    }

    companion object {
        const val ALIAS = "vto-gemini-key"
        const val SEALED = "sealed"
        const val READY_CHANNEL = "vto-ready"
    }
}
