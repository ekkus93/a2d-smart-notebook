package com.a2d.notebook.app

import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.assertIsDisplayed
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.home.HomeScreenTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Instrumentation test (TODO 1.2's instrumentation test source set). Proves the placeholder app
 * actually launches and renders on a real device/emulator -- TODO 1.2's acceptance criterion,
 * not just that the code compiles.
 */
@RunWith(AndroidJUnit4::class)
class HomeScreenLaunchTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun homeScreenTitleIsDisplayedOnLaunch() {
        composeRule.onNodeWithTag(HomeScreenTestTags.TITLE).assertIsDisplayed()
    }
}
