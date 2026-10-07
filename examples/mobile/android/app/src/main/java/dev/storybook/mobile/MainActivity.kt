/*
 * InputConnection portions adapted from GPUI Mobile under Apache-2.0.
 * See NOTICE and LICENSE-APACHE for source attribution and selected terms.
 * Modified for the Storybook Kotlin Compose/SurfaceView host and IME insets.
 */
package dev.storybook.mobile

import android.content.pm.ApplicationInfo
import dev.storybook.automation.NativeBridge
import dev.storybook.automation.NativeFrame
import dev.storybook.automation.NativeSelection
import dev.storybook.automation.StorybookAutomation
import org.json.JSONArray
import org.json.JSONObject
import android.graphics.Color
import android.graphics.PixelFormat
import android.os.Bundle
import android.text.Editable
import android.text.InputType
import android.text.Selection
import android.text.TextWatcher
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.Surface
import android.view.SurfaceHolder
import android.view.SurfaceView
import android.view.ViewGroup
import android.view.WindowInsets
import android.view.WindowInsetsController
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputConnectionWrapper
import android.view.inputmethod.InputMethodManager
import android.widget.EditText
import android.widget.LinearLayout
import androidx.activity.ComponentActivity

/** Compose owns native controls; GPUI owns the embedded content. */
class MainActivity : ComponentActivity(), SurfaceHolder.Callback {
    private lateinit var shell: LinearLayout
    private lateinit var surface: SurfaceView
    private lateinit var compose: ComposeShell
    private var route = "embedded-counter"
    private var dark = false
    private var input: InputProxy? = null
    private lateinit var automation: StorybookAutomation
    private var optedIn = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        route = savedInstanceState?.getString("route", route) ?: route
        dark = savedInstanceState?.getBoolean("dark", false) ?: false
        optedIn = applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE != 0 &&
            (savedInstanceState?.getBoolean("automation") ?: intent.getBooleanExtra("storybook_automation", false))
        nativeStart(optedIn)
        shell = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setOnApplyWindowInsetsListener { view, insets ->
                val bars = insets.getInsets(WindowInsets.Type.systemBars())
                val ime = insets.getInsets(WindowInsets.Type.ime())
                view.setPadding(bars.left, bars.top, bars.right, maxOf(bars.bottom, ime.bottom))
                insets
            }
        }
        compose = ComposeShell(
            this, route, dark, savedInstanceState?.getInt("composeCount", 0) ?: 0,
            selection = ::select,
            changed = { if (::automation.isInitialized) automation.stateChanged() },
        )
        shell.addView(compose.view)
        surface = SurfaceView(this).apply {
            isFocusableInTouchMode = true
            holder.setFormat(PixelFormat.RGBA_8888)
            holder.addCallback(this@MainActivity)
            setOnTouchListener { view, event ->
                // Preserve an active InputConnection when tapping an already
                // focused GPUI text input. Otherwise return key ownership from Compose.
                if (event.actionMasked == MotionEvent.ACTION_DOWN && input?.hasFocus() != true) {
                    view.requestFocus()
                }
                for (i in 0 until event.pointerCount) {
                    val action = when (event.actionMasked) {
                        MotionEvent.ACTION_POINTER_DOWN, MotionEvent.ACTION_POINTER_UP -> {
                            if (i != event.actionIndex) continue
                            if (event.actionMasked == MotionEvent.ACTION_POINTER_DOWN) {
                                MotionEvent.ACTION_DOWN
                            } else {
                                MotionEvent.ACTION_UP
                            }
                        }
                        else -> event.actionMasked
                    }
                    nativeTouch(action, event.getPointerId(i), event.getX(i), event.getY(i))
                }
                true
            }
        }
        shell.addView(surface, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        setContentView(shell)
        automation = StorybookAutomation(this, object : NativeBridge {
            override fun publish(event: String) = nativeAutomationEvent(event)
            override fun isCurrent(request: Long) = nativeSelectionCurrent(request)
            override fun isInputAllowed() = nativeInputAllowed()
        }, surface = { surface }, readCommittedFrame = ::nativeFrame, apply = ::applyNative)
        select(route, dark)
    }

    private fun select(next: String, nextDark: Boolean) {
        route = next
        dark = nextDark
        compose.select(route, dark)
        val lightBars = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
            WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
        window.insetsController?.setSystemBarsAppearance(if (dark) 0 else lightBars, lightBars)
        shell.setBackgroundColor(if (dark) Color.rgb(24, 24, 27) else Color.rgb(250, 250, 250))
        if (::automation.isInitialized) automation.stateChanged()
    }

    /** Single stable JNI entry point implemented by the SDK adapter. */
    fun storybookDispatch(encoded: String): Boolean = automation.dispatch(encoded)

    private fun applyNative(selection: NativeSelection): Boolean {
        if (selection.route !in setOf("embedded-counter", "embedded-notes") || selection.appearance !in setOf("light", "dark")) return false
        val action = selection.action
        if (action != null && (action.optString("action") != "invoke"
            || action.optString("name") !in setOf("compose.increment", "compose.reset")
            || action.optJSONObject("arguments")?.length() != 0)) return false
        select(selection.route, selection.appearance == "dark")
        when (action?.optString("name")) {
            "compose.increment" -> compose.increment()
            "compose.reset" -> compose.reset()
        }
        return true
    }

    private fun nativeFrame(): NativeFrame? {
        if (!compose.isCommitted(route, dark)) return null
        val actions = JSONArray()
        for ((name, description) in listOf("compose.increment" to "Increment the Compose counter", "compose.reset" to "Reset the Compose counter")) {
            val schema = JSONObject().put("type", "object").put("required", JSONArray(listOf("action", "name", "arguments")))
                .put("properties", JSONObject().put("action", JSONObject().put("const", "invoke"))
                    .put("name", JSONObject().put("const", name))
                    .put("arguments", JSONObject().put("type", "object").put("additionalProperties", false)))
                .put("additionalProperties", false)
            actions.put(JSONObject().put("name", name).put("description", description).put("input_schema", schema))
        }
        val values = JSONArray().put(JSONObject().put("key", "native.compose-counter").put("label", "Compose counter")
            .put("value", JSONObject().put("count", compose.count())))
        return NativeFrame(route, if (dark) "dark" else "light", actions, values)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        nativeSurface(holder.surface, resources.displayMetrics.density)
        if (::automation.isInitialized) automation.stateChanged()
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        nativeSurface(holder.surface, resources.displayMetrics.density)
        if (::automation.isInitialized) automation.stateChanged()
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        if (::automation.isInitialized) automation.surfaceReleased()
        nativeRelease()
    }

    override fun onResume() {
        super.onResume()
        nativeActive(true)
        automation.setActive(true)
    }

    override fun onPause() {
        automation.setActive(false)
        nativeActive(false)
        super.onPause()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putBoolean("automation", optedIn)
        outState.putString("route", route)
        outState.putBoolean("dark", dark)
        outState.putInt("composeCount", compose.count())
        super.onSaveInstanceState(outState)
    }

    override fun onDestroy() {
        automation.close()
        super.onDestroy()
    }

    override fun dispatchKeyEvent(event: KeyEvent): Boolean {
        if (input?.hasFocus() == true || compose.view.hasFocus()) {
            return super.dispatchKeyEvent(event)
        }
        nativeKey(event.keyCode, event.action, event.metaState)
        return true
    }

    // InputConnection batching is adapted under Apache-2.0 from GPUI Mobile's
    // GpuiInputActivity.java at 9075e3aa3eea812127f2c60ed66f0cd5798ff245.
    @Suppress("UNUSED_PARAMETER") // Signature called by the GPUI Mobile JNI bridge.
    fun gpuiShowKeyboard(keyboardType: Int, session: Long) {
        runOnUiThread {
            val proxy = input ?: InputProxy().also {
                it.alpha = 0f
                it.setPadding(0, 0, 0, 0)
                addContentView(it, ViewGroup.LayoutParams(1, 1))
                input = it
            }
            proxy.reset(session)
            proxy.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            proxy.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI
            proxy.requestFocus()
            val imm = getSystemService(InputMethodManager::class.java)
            imm.restartInput(proxy)
            imm.showSoftInput(proxy, InputMethodManager.SHOW_IMPLICIT)
        }
    }

    fun gpuiHideKeyboard(session: Long) {
        runOnUiThread {
            val proxy = input ?: return@runOnUiThread
            proxy.reset(session)
            getSystemService(InputMethodManager::class.java)
                .hideSoftInputFromWindow(proxy.windowToken, 0)
            proxy.clearFocus()
        }
    }

    fun gpuiResetComposition(session: Long) {
        runOnUiThread {
            val proxy = input ?: return@runOnUiThread
            proxy.reset(session)
            getSystemService(InputMethodManager::class.java).restartInput(proxy)
        }
    }

    private inner class InputProxy : EditText(this@MainActivity) {
        private var session = 0L
        private var depth = 0
        private var marked = false

        init {
            addTextChangedListener(object : TextWatcher {
                override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
                override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
                override fun afterTextChanged(text: Editable?) {
                    if (depth == 0) {
                        depth++
                        endEdit()
                    }
                }
            })
        }

        fun reset(next: Long) {
            depth++
            text?.clear()
            marked = false
            session = next
            depth = 0
        }

        private fun endEdit() {
            depth--
            if (depth != 0) return
            val editable = text ?: return
            val composing = BaseInputConnection.getComposingSpanStart(editable) >= 0
            if (composing || marked || editable.isNotEmpty()) {
                nativeIme(
                    session, if (composing) 0 else 1, editable.toString(),
                    maxOf(0, Selection.getSelectionStart(editable)),
                    maxOf(0, Selection.getSelectionEnd(editable)),
                )
                marked = composing
                if (!composing) {
                    depth++
                    editable.clear()
                    depth--
                }
            }
        }

        override fun onKeyDown(code: Int, event: KeyEvent): Boolean {
            if (code == KeyEvent.KEYCODE_DEL && text.isNullOrEmpty() && !marked) {
                nativeIme(session, 3, "", 1, 0)
                return true
            }
            return super.onKeyDown(code, event)
        }

        override fun onKeyPreIme(code: Int, event: KeyEvent): Boolean {
            if (code == KeyEvent.KEYCODE_BACK && event.action == KeyEvent.ACTION_UP) {
                nativeIme(session, 4, "", 0, 0)
            }
            return super.onKeyPreIme(code, event)
        }

        override fun onCreateInputConnection(info: EditorInfo): InputConnection? {
            val connection = super.onCreateInputConnection(info) ?: return null
            val currentSession = session
            return object : InputConnectionWrapper(connection, false) {
                override fun beginBatchEdit(): Boolean {
                    if (currentSession != session) return false
                    depth++
                    return super.beginBatchEdit()
                }

                override fun endBatchEdit(): Boolean {
                    if (currentSession != session) return false
                    val result = super.endBatchEdit()
                    if (depth > 0) endEdit()
                    return result
                }

                private fun edit(operation: () -> Boolean): Boolean {
                    if (currentSession != session) return false
                    depth++
                    return try {
                        operation()
                    } finally {
                        endEdit()
                    }
                }

                override fun setComposingText(text: CharSequence?, cursor: Int): Boolean =
                    edit { super.setComposingText(text, cursor) }

                override fun setComposingRegion(start: Int, end: Int): Boolean =
                    edit { super.setComposingRegion(start, end) }

                override fun finishComposingText(): Boolean = edit { super.finishComposingText() }

                override fun commitText(text: CharSequence?, cursor: Int): Boolean =
                    edit { super.commitText(text, cursor) }

                override fun deleteSurroundingText(before: Int, after: Int): Boolean {
                    if (currentSession != session) return false
                    if (text.isNullOrEmpty() && !marked) {
                        nativeIme(session, 2, "", before, after)
                        return true
                    }
                    return edit { super.deleteSurroundingText(before, after) }
                }

                override fun deleteSurroundingTextInCodePoints(before: Int, after: Int): Boolean {
                    if (currentSession != session) return false
                    if (text.isNullOrEmpty() && !marked) {
                        nativeIme(session, 3, "", before, after)
                        return true
                    }
                    return edit { super.deleteSurroundingTextInCodePoints(before, after) }
                }

                override fun sendKeyEvent(event: KeyEvent): Boolean {
                    if (currentSession != session) return false
                    return when (event.keyCode) {
                        KeyEvent.KEYCODE_DEL -> {
                            if (event.action == KeyEvent.ACTION_DOWN) deleteSurroundingText(1, 0)
                            true
                        }
                        KeyEvent.KEYCODE_ENTER -> {
                            if (event.action == KeyEvent.ACTION_DOWN) commitText("\n", 1)
                            true
                        }
                        else -> super.sendKeyEvent(event)
                    }
                }

                override fun performEditorAction(action: Int): Boolean {
                    if (currentSession != session) return false
                    if (action == EditorInfo.IME_ACTION_DONE) {
                        finishComposingText()
                        nativeIme(session, 4, "", 0, 0)
                        return true
                    }
                    return commitText("\n", 1)
                }
            }
        }
    }

    private external fun nativeSelectionCurrent(request: Long): Boolean
    private external fun nativeAutomationEvent(event: String): Boolean
    private external fun nativeInputAllowed(): Boolean
    private external fun nativeStart(automation: Boolean)
    private external fun nativeSurface(surface: Surface, scale: Float)
    private external fun nativeRelease()
    private external fun nativeTouch(action: Int, id: Int, x: Float, y: Float)
    private external fun nativeKey(code: Int, action: Int, meta: Int)
    private external fun nativeActive(active: Boolean)
    private external fun nativeIme(session: Long, kind: Int, text: String, start: Int, end: Int)

    private companion object {
        init {
            System.loadLibrary("gpui_storybook_example_mobile")
        }
    }
}
