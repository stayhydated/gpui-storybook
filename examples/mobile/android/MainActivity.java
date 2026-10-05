/*
 * InputConnection portions adapted from GPUI Mobile under Apache-2.0.
 * See NOTICE and LICENSE-APACHE for source attribution and selected terms.
 * Modified for the Storybook plain Activity/SurfaceView host and IME insets.
 */
package dev.storybook.mobile;

import android.app.Activity;
import android.os.Bundle;
import android.graphics.Color;
import android.text.Editable;
import android.text.InputType;
import android.text.Selection;
import android.text.TextWatcher;
import android.view.KeyEvent;
import android.view.Surface;
import android.view.SurfaceHolder;
import android.view.SurfaceView;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.BaseInputConnection;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputMethodManager;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;

/** Native navigation owns route/appearance; GPUI owns the embedded content. */
public final class MainActivity extends Activity implements SurfaceHolder.Callback {
    static { System.loadLibrary("gpui_storybook_example_mobile"); }
    private LinearLayout shell;
    private SurfaceView surface;
    private Button counter, notes, appearance;
    private String route = "embedded-counter";
    private boolean dark;
    private InputProxy input;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        if (state != null) {
            route = state.getString("route", route);
            dark = state.getBoolean("dark", false);
        }
        nativeStart(getIntent().getBooleanExtra("storybook_automation", false));
        shell = new LinearLayout(this);
        shell.setOrientation(LinearLayout.VERTICAL);
        shell.setOnApplyWindowInsetsListener((view, insets) -> {
            android.graphics.Insets bars = insets.getInsets(android.view.WindowInsets.Type.systemBars());
            android.graphics.Insets ime = insets.getInsets(android.view.WindowInsets.Type.ime());
            view.setPadding(bars.left, bars.top, bars.right, Math.max(bars.bottom, ime.bottom));
            return insets;
        });
        TextView title = new TextView(this);
        title.setText("Embedded Storybook"); title.setTextSize(20);
        title.setPadding(dp(16), dp(12), dp(16), dp(12));
        shell.addView(title);
        LinearLayout tabs = new LinearLayout(this);
        counter = button("Counter", () -> select("embedded-counter", dark));
        notes = button("Notes", () -> select("embedded-notes", dark));
        appearance = button("Dark", () -> select(route, !dark));
        for (Button button : new Button[] {counter, notes, appearance})
            tabs.addView(button, new LinearLayout.LayoutParams(0, dp(56), 1));
        shell.addView(tabs);
        surface = new SurfaceView(this);
        surface.getHolder().setFormat(android.graphics.PixelFormat.RGBA_8888);
        surface.getHolder().addCallback(this);
        surface.setOnTouchListener((view, event) -> {
            for (int i = 0; i < event.getPointerCount(); i++) {
                int action = event.getActionMasked();
                if (action == 5 || action == 6) {
                    if (i != event.getActionIndex()) continue;
                    action = action == 5 ? 0 : 1;
                }
                nativeTouch(action, event.getPointerId(i), event.getX(i), event.getY(i));
            }
            return true;
        });
        shell.addView(surface, new LinearLayout.LayoutParams(-1, 0, 1));
        setContentView(shell);
        select(route, dark);
    }

    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
    private Button button(String label, Runnable action) {
        Button button = new Button(this); button.setText(label);
        button.setOnClickListener(view -> action.run()); return button;
    }
    private void select(String next, boolean nextDark) {
        route = next; dark = nextDark;
        counter.setSelected(route.equals("embedded-counter"));
        notes.setSelected(route.equals("embedded-notes"));
        counter.setText(route.equals("embedded-counter") ? "Counter ✓" : "Counter");
        notes.setText(route.equals("embedded-notes") ? "Notes ✓" : "Notes");
        appearance.setText(dark ? "Light" : "Dark");
        int lightBars = android.view.WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
                      | android.view.WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
        getWindow().getInsetsController().setSystemBarsAppearance(dark ? 0 : lightBars, lightBars);
        shell.setBackgroundColor(dark ? Color.rgb(24, 24, 27) : Color.rgb(250, 250, 250));
        ((TextView)shell.getChildAt(0)).setTextColor(dark ? Color.WHITE : Color.BLACK);
        surface.post(() -> publishShell(0));
    }
    public void gpuiSelect(String next, boolean nextDark, long request) {
        runOnUiThread(() -> { select(next, nextDark); surface.post(() -> publishShell(request)); });
    }
    private void publishShell(long request) {
        int[] position = new int[2]; surface.getLocationOnScreen(position);
        android.util.DisplayMetrics display = new android.util.DisplayMetrics();
        getWindowManager().getDefaultDisplay().getRealMetrics(display);
        nativeShell(route, dark, position[0], position[1], surface.getWidth(), surface.getHeight(),
                    getResources().getDisplayMetrics().density, display.widthPixels, display.heightPixels, request);
    }
    @Override public void surfaceCreated(SurfaceHolder holder) {
        nativeSurface(holder.getSurface(), getResources().getDisplayMetrics().density);
        surface.post(() -> publishShell(0));
    }
    @Override public void surfaceChanged(SurfaceHolder holder, int format, int width, int height) {
        nativeSurface(holder.getSurface(), getResources().getDisplayMetrics().density);
        surface.post(() -> publishShell(0));
    }
    @Override public void surfaceDestroyed(SurfaceHolder holder) { nativeRelease(); }
    @Override protected void onResume() { super.onResume(); nativeActive(true); }
    @Override protected void onPause() { nativeActive(false); super.onPause(); }
    @Override protected void onSaveInstanceState(Bundle state) {
        state.putString("route", route); state.putBoolean("dark", dark); super.onSaveInstanceState(state);
    }
    @Override public boolean dispatchKeyEvent(KeyEvent event) {
        if (input != null && input.hasFocus()) return super.dispatchKeyEvent(event);
        nativeKey(event.getKeyCode(), event.getAction(), event.getMetaState());
        return true;
    }

    // InputConnection batching is adapted under Apache-2.0 from gpui-mobile's
    // GpuiInputActivity.java at 9075e3aa3eea812127f2c60ed66f0cd5798ff245.
    public void gpuiShowKeyboard(int keyboardType, long session) {
        runOnUiThread(() -> {
            if (input == null) {
                input = new InputProxy(); input.setAlpha(0f); input.setPadding(0, 0, 0, 0);
                addContentView(input, new ViewGroup.LayoutParams(1, 1));
            }
            input.reset(session);
            input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
            input.setImeOptions(EditorInfo.IME_FLAG_NO_EXTRACT_UI);
            input.requestFocus();
            InputMethodManager imm = (InputMethodManager)getSystemService(INPUT_METHOD_SERVICE);
            imm.restartInput(input); imm.showSoftInput(input, InputMethodManager.SHOW_IMPLICIT);
        });
    }
    public void gpuiHideKeyboard(long session) {
        runOnUiThread(() -> {
            if (input == null) return;
            input.reset(session);
            ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(input.getWindowToken(), 0);
            input.clearFocus();
        });
    }
    public void gpuiResetComposition(long session) {
        runOnUiThread(() -> {
            if (input == null) return;
            input.reset(session);
            ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).restartInput(input);
        });
    }
    private final class InputProxy extends EditText {
        private long session;
        private int depth;
        private boolean marked;
        InputProxy() {
            super(MainActivity.this);
            addTextChangedListener(new TextWatcher() {
                public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
                public void onTextChanged(CharSequence s, int start, int before, int count) {}
                public void afterTextChanged(Editable text) { if (depth == 0) { depth++; endEdit(); } }
            });
        }
        void reset(long next) { depth++; getText().clear(); marked = false; session = next; depth = 0; }
        private void endEdit() {
            if (--depth != 0) return;
            Editable text = getText();
            boolean composing = BaseInputConnection.getComposingSpanStart(text) >= 0;
            if (composing || marked || text.length() > 0) {
                nativeIme(session, composing ? 0 : 1, text.toString(),
                          Math.max(0, Selection.getSelectionStart(text)), Math.max(0, Selection.getSelectionEnd(text)));
                marked = composing;
                if (!composing) { depth++; text.clear(); depth--; }
            }
        }
        @Override public boolean onKeyDown(int code, KeyEvent event) {
            if (code == KeyEvent.KEYCODE_DEL && getText().length() == 0 && !marked) {
                nativeIme(session, 3, "", 1, 0); return true;
            }
            return super.onKeyDown(code, event);
        }
        @Override public boolean onKeyPreIme(int code, KeyEvent event) {
            if (code == KeyEvent.KEYCODE_BACK && event.getAction() == KeyEvent.ACTION_UP)
                nativeIme(session, 4, "", 0, 0);
            return super.onKeyPreIme(code, event);
        }
        @Override public InputConnection onCreateInputConnection(EditorInfo info) {
            InputConnection connection = super.onCreateInputConnection(info);
            if (connection == null) return null;
            final long currentSession = session;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean beginBatchEdit() {
                    if (currentSession != session) return false;
                    depth++; return super.beginBatchEdit();
                }
                @Override public boolean endBatchEdit() {
                    if (currentSession != session) return false;
                    boolean result = super.endBatchEdit(); if (depth > 0) endEdit(); return result;
                }
                @Override public boolean setComposingText(CharSequence text, int cursor) {
                    if (currentSession != session) return false;
                    depth++; try { return super.setComposingText(text, cursor); } finally { endEdit(); }
                }
                @Override public boolean setComposingRegion(int start, int end) {
                    if (currentSession != session) return false;
                    depth++; try { return super.setComposingRegion(start, end); } finally { endEdit(); }
                }
                @Override public boolean finishComposingText() {
                    if (currentSession != session) return false;
                    depth++; try { return super.finishComposingText(); } finally { endEdit(); }
                }
                @Override public boolean commitText(CharSequence text, int cursor) {
                    if (currentSession != session) return false;
                    depth++; try { return super.commitText(text, cursor); } finally { endEdit(); }
                }
                @Override public boolean deleteSurroundingText(int before, int after) {
                    if (currentSession != session) return false;
                    if (getText().length() == 0 && !marked) { nativeIme(session, 2, "", before, after); return true; }
                    depth++; try { return super.deleteSurroundingText(before, after); } finally { endEdit(); }
                }
                @Override public boolean deleteSurroundingTextInCodePoints(int before, int after) {
                    if (currentSession != session) return false;
                    if (getText().length() == 0 && !marked) { nativeIme(session, 3, "", before, after); return true; }
                    depth++; try { return super.deleteSurroundingTextInCodePoints(before, after); } finally { endEdit(); }
                }
                @Override public boolean sendKeyEvent(KeyEvent event) {
                    if (currentSession != session) return false;
                    if (event.getKeyCode() == KeyEvent.KEYCODE_DEL) {
                        if (event.getAction() == KeyEvent.ACTION_DOWN) deleteSurroundingText(1, 0); return true;
                    }
                    if (event.getKeyCode() == KeyEvent.KEYCODE_ENTER) {
                        if (event.getAction() == KeyEvent.ACTION_DOWN) commitText("\n", 1); return true;
                    }
                    return super.sendKeyEvent(event);
                }
                @Override public boolean performEditorAction(int action) {
                    if (currentSession != session) return false;
                    if (action == EditorInfo.IME_ACTION_DONE) {
                        finishComposingText(); nativeIme(session, 4, "", 0, 0); return true;
                    }
                    return commitText("\n", 1);
                }
            };
        }
    }
    private native void nativeStart(boolean automation);
    private native void nativeSurface(Surface surface, float scale);
    private native void nativeRelease();
    private native void nativeShell(String route, boolean dark, int x, int y, int width, int height, float scale, int displayWidth, int displayHeight, long request);
    private native void nativeTouch(int action, int id, float x, float y);
    private native void nativeKey(int code, int action, int meta);
    private native void nativeActive(boolean active);
    private native void nativeIme(long session, int kind, String text, int start, int end);
}
