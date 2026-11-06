package @@APP_PACKAGE@@.ui

import androidx.test.ext.junit.runners.AndroidJUnit4
import com.github.takahirom.roborazzi.RobolectricDeviceQualifiers
import com.github.takahirom.roborazzi.captureRoboImage
import @@APP_PACKAGE@@.ui.theme.AppTheme
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Screenshot tests: `./gradlew recordRoborazziDebug` writes PNGs to
 * app/build/outputs/roborazzi/, `verifyRoborazziDebug` fails on visual diffs.
 * Add one @Test per screen/state you want rendered.
 */
@RunWith(AndroidJUnit4::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = RobolectricDeviceQualifiers.Pixel7)
class HomeScreenScreenshotTest {

    @Test
    fun homeScreen_light() {
        captureRoboImage("build/outputs/roborazzi/HomeScreen_light.png") {
            AppTheme(darkTheme = false) { HomeScreen() }
        }
    }

    @Test
    fun homeScreen_dark() {
        captureRoboImage("build/outputs/roborazzi/HomeScreen_dark.png") {
            AppTheme(darkTheme = true) { HomeScreen(initialCount = 3) }
        }
    }
}
