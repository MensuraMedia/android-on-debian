package @@APP_PACKAGE@@.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import @@APP_PACKAGE@@.ui.theme.AppTheme

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(initialCount: Int = 0) {
    var count by rememberSaveable { mutableIntStateOf(initialCount) }
    Scaffold(
        topBar = { TopAppBar(title = { Text("@@APP_NAME@@") }) },
    ) { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Card {
                Column(Modifier.padding(16.dp)) {
                    Text("Hello from @@APP_NAME@@", style = MaterialTheme.typography.titleLarge)
                    Text("Rendered by Roborazzi on the JVM — no emulator needed.")
                }
            }
            Text("Button pressed $count times", style = MaterialTheme.typography.bodyLarge)
            Button(onClick = { count++ }) { Text("Press me") }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun HomeScreenPreview() {
    AppTheme { HomeScreen() }
}
