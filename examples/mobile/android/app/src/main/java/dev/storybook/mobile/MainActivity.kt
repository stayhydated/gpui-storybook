/*
 * InputConnection portions adapted from GPUI Mobile under Apache-2.0.
 * See NOTICE and LICENSE-APACHE for source attribution and selected terms.
 * Modified for the Storybook Kotlin Compose/SurfaceView host and IME insets.
 */
package dev.storybook.mobile

import android.graphics.Color
import android.graphics.PixelFormat
import android.os.Bundle
import android.text.Editable
import android.text.InputType
import android.text.Selection
import android.text.TextWatcher
import android.util.DisplayMetrics
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        route = savedInstanceState?.getString("route", route) ?: route
        dark = savedInstanceState?.getBoolean("dark", false) ?: false
        nativeStart(intent.getBooleanExtra("storybook_automation", false))
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
            changed = { surface.post { publishShell(0) } },
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
        surface.post { publishShell(0) }
    }

    fun gpuiSelect(next: String, nextDark: Boolean, request: Long, action: String) {
        runOnUiThread {
            if (!nativeSelectionCurrent(request)) {
                nativeSelectionSettled(request, false)
                return@runOnUiThread
            }
            select(next, nextDark)
            when (action) {
                "compose.increment" -> compose.increment()
                "compose.reset" -> compose.reset()
            }
            // Observe Compose's committed frame before acknowledging native work.
            compose.afterFrame {
                publishShell(request)
                nativeSelectionSettled(request, true)
            }
        }
    }

    @Suppress("DEPRECATION") // Full display geometry includes compositor system/IME regions.
    private fun publishShell(request: Long) {
        val position = IntArray(2)
        surface.getLocationOnScreen(position)
        val display = DisplayMetrics()
        windowManager.defaultDisplay.getRealMetrics(display)
        nativeShell(
            route, dark, position[0], position[1], surface.width, surface.height,
            resources.displayMetrics.density, display.widthPixels, display.heightPixels,
            request, compose.count(),
        )
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        nativeSurface(holder.surface, resources.displayMetrics.density)
        surface.post { publishShell(0) }
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        nativeSurface(holder.surface, resources.displayMetrics.density)
        surface.post { publishShell(0) }
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) = nativeRelease()

    override fun onResume() {
        super.onResume()
        nativeActive(true)
    }

    override fun onPause() {
        nativeActive(false)
        super.onPause()
    }

    override fun onSaveInstanceState(outState: Bundle) {
        outState.putString("route", route)
        outState.putBoolean("dark", dark)
        outState.putInt("composeCount", compose.count())
        super.onSaveInstanceState(outState)
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
    private external fun nativeSelectionSettled(request: Long, applied: Boolean)
    private external fun nativeStart(automation: Boolean)
    private external fun nativeSurface(surface: Surface, scale: Float)
    private external fun nativeRelease()
    private external fun nativeShell(
        route: String, dark: Boolean, x: Int, y: Int, width: Int, height: Int,
        scale: Float, displayWidth: Int, displayHeight: Int, request: Long, composeCount: Int,
    )
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
