package dev.storybook.mobile

import androidx.activity.ComponentActivity
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Tab
import androidx.compose.material3.PrimaryTabRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.ViewCompositionStrategy
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import java.util.function.BiConsumer

/** Activity-owned state shared by Compose callbacks and admitted MCP commands. */
class ComposeShell(
    activity: ComponentActivity,
    initialRoute: String,
    initialDark: Boolean,
    initialCount: Int,
    private val selection: BiConsumer<String, Boolean>,
    private val changed: Runnable,
) {
    private var route by mutableStateOf(initialRoute)
    private var dark by mutableStateOf(initialDark)
    private var count by mutableIntStateOf(initialCount.coerceIn(0, MAX_COUNT))

    val view = ComposeView(activity).apply {
        id = dev.storybook.mobile.R.id.compose_shell
        setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnViewTreeLifecycleDestroyed)
        setContent {
            MaterialTheme(colorScheme = if (dark) darkColorScheme() else lightColorScheme()) {
                Surface {
                    Column(Modifier.fillMaxWidth().semantics { testTagsAsResourceId = true }) {
                        Row(
                            Modifier.fillMaxWidth().padding(horizontal = ShellSpace.inset),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Text("Embedded Storybook", Modifier.weight(1f), style = MaterialTheme.typography.titleLarge)
                            TextButton(onClick = { selection.accept(route, !dark) }, modifier = Modifier.testTag("storybook.appearance")) {
                                Text(if (dark) "Light" else "Dark")
                            }
                        }
                        PrimaryTabRow(selectedTabIndex = if (route == "embedded-notes") 1 else 0) {
                            for ((key, label) in listOf("counter" to "Counter", "notes" to "Notes")) {
                                Tab(
                                    selected = route == "embedded-$key",
                                    onClick = { selection.accept("embedded-$key", dark) },
                                    modifier = Modifier.testTag("storybook.$key"),
                                    text = { Text(label) },
                                )
                            }
                        }
                        Row(
                            Modifier.fillMaxWidth().padding(horizontal = ShellSpace.inset, vertical = ShellSpace.gap),
                            horizontalArrangement = Arrangement.spacedBy(ShellSpace.gap),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text("Compose counter", style = MaterialTheme.typography.labelLarge)
                                Text(count.toString(), Modifier.testTag("storybook.compose.count"), style = MaterialTheme.typography.titleLarge)
                            }
                            OutlinedButton(onClick = ::increment, enabled = count < MAX_COUNT, modifier = Modifier.testTag("storybook.compose.increment")) { Text("Increment") }
                            TextButton(onClick = ::reset, enabled = count != 0, modifier = Modifier.testTag("storybook.compose.reset")) { Text("Reset") }
                        }
                        HorizontalDivider()
                    }
                }
            }
        }
    }

    fun select(nextRoute: String, nextDark: Boolean) {
        route = nextRoute
        dark = nextDark
    }

    fun count(): Int = count

    fun increment() {
        count = (count + 1).coerceAtMost(MAX_COUNT)
        changed.run()
    }

    fun reset() {
        count = 0
        changed.run()
    }

    fun afterFrame(action: Runnable) {
        view.viewTreeObserver.registerFrameCommitCallback(action)
        view.invalidate()
    }

    private companion object {
        const val MAX_COUNT = 1_000_000
    }
}

private object ShellSpace {
    val inset = 16.dp
    val gap = 8.dp
}
