package dev.vto.overlay

import android.webkit.CookieManager
import android.webkit.WebView
import android.os.Build

// No edge-to-edge: the settings screen sits below the status bar on Android 14
// and earlier; later versions use the page's safe-area padding.
class MainActivity : TauriActivity() {
    private var videoChrome: VideoChromeClient? = null
    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        CookieManager.getInstance().apply {
            setAcceptCookie(true)
            // Google transfers the YouTube session across its sign-in origins.
            setAcceptThirdPartyCookies(webView, true)
        }
        // Wry installs its client after this hook. Wrap it on the next UI turn.
        // API 24-25 have no getter; create the same delegate before Activity STARTED.
        val legacy = if (Build.VERSION.SDK_INT < 26) RustWebChromeClient(this) else null
        webView.post {
            val original = if (Build.VERSION.SDK_INT >= 26) webView.webChromeClient else legacy
            if (original != null) {
                val chrome = VideoChromeClient(this, original)
                videoChrome = chrome
                webView.webChromeClient = chrome
                onBackPressedDispatcher.addCallback(this, chrome.back)
            }
        }
    }

    override fun onPause() {
        CookieManager.getInstance().flush()
        super.onPause()
    }

    override fun onDestroy() {
        videoChrome?.onHideCustomView()
        super.onDestroy()
    }
}
