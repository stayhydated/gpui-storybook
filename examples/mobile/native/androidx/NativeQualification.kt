package dev.storybook.mobile.test

import android.app.Instrumentation
import android.graphics.BitmapFactory
import android.os.Bundle
import android.os.SystemClock
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.File
import java.io.IOException
import java.net.InetSocketAddress
import java.net.Socket
import java.util.Locale
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.roundToInt

/** Runs outside the application process against AndroidX UI Automator 2.4.0. */
class NativeQualification : Instrumentation() {
    private lateinit var device: UiDevice
    private var session = ""
    private var requestId = System.nanoTime()
    private val report = JSONObject()
    private var stateRediscoveries = 0

    private class UnreadyObservation(message: String) : IOException(message)

    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
        InstrumentationRegistry.registerInstance(this, arguments ?: Bundle())
        start()
    }

    override fun onStart() {
        val result = Bundle()
        var status = -1
        try {
            device = UiDevice.getInstance(this)
            runProof()
            report.put("state_read_rediscoveries", stateRediscoveries)
            report.put("passed", true)
        } catch (error: Throwable) {
            status = 0
            result.putString("error", Log.getStackTraceString(error))
            report.put("error", Log.getStackTraceString(error))
        } finally {
            if (::device.isInitialized) {
                try {
                    device.setOrientationNatural()
                    device.unfreezeRotation()
                } catch (_: Exception) {
                    // Preserve the instrumentation result if rotation cleanup also fails.
                }
            }
            try {
                File(context.filesDir, "report.json").writeText(report.toString(2))
            } catch (error: Exception) {
                status = 0
                result.putString("write_error", error.toString())
            }
        }
        result.putString("report", report.toString())
        finish(status, result)
    }

    private fun verify(condition: Boolean, reason: String) {
        if (!condition) throw AssertionError(reason)
    }

    private fun command(operation: String): JSONObject = JSONObject().put("operation", operation)

    private fun response(command: JSONObject): JSONObject {
        val request = JSONObject().put("protocol_version", 2).put("session", session)
            .put("request_id", ++requestId).put("command", command)
        val data = request.toString().toByteArray(Charsets.UTF_8)
        verify(data.isNotEmpty() && data.size <= 1024 * 1024, "bounded request")
        return Socket().use { socket ->
            socket.connect(InetSocketAddress("127.0.0.1", 28437), 5000)
            socket.soTimeout = 15000
            val output = DataOutputStream(socket.getOutputStream())
            output.writeInt(data.size)
            output.write(data)
            output.flush()
            val input = DataInputStream(socket.getInputStream())
            val size = input.readInt()
            verify(size > 0 && size <= 1024 * 1024, "bounded response")
            val body = ByteArray(size)
            input.readFully(body)
            val reply = JSONObject(body.toString(Charsets.UTF_8))
            verify(reply.getInt("protocol_version") == 2, "response protocol")
            verify(reply.getLong("request_id") == requestId, "response identity")
            verify(session.isEmpty() || session == reply.getString("session"), "response session")
            reply.getJSONObject("outcome")
        }
    }

    private fun rpc(command: JSONObject): JSONObject {
        val outcome = response(command)
        if (outcome.has("Err")) {
            when (outcome.getJSONObject("Err").optString("code")) {
                "stale_host", "no_live_host", "host_not_ready" -> throw UnreadyObservation(outcome.toString())
                else -> throw IOException(outcome.toString())
            }
        }
        return outcome.getJSONObject("Ok").getJSONObject("value")
    }

    private fun ready(predicate: (JSONObject) -> Boolean = { true }): JSONObject {
        val deadline = SystemClock.elapsedRealtime() + 20000
        var last: Exception? = null
        while (SystemClock.elapsedRealtime() < deadline) {
            try {
                session = ""
                val host = rpc(command("get_host"))
                if (host.getString("active_route") == host.getString("native_route") && predicate(host)) {
                    session = host.getString("session")
                    return host
                }
            } catch (error: Exception) {
                last = error
            }
            SystemClock.sleep(50)
        }
        throw AssertionError("rendered host deadline", last)
    }

    private fun state(key: String = "public-state"): JSONObject {
        val deadline = SystemClock.elapsedRealtime() + 20000
        var last: UnreadyObservation? = null
        while (SystemClock.elapsedRealtime() < deadline) {
            try {
                val values = rpc(command("read_values")).getJSONArray("values")
                for (i in 0 until values.length()) {
                    val value = values.getJSONObject(i)
                    if (value.getString("key") == key) return value.getJSONObject("value")
                }
                throw AssertionError("public state missing")
            } catch (error: UnreadyObservation) {
                // Lifecycle can invalidate a read between discovery and admission.
                // Only read-only observations are repeated here.
                last = error
                stateRediscoveries++
                session = ""
                try {
                    session = rpc(command("get_host")).getString("session")
                } catch (_: UnreadyObservation) {
                    // Continue observing within the original deadline.
                }
            }
            SystemClock.sleep(50)
        }
        throw AssertionError("public state readiness deadline", last)
    }

    private fun waitState(predicate: (JSONObject) -> Boolean) {
        val deadline = SystemClock.elapsedRealtime() + 15000
        while (SystemClock.elapsedRealtime() < deadline) {
            if (predicate(state())) return
            SystemClock.sleep(50)
        }
        throw AssertionError("rendered public state deadline")
    }

    private fun tab(label: String, route: String) {
        val button = device.wait(
            Until.findObject(By.res("storybook." + label.lowercase(Locale.ROOT))), 10000,
        ) ?: throw AssertionError("native tab $label")
        button.click()
        ready { it.optString("active_route") == route }
    }

    private fun touch(key: String) {
        val geometry = ready().getJSONObject("geometry")
        val targets = rpc(command("list_targets")).getJSONArray("targets")
        for (i in 0 until targets.length()) {
            val target = targets.getJSONObject(i)
            if (key != target.getString("key")) continue
            val bounds = target.getJSONObject("bounds")
            val x = (geometry.getDouble("x") + geometry.getDouble("scale") *
                (bounds.getDouble("x") + bounds.getDouble("width") / 2)).roundToInt()
            val y = (geometry.getDouble("y") + geometry.getDouble("scale") *
                (bounds.getDouble("y") + bounds.getDouble("height") / 2)).roundToInt()
            verify(device.click(x, y), "native touch $key")
            return
        }
        throw AssertionError("GPUI target $key")
    }

    private fun runProof() {
        device.executeShellCommand("am start -S -n dev.storybook.mobile/.MainActivity --ez storybook_automation true")
        device.setOrientationNatural()
        val original = ready { it.optString("orientation") == "portrait" }
        val pid = device.executeShellCommand("pidof dev.storybook.mobile").trim()
        verify(state("native.compose-counter").getInt("count") == 0, "fresh Compose counter")
        val increment = device.wait(Until.findObject(By.res("storybook.compose.increment")), 10000)
            ?: throw AssertionError("Compose increment missing")
        verify(increment.isEnabled, "Compose increment semantics")
        increment.click()
        val composeDeadline = SystemClock.elapsedRealtime() + 10000
        while (state("native.compose-counter").getInt("count") != 1) {
            verify(SystemClock.elapsedRealtime() < composeDeadline, "native Compose count deadline")
            SystemClock.sleep(50)
        }
        verify(
            device.wait(Until.findObject(By.res("storybook.compose.count").text("1")), 10000) != null,
            "rendered Compose count",
        )
        val composeAction = JSONObject().put("action", "invoke").put("name", "compose.increment")
            .put("arguments", JSONObject())
        rpc(command("dispatch_host_action").put("action", composeAction))
        verify(state("native.compose-counter").getInt("count") == 2, "MCP Compose increment")
        verify(
            device.wait(Until.findObject(By.res("storybook.compose.count")), 10000)?.text == "2",
            "MCP acknowledged Compose frame",
        )
        composeAction.put("name", "compose.reset")
        rpc(command("dispatch_host_action").put("action", composeAction))
        verify(state("native.compose-counter").getInt("count") == 0, "MCP Compose reset")
        val reset = device.wait(Until.findObject(By.res("storybook.compose.reset")), 10000)
            ?: throw AssertionError("Compose reset missing")
        verify(!reset.isEnabled, "zero Compose reset disabled")
        composeAction.put("name", "compose.increment")
        rpc(command("dispatch_host_action").put("action", composeAction))
        rpc(command("dispatch_host_action").put("action", composeAction))
        report.put("compose", JSONObject().put("native_click_delta", 1).put("mcp_increment_delta", 1)
            .put("reset", 0).put("retained_count", 2))
        tab("Notes", "embedded-notes")
        tab("Counter", "embedded-counter")
        report.put("selector_routes", JSONArray().put("embedded-notes").put("embedded-counter"))
        val before = state().getInt("count")
        touch("increment")
        waitState { it.optInt("count") == before + 1 }
        report.put("count_delta", state().getInt("count") - before)
        val hierarchy = File(context.filesDir, "hierarchy.xml")
        device.dumpWindowHierarchy(hierarchy)
        verify(hierarchy.readText().contains("android.view.SurfaceView"), "native surface accessibility node")
        report.put("gpui_semantics", "Storybook registry supplies GPUI target bounds")

        val ticket = rpc(command("prepare_capture"))
        val screenshot = File(context.filesDir, "androidx.png")
        try {
            verify(device.takeScreenshot(screenshot), "native screenshot")
        } finally {
            rpc(command("finish_capture").put("ticket", ticket.get("ticket")))
        }
        val bitmap = BitmapFactory.decodeFile(screenshot.toString())
            ?: throw AssertionError("decoded native screenshot")
        val geometry = ticket.getJSONObject("host").getJSONObject("geometry")
        verify(
            bitmap.width == geometry.getInt("display_width") && bitmap.height == geometry.getInt("display_height"),
            "decoded full display dimensions",
        )
        val colors = mutableSetOf<Int>()
        for (y in geometry.getInt("y") + 10 until geometry.getInt("y") + geometry.getInt("height") - 10 step 7) {
            for (x in geometry.getInt("x") + 10 until geometry.getInt("x") + geometry.getInt("width") - 10 step 7) {
                colors.add(bitmap.getPixel(x, y))
            }
        }
        verify(colors.size > 10, "rendered GPUI content")
        report.put("capture", JSONObject().put("width", bitmap.width).put("height", bitmap.height)
            .put("surface_colors", colors.size).put("ticket", ticket))
        bitmap.recycle()

        tab("Notes", "embedded-notes")
        rpc(command("set_control").put("key", "note")
            .put("value", JSONObject().put("type", "text").put("value", "")))
        touch("note-input")
        val ime = device.executeShellCommand("settings get secure default_input_method").trim()
        verify(ime == "com.android.inputmethod.latin/.LatinIME", "qualified AOSP keyboard: $ime")
        val originalHeight = original.getJSONObject("geometry").getInt("height")
        val keyboardHost = ready { it.getJSONObject("geometry").getInt("height") < originalHeight }
        val keyboardGeometry = keyboardHost.getJSONObject("geometry")
        val top = keyboardGeometry.getDouble("y") + keyboardGeometry.getDouble("height")
        val bottom = geometry.getDouble("y") + originalHeight
        val width = keyboardGeometry.getDouble("display_width")
        val scale = keyboardGeometry.getDouble("scale")
        // Suggestion-strip visibility changes IME height; the qualified AOSP
        // portrait key rows retain their positions relative to its bottom.
        verify(bottom - top >= 240 * scale, "qualified keyboard height")
        verify(device.click((width * .1).roundToInt(), (bottom - 160 * scale).roundToInt()), "native soft key a")
        waitState { it.optString("note") == "a" }
        verify(device.click((width * .5).roundToInt(), (bottom - 40 * scale).roundToInt()), "native soft key space")
        waitState { it.optString("note") == "a " }
        report.put("native_ime", state().getString("note"))
        device.pressBack()
        tab("Counter", "embedded-counter")
        val portrait = ready()
        device.setOrientationLeft()
        val landscape = ready {
            it.optString("orientation") == "landscape" && it.optString("session") != portrait.optString("session")
        }
        verify(landscape.getLong("surface_revision") > portrait.getLong("surface_revision"), "new rendered surface")
        verify(state().getInt("count") == before + 1, "rotation retains state")
        verify(state("native.compose-counter").getInt("count") == 2, "rotation retains Compose state")
        verify(device.executeShellCommand("pidof dev.storybook.mobile").trim() == pid, "retained process")
        report.put("rotation", landscape)
        device.pressHome()
        val pausedDeadline = SystemClock.elapsedRealtime() + 10000
        var background = false
        while (SystemClock.elapsedRealtime() < pausedDeadline) {
            session = ""
            val paused = response(command("get_host"))
            val error = paused.optJSONObject("Err")
            if (error?.optString("code") == "host_not_ready" && error.optJSONObject("issue")?.optString("reason") == "background") {
                background = true
                break
            }
            SystemClock.sleep(50)
        }
        verify(background, "paused endpoint reports background lifecycle")
        device.executeShellCommand("am start -n dev.storybook.mobile/.MainActivity --ez storybook_automation true")
        ready()
        verify(state().getInt("count") == before + 1, "resume retains state")
        verify(state("native.compose-counter").getInt("count") == 2, "resume retains Compose state")
        report.put("pause_resume", "retained count and PID")
    }
}
