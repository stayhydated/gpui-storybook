package dev.storybook.mobile.test;

import android.app.Instrumentation;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.os.Bundle;
import android.os.SystemClock;
import androidx.test.platform.app.InstrumentationRegistry;
import androidx.test.uiautomator.By;
import androidx.test.uiautomator.UiDevice;
import androidx.test.uiautomator.UiObject2;
import androidx.test.uiautomator.Until;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.util.HashSet;
import java.util.function.Predicate;
import org.json.JSONArray;
import org.json.JSONObject;

/** Runs outside the application process against AndroidX UI Automator 2.4.0. */
public final class NativeQualification extends Instrumentation {
    private UiDevice device;
    private String session = "";
    private long requestId = System.nanoTime();
    private final JSONObject report = new JSONObject();
    private int stateRediscoveries;

    private static final class UnreadyObservation extends java.io.IOException {
        UnreadyObservation(String message) { super(message); }
    }

    @Override public void onCreate(Bundle arguments) {
        super.onCreate(arguments);
        InstrumentationRegistry.registerInstance(this, arguments);
        start();
    }

    @Override public void onStart() {
        Bundle result = new Bundle();
        int status = -1;
        try {
            device = UiDevice.getInstance(this);
            runProof();
            report.put("state_read_rediscoveries", stateRediscoveries);
            report.put("passed", true);
        } catch (Throwable error) {
            status = 0;
            result.putString("error", android.util.Log.getStackTraceString(error));
            try { report.put("error", android.util.Log.getStackTraceString(error)); } catch (Exception ignored) { }
        } finally {
            try { device.setOrientationNatural(); device.unfreezeRotation(); } catch (Exception ignored) { }
            try {
                Files.write(new File(getContext().getFilesDir(), "report.json").toPath(), report.toString(2).getBytes(StandardCharsets.UTF_8));
            } catch (Exception error) { status = 0; result.putString("write_error", error.toString()); }
        }
        result.putString("report", report.toString());
        finish(status, result);
    }

    private void check(boolean condition, String reason) {
        if (!condition) throw new AssertionError(reason);
    }

    private JSONObject command(String operation) throws Exception {
        return new JSONObject().put("operation", operation);
    }

    private JSONObject response(JSONObject command) throws Exception {
        JSONObject request = new JSONObject().put("protocol_version", 1).put("session", session)
            .put("request_id", ++requestId).put("command", command);
        byte[] data = request.toString().getBytes(StandardCharsets.UTF_8);
        check(data.length > 0 && data.length <= 1024 * 1024, "bounded request");
        try (Socket socket = new Socket()) {
            socket.connect(new InetSocketAddress("127.0.0.1", 28437), 5000);
            socket.setSoTimeout(15000);
            DataOutputStream out = new DataOutputStream(socket.getOutputStream());
            out.writeInt(data.length); out.write(data); out.flush();
            DataInputStream in = new DataInputStream(socket.getInputStream());
            int size = in.readInt();
            check(size > 0 && size <= 1024 * 1024, "bounded response");
            byte[] body = new byte[size]; in.readFully(body);
            JSONObject reply = new JSONObject(new String(body, StandardCharsets.UTF_8));
            check(reply.getInt("protocol_version") == 1, "response protocol");
            check(reply.getLong("request_id") == requestId, "response identity");
            check(session.isEmpty() || session.equals(reply.getString("session")), "response session");
            return reply.getJSONObject("outcome");
        }
    }

    private JSONObject rpc(JSONObject command) throws Exception {
        JSONObject outcome = response(command);
        if (outcome.has("Err")) {
            String code = outcome.getJSONObject("Err").optString("code");
            if (code.equals("stale_host") || code.equals("no_live_host"))
                throw new UnreadyObservation(outcome.toString());
            throw new java.io.IOException(outcome.toString());
        }
        return outcome.getJSONObject("Ok").getJSONObject("value");
    }

