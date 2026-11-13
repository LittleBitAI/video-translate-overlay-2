package dev.vto.overlay

import android.content.pm.ActivityInfo
import android.graphics.Color
import android.net.Uri
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.*
import androidx.activity.OnBackPressedCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class VideoChromeClient(private val activity: AppCompatActivity, private val original: WebChromeClient) : WebChromeClient() {
    private var fullView: View? = null
    private var fullCallback: CustomViewCallback? = null
    private var previousOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    private var previousBars = 0
    private var previousBarsBehavior = 0
    private var wasAwake = false
    val back = object : OnBackPressedCallback(false) {
        override fun handleOnBackPressed() = onHideCustomView()
    }

    override fun onShowCustomView(view: View, callback: CustomViewCallback) {
        if (fullView != null) {
            callback.onCustomViewHidden()
            return
        }
        val decor = activity.window.decorView as ViewGroup
        previousOrientation = activity.requestedOrientation
        val insets = ViewCompat.getRootWindowInsets(decor)
        previousBars = 0
        for (bar in intArrayOf(WindowInsetsCompat.Type.statusBars(), WindowInsetsCompat.Type.navigationBars())) {
            if (insets?.isVisible(bar) != false) previousBars = previousBars or bar
        }
        wasAwake = activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON != 0
        fullView = view
        fullCallback = callback
        view.setBackgroundColor(Color.BLACK)
        decor.addView(view, ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
        WindowInsetsControllerCompat(activity.window, decor).apply {
            previousBarsBehavior = systemBarsBehavior
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            hide(WindowInsetsCompat.Type.systemBars())
        }
        activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
        back.isEnabled = true
    }

    override fun onHideCustomView() {
        val view = fullView ?: return
        val callback = fullCallback
        fullView = null
        fullCallback = null
        back.isEnabled = false
        (view.parent as? ViewGroup)?.removeView(view)
        if (!wasAwake) activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        activity.requestedOrientation = previousOrientation
        WindowInsetsControllerCompat(activity.window, activity.window.decorView).apply {
            systemBarsBehavior = previousBarsBehavior
            show(previousBars)
        }
        callback?.onCustomViewHidden()
    }

    // shortcut: delegate wry 0.55's non-fullscreen callbacks; audit this list when upgrading wry.
    override fun onPermissionRequest(request: PermissionRequest) = original.onPermissionRequest(request)
    override fun onPermissionRequestCanceled(request: PermissionRequest) = original.onPermissionRequestCanceled(request)
    override fun onJsAlert(view: WebView, url: String, message: String, result: JsResult) = original.onJsAlert(view, url, message, result)
    override fun onJsConfirm(view: WebView, url: String, message: String, result: JsResult) = original.onJsConfirm(view, url, message, result)
    override fun onJsPrompt(view: WebView, url: String, message: String, defaultValue: String, result: JsPromptResult) = original.onJsPrompt(view, url, message, defaultValue, result)
    override fun onJsBeforeUnload(view: WebView, url: String, message: String, result: JsResult) = original.onJsBeforeUnload(view, url, message, result)
    override fun onGeolocationPermissionsShowPrompt(origin: String, callback: GeolocationPermissions.Callback) = original.onGeolocationPermissionsShowPrompt(origin, callback)
    override fun onShowFileChooser(view: WebView, callback: ValueCallback<Array<Uri>>, params: FileChooserParams) = original.onShowFileChooser(view, callback, params)
    override fun onConsoleMessage(message: ConsoleMessage) = original.onConsoleMessage(message)
    override fun onReceivedTitle(view: WebView, title: String) = original.onReceivedTitle(view, title)
}
