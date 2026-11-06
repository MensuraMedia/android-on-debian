package @@APP_PACKAGE@@

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import @@APP_PACKAGE@@.ui.HomeScreen
import @@APP_PACKAGE@@.ui.theme.AppTheme

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        AppLog.i("UI", "MainActivity created (restored=${savedInstanceState != null})")
        enableEdgeToEdge()
        setContent {
            AppTheme {
                HomeScreen()
            }
        }
    }
}