    private JSONObject ready(Predicate<JSONObject> predicate) throws Exception {
        long deadline = SystemClock.elapsedRealtime() + 20000;
        Exception last = null;
        while (SystemClock.elapsedRealtime() < deadline) {
            try {
                session = "";
                JSONObject host = rpc(command("get_host"));
                if (host.getString("active_route").equals(host.getString("native_route")) && predicate.test(host)) {
                    session = host.getString("session"); return host;
                }
            } catch (Exception error) { last = error; }
            SystemClock.sleep(50);
        }
        throw new AssertionError("rendered host deadline", last);
    }

    private JSONObject ready() throws Exception { return ready(host -> true); }

    private JSONObject state() throws Exception {
        long deadline = SystemClock.elapsedRealtime() + 20000;
        UnreadyObservation last = null;
        while (SystemClock.elapsedRealtime() < deadline) {
            try {
                JSONArray values = rpc(command("read_values")).getJSONArray("values");
                for (int i = 0; i < values.length(); i++) {
                    JSONObject value = values.getJSONObject(i);
                    if (value.getString("key").equals("public-state")) return value.getJSONObject("value");
                }
                throw new AssertionError("public state missing");
            } catch (UnreadyObservation error) {
                // Lifecycle can invalidate a read between rendered discovery and
                // admission. Only read-only observations are repeated here.
                last = error;
                stateRediscoveries++;
                session = "";
                try { session = rpc(command("get_host")).getString("session"); }
                catch (UnreadyObservation ignored) { }
            }
            SystemClock.sleep(50);
        }
        throw new AssertionError("public state readiness deadline", last);
    }

    private void waitState(Predicate<JSONObject> predicate) throws Exception {
        long deadline = SystemClock.elapsedRealtime() + 15000;
        while (SystemClock.elapsedRealtime() < deadline) {
            if (predicate.test(state())) return;
            SystemClock.sleep(50);
        }
        throw new AssertionError("rendered public state deadline");
    }

    private void tab(String label, String route) throws Exception {
        UiObject2 button = device.wait(Until.findObject(By.textStartsWith(label).clazz("android.widget.Button")), 10000);
        check(button != null, "native tab " + label);
        button.click();
        ready(host -> route.equals(host.optString("active_route")));
    }

    private void touch(String key) throws Exception {
        JSONObject geometry = ready().getJSONObject("geometry");
        JSONArray targets = rpc(command("list_targets")).getJSONArray("targets");
        for (int i = 0; i < targets.length(); i++) {
            JSONObject target = targets.getJSONObject(i);
            if (!key.equals(target.getString("key"))) continue;
            JSONObject b = target.getJSONObject("bounds");
            int x = (int) Math.round(geometry.getDouble("x") + geometry.getDouble("scale") * (b.getDouble("x") + b.getDouble("width") / 2));
            int y = (int) Math.round(geometry.getDouble("y") + geometry.getDouble("scale") * (b.getDouble("y") + b.getDouble("height") / 2));
            check(device.click(x, y), "native touch " + key); return;
        }
        throw new AssertionError("GPUI target " + key);
    }

