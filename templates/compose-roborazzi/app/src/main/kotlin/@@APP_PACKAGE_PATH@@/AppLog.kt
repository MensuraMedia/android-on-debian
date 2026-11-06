package @@APP_PACKAGE@@

import android.util.Log
import kotlin.coroutines.cancellation.CancellationException

/**
 * App-wide logging. Every line is tagged `@@APP_NAME@@/<area>` so one filter shows the whole app:
 *
 *     adb logcat | grep '@@APP_NAME@@/'          # live
 *     adb logcat -d | grep '@@APP_NAME@@/'       # dump after the fact
 *
 * Suggested areas: Net (HTTP), Loc (location), DB (storage), UI (screen state changes).
 */
object AppLog {
    private const val PREFIX = "@@APP_NAME@@/"

    // Debug and info lines may carry user data (location, URLs): debug builds only.
    fun d(area: String, msg: String) { if (BuildConfig.DEBUG) Log.d(PREFIX + area, msg) }
    fun i(area: String, msg: String) { if (BuildConfig.DEBUG) Log.i(PREFIX + area, msg) }
    fun w(area: String, msg: String, t: Throwable? = null) = Log.w(PREFIX + area, msg, t)
    fun e(area: String, msg: String, t: Throwable? = null) = Log.e(PREFIX + area, msg, t)

    /** Runs [block], logging how long it took and whether it failed. */
    inline fun <T> timed(area: String, what: String, block: () -> T): T {
        val start = System.nanoTime()
        return try {
            block().also { i(area, "$what ok in ${(System.nanoTime() - start) / 1_000_000} ms") }
        } catch (t: CancellationException) {
            d(area, "$what cancelled")
            throw t
        } catch (t: Throwable) {
            e(area, "$what failed after ${(System.nanoTime() - start) / 1_000_000} ms: $t", t)
            throw t
        }
    }
}
