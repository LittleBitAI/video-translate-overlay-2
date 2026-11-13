package dev.vto.overlay

import android.webkit.CookieManager
import android.webkit.WebView
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
class YouTubeSessionTest {
    // Run write, force-stop the app, then run read in a new instrumentation process.
    @Test
    fun cookie_policy_and_persistence() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val phase = InstrumentationRegistry.getArguments().getString("phase", "write")
        val read = phase != "write"
        val cookies = CookieManager.getInstance()
        val saved = CountDownLatch(1)
        var accepted = false
        instrumentation.runOnMainSync {
            val view = WebView(instrumentation.targetContext)
            MainActivity().onWebViewCreate(view)
            assertTrue(cookies.acceptCookie())
            assertTrue(cookies.acceptThirdPartyCookies(view))
            view.destroy()
            if (!read) {
                cookies.setCookie(URL, "$COOKIE; Max-Age=3600; Secure; SameSite=None") {
                    accepted = it
                    saved.countDown()
                }
            }
        }
        if (!read) {
            assertTrue(saved.await(10, TimeUnit.SECONDS))
            assertTrue(accepted)
        }
        cookies.flush()
        val present = cookies.getCookie(URL)?.contains(COOKIE) == true
        assertTrue(if (phase == "cleared") !present else present)
    }

    companion object {
        const val URL = "https://www.youtube.com/"
        const val COOKIE = "vto_session_probe=persisted"
    }
}