    private void runProof() throws Exception {
        device.executeShellCommand("am start -S -n dev.storybook.mobile/.MainActivity --ez storybook_automation true");
        device.setOrientationNatural();
        JSONObject original = ready(host -> "portrait".equals(host.optString("orientation")));
        String pid = device.executeShellCommand("pidof dev.storybook.mobile").trim();
        tab("NOTES", "embedded-notes");
        tab("COUNTER", "embedded-counter");
        report.put("selector_routes", new JSONArray().put("embedded-notes").put("embedded-counter"));
        int before = state().getInt("count");
        touch("increment");
        waitState(value -> value.optInt("count") == before + 1);
        report.put("count_delta", state().getInt("count") - before);
        File hierarchy = new File(getContext().getFilesDir(), "hierarchy.xml");
        device.dumpWindowHierarchy(hierarchy);
        String xml = new String(Files.readAllBytes(hierarchy.toPath()), StandardCharsets.UTF_8);
        check(xml.contains("android.view.SurfaceView"), "native surface accessibility node");
        report.put("gpui_semantics", "Storybook registry supplies GPUI target bounds");

        JSONObject ticket = rpc(command("prepare_capture"));
        File screenshot = new File(getContext().getFilesDir(), "androidx.png");
        try {
            check(device.takeScreenshot(screenshot), "native screenshot");
        } finally {
            rpc(command("finish_capture").put("ticket", ticket.get("ticket")));
        }
        Bitmap bitmap = BitmapFactory.decodeFile(screenshot.toString());
        JSONObject geometry = ticket.getJSONObject("host").getJSONObject("geometry");
        check(bitmap.getWidth() == geometry.getInt("display_width") && bitmap.getHeight() == geometry.getInt("display_height"), "decoded full display dimensions");
        HashSet<Integer> colors = new HashSet<>();
        for (int y = geometry.getInt("y") + 10; y < geometry.getInt("y") + geometry.getInt("height") - 10; y += 7)
            for (int x = geometry.getInt("x") + 10; x < geometry.getInt("x") + geometry.getInt("width") - 10; x += 7)
                colors.add(bitmap.getPixel(x, y));
        check(colors.size() > 10, "rendered GPUI content");
        report.put("capture", new JSONObject().put("width", bitmap.getWidth()).put("height", bitmap.getHeight()).put("surface_colors", colors.size()).put("ticket", ticket));
        bitmap.recycle();

        tab("NOTES", "embedded-notes");
        rpc(command("set_control").put("key", "note").put("value", new JSONObject().put("type", "text").put("value", "")));
        touch("note-input");
        String ime = device.executeShellCommand("settings get secure default_input_method").trim();
        check(ime.equals("com.android.inputmethod.latin/.LatinIME"), "qualified AOSP keyboard: " + ime);
        int originalHeight = original.getJSONObject("geometry").getInt("height");
        JSONObject keyboardHost = ready(host -> host.optJSONObject("geometry").optInt("height") < originalHeight);
        JSONObject keyboardGeometry = keyboardHost.getJSONObject("geometry");
        double top = keyboardGeometry.getDouble("y") + keyboardGeometry.getDouble("height");
        double bottom = geometry.getDouble("y") + originalHeight;
        double width = keyboardGeometry.getDouble("display_width");
        check(device.click((int) Math.round(width * .1), (int) Math.round(top + (bottom - top) * .46)), "native soft key a");
        waitState(value -> "a".equals(value.optString("note")));
        check(device.click((int) Math.round(width * .5), (int) Math.round(top + (bottom - top) * .90)), "native soft key space");
        waitState(value -> "a ".equals(value.optString("note")));
        report.put("native_ime", state().getString("note"));
        device.pressBack();
        tab("COUNTER", "embedded-counter");
        JSONObject portrait = ready();
        device.setOrientationLeft();
        JSONObject landscape = ready(host -> "landscape".equals(host.optString("orientation")) && !portrait.optString("session").equals(host.optString("session")));
        check(landscape.getLong("surface_revision") > portrait.getLong("surface_revision"), "new rendered surface");
        check(state().getInt("count") == before + 1, "rotation retains state");
        check(device.executeShellCommand("pidof dev.storybook.mobile").trim().equals(pid), "retained process");
        report.put("rotation", landscape);
        device.pressHome();
        JSONObject paused = response(command("get_host"));
        check("no_live_host".equals(paused.getJSONObject("Err").getString("code")), "paused endpoint");
        device.executeShellCommand("am start -n dev.storybook.mobile/.MainActivity --ez storybook_automation true");
        ready();
        check(state().getInt("count") == before + 1, "resume retains state");
        report.put("pause_resume", "retained count and PID");
    }
}
