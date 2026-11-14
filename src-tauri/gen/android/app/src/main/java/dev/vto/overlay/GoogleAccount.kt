package dev.vto.overlay

import android.app.Activity
import android.content.Context
import android.os.CancellationSignal
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.core.content.ContextCompat
import androidx.credentials.ClearCredentialStateRequest
import androidx.credentials.CredentialManager
import androidx.credentials.CredentialManagerCallback
import androidx.credentials.CustomCredential
import androidx.credentials.GetCredentialRequest
import androidx.credentials.GetCredentialResponse
import androidx.credentials.exceptions.ClearCredentialException
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialException
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import app.tauri.annotation.InvokeArg
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import com.google.android.libraries.identity.googleid.GetSignInWithGoogleOption
import com.google.android.libraries.identity.googleid.GoogleIdTokenCredential
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class GoogleSignInArgs {
    var nonce: String = ""
    var webClientId: String = ""
    lateinit var channel: Channel
}

@InvokeArg
class GoogleSessionArgs { var value: String = "" }

/** Owned by one activity/plugin. Rust alone receives tokens; no plugin command is granted to pages. */
class GoogleAccount(private val activity: Activity) {
    private val prefs = activity.getSharedPreferences("vto-google-account", Context.MODE_PRIVATE)
    private var pending: CancellationSignal? = null
    private var generation = 0L

    init {
        (activity as? LifecycleOwner)?.lifecycle?.addObserver(object : DefaultLifecycleObserver {
            override fun onDestroy(owner: LifecycleOwner) { cancel() }
        })
    }

    private fun safe(invoke: Invoke, action: () -> Unit) {
        try { action() } catch (_: Exception) { invoke.reject("Google account operation unavailable") }
    }

    private fun secret(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256).build())
        }.generateKey()
    }

    fun get(invoke: Invoke) = safe(invoke) {
        val result = JSObject()
        prefs.getString("sealed", null)?.let { value ->
            require(value.length <= 192 * 1024)
            val pieces = value.split(':')
            require(pieces.size == 2)
            val iv = Base64.decode(pieces[0], Base64.NO_WRAP)
            val body = Base64.decode(pieces[1], Base64.NO_WRAP)
            require(iv.size == 12)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, secret(), GCMParameterSpec(128, iv))
            val plain = cipher.doFinal(body)
            require(plain.size <= 64 * 1024)
            result.put("value", String(plain, Charsets.UTF_8))
        }
        invoke.resolve(result)
    }

    fun set(invoke: Invoke) = safe(invoke) {
        val value = invoke.parseArgs(GoogleSessionArgs::class.java).value.toByteArray(Charsets.UTF_8)
        require(value.size <= 64 * 1024)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, secret())
        val body = cipher.doFinal(value)
        val sealed = Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(body, Base64.NO_WRAP)
        check(prefs.edit().putString("sealed", sealed).commit())
        invoke.resolve()
    }

    fun delete(invoke: Invoke) = safe(invoke) {
        check(prefs.edit().remove("sealed").commit())
        activity.runOnUiThread {
            try {
                CredentialManager.create(activity).clearCredentialStateAsync(ClearCredentialStateRequest(), null,
                    ContextCompat.getMainExecutor(activity), object : CredentialManagerCallback<Void?, ClearCredentialException> {
                        override fun onResult(result: Void?) {}
                        override fun onError(e: ClearCredentialException) {}
                    })
            } catch (_: Exception) { /* Explicit sign-in always opens the account picker. */ }
        }
        invoke.resolve()
    }

    fun cancel() {
        generation++
        pending?.cancel()
        pending = null
    }

    fun cancel(invoke: Invoke) {
        activity.runOnUiThread { cancel() }
        invoke.resolve()
    }

    fun signIn(invoke: Invoke) = safe(invoke) {
        val args = invoke.parseArgs(GoogleSignInArgs::class.java)
        require(Regex("[A-Za-z0-9_-]{32,128}").matches(args.nonce))
        require(args.webClientId.length <= 200 && args.webClientId.endsWith(".apps.googleusercontent.com"))
        activity.runOnUiThread {
            cancel()
            val ticket = generation
            val cancellation = CancellationSignal().also { pending = it }
            fun deliver(value: JSObject) {
                if (generation == ticket && !cancellation.isCanceled) {
                    pending = null
                    args.channel.send(value)
                }
            }
            try {
                val option = GetSignInWithGoogleOption.Builder(args.webClientId).setNonce(args.nonce).build()
                val request = GetCredentialRequest.Builder().addCredentialOption(option).build()
                CredentialManager.create(activity).getCredentialAsync(activity, request, cancellation,
                    ContextCompat.getMainExecutor(activity), object : CredentialManagerCallback<GetCredentialResponse, GetCredentialException> {
                        override fun onResult(result: GetCredentialResponse) {
                            try {
                                val credential = result.credential
                                require(credential is CustomCredential && credential.type == GoogleIdTokenCredential.TYPE_GOOGLE_ID_TOKEN_CREDENTIAL)
                                val token = GoogleIdTokenCredential.createFrom(credential.data).idToken
                                require(token.length <= 8192)
                                deliver(JSObject().put("idToken", token))
                            } catch (_: Exception) { deliver(JSObject().put("error", "unavailable")) }
                        }
                        override fun onError(e: GetCredentialException) {
                            deliver(JSObject().put("error", if (e is GetCredentialCancellationException) "cancelled" else "unavailable"))
                        }
                    })
            } catch (_: Exception) { deliver(JSObject().put("error", "unavailable")) }
        }
        invoke.resolve()
    }

    companion object { private const val ALIAS = "vto-google-account-key" }
}
