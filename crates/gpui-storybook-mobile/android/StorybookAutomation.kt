package dev.storybook.automation

import android.annotation.TargetApi
import android.app.Activity
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.DisplayMetrics
import android.view.SurfaceView
import android.view.ViewTreeObserver
import org.json.JSONArray
import org.json.JSONObject

/** JNI callbacks forward to the retained Rust NativeShellHandle. */
interface NativeBridge {
    fun publish(event: String): Boolean
    fun isCurrent(request: Long): Boolean
    fun isInputAllowed(): Boolean
}

/** Application state after composition. Return null until the state is committed. */
data class NativeFrame(
    val route: String,
    val appearance: String,
    val actions: JSONArray = JSONArray(),
    val values: JSONArray = JSONArray(),
)

/** One admitted request. Apply on the Activity owner; application callbacks own policy. */
data class NativeSelection(
    val route: String,
    val appearance: String,
    val request: Long,
    val action: JSONObject?,
)

/**
 * SDK-owned lifecycle, geometry observation, and committed-frame acknowledgment.
 * Applications expose storybookDispatch(String): Boolean, forward lifecycle and
 * SurfaceView release events here before releasing the native renderer, and
 * supply their state/application callbacks. Construct on the main thread.
 */
@TargetApi(Build.VERSION_CODES.Q)
class StorybookAutomation(
    private val activity: Activity,
    private val bridge: NativeBridge,
    private val surface: () -> SurfaceView?,
    private val readCommittedFrame: () -> NativeFrame?,
    private val apply: (NativeSelection) -> Boolean,
) : AutoCloseable {
    private val main = Handler(Looper.getMainLooper())
    private var active = false
    private var publishedActive: Boolean? = null
    private var pending: NativeSelection? = null
    private var publication: Publication? = null
    private var lastPublished: String? = null
    private var retryPosted = false
    @Volatile private var closed = false
    private val preDraw = ViewTreeObserver.OnPreDrawListener { observe(); true }

    init {
        check(Looper.myLooper() == Looper.getMainLooper()) { "Construct the native adapter on the Activity owner" }
        check(Build.VERSION.SDK_INT >= 29) { "Native frame acknowledgment requires Android 10 or later" }
        activity.window.decorView.viewTreeObserver.addOnPreDrawListener(preDraw)
    }

    fun setActive(value: Boolean) {
        checkOwner()
        active = value
        if (!active) { cancelSelection(); publication = null }
        publishActive()
        stateChanged()
    }

    /** Notify after application or Compose state changes; revisions follow committed frames. */
    fun stateChanged() {
        if (!closed) activity.window.decorView.invalidate()
    }

    /** Suspend Rust admission synchronously before the application's native surface release. */
    fun surfaceReleased() {
        checkOwner()
        if (closed) return
        cancelSelection()
        publication = null
        lastPublished = null
        bridge.publish(JSONObject().put("event", "surface_released").toString())
    }

    /** Called by JNI on the GPUI thread. Enqueue once; check the permit again on main. */
    fun dispatch(encoded: String): Boolean {
        if (closed || encoded.length > 16 * 1024) return false
        val selection = try {
            val value = JSONObject(encoded)
            NativeSelection(value.getString("route"), value.getString("appearance"),
                value.getLong("request_id"), value.optJSONObject("action"))
        } catch (_: Exception) { return false }
        if (selection.request == 0L || selection.route.isBlank() || selection.route.length > 256
            || selection.appearance.isBlank() || selection.appearance.length > 128) return false
        return main.post {
            val view = surface()
            if (closed || !active || view == null || !usable(view) || !bridge.isCurrent(selection.request)) {
                settle(selection.request, false)
                return@post
            }
            cancelSelection()
            if (!apply(selection)) {
                settle(selection.request, false)
                return@post
            }
            pending = selection
            stateChanged()
        }
    }

    fun isInputAllowed(): Boolean = bridge.isInputAllowed()

    private fun checkOwner() = check(Looper.myLooper() == Looper.getMainLooper()) { "Native lifecycle callbacks belong to the Activity owner" }
    private fun usable(view: SurfaceView) = view.isAttachedToWindow && view.holder.surface.isValid
        && view.width > 0 && view.height > 0 && activity.window.decorView.isHardwareAccelerated
    private fun publishActive() {
        if (publishedActive != active) {
            if (bridge.publish(JSONObject().put("event", "active").put("active", active).toString())) publishedActive = active
            else retryObservation()
        }
    }
    // Native startup can precede the GPUI root. Retry only observations of the
    // current lifecycle/frame; admitted selections are never dispatched again.
    private fun retryObservation() {
        if (closed || retryPosted) return
        retryPosted = true
        main.postDelayed({
            retryPosted = false
            if (!closed) { publishActive(); stateChanged() }
        }, 100)
    }
    private fun settle(request: Long, applied: Boolean) {
        bridge.publish(JSONObject().put("event", "settled").put("request_id", request).put("applied", applied).toString())
    }
    private fun cancelSelection() {
        pending?.let { settle(it.request, false) }
        pending = null
    }

    private data class Publication(val snapshot: String, val request: Long, val surface: SurfaceView)

    private fun observe() {
        publishActive()
        val view = surface() ?: return
        if (closed || !active || !usable(view)) return
        val frame = readCommittedFrame() ?: return
        val snapshot = snapshot(view, frame)
        val request = pending?.takeIf { it.route == frame.route && it.appearance == frame.appearance }?.request ?: 0L
        if (publication != null || (snapshot == lastPublished && request == 0L)) return
        val next = Publication(snapshot, request, view)
        publication = next
        activity.window.decorView.viewTreeObserver.registerFrameCommitCallback {
            main.post { committed(next) }
        }
    }

    private fun committed(next: Publication) {
        publishActive()
        if (publication !== next) return
        publication = null
        if (closed || !active || surface() !== next.surface || !usable(next.surface)) return
        val frame = readCommittedFrame()
        if (frame == null || snapshot(next.surface, frame) != next.snapshot) { stateChanged(); return }
        if (next.request != 0L && (pending?.request != next.request || !bridge.isCurrent(next.request))) {
            cancelSelection()
            stateChanged()
            return
        }
        val event = JSONObject().put("event", "frame").put("observation", JSONObject(next.snapshot)).put("request_id", next.request)
        if (bridge.publish(event.toString())) {
            lastPublished = next.snapshot
            if (next.request != 0L) { pending = null; settle(next.request, true) }
        } else retryObservation()
    }

    @Suppress("DEPRECATION") // Full compositor dimensions include system bars and IME regions.
    private fun snapshot(view: SurfaceView, frame: NativeFrame): String {
        val position = IntArray(2)
        view.getLocationOnScreen(position)
        val display = DisplayMetrics()
        activity.windowManager.defaultDisplay.getRealMetrics(display)
        val geometry = JSONObject().put("x", position[0]).put("y", position[1])
            .put("width", view.width).put("height", view.height).put("scale", activity.resources.displayMetrics.density)
            .put("display_width", display.widthPixels).put("display_height", display.heightPixels)
        return JSONObject().put("route", frame.route).put("appearance", frame.appearance).put("geometry", geometry)
            .put("actions", frame.actions).put("values", frame.values).toString()
    }

    override fun close() {
        checkOwner()
        if (closed) return
        setActive(false)
        surfaceReleased()
        closed = true
        val observer = activity.window.decorView.viewTreeObserver
        if (observer.isAlive) observer.removeOnPreDrawListener(preDraw)
        main.removeCallbacksAndMessages(null)
    }
}
