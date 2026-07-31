package com.a2d.notebook.app

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.a2d.notebook.feature.home.HomeScreenTestTags
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Proves the real activity launches and exposes the three v0.1 top-level workflows. */
@RunWith(AndroidJUnit4::class)
class HomeScreenLaunchTest {
    @get:Rule
    val composeRule = createAndroidComposeRule<MainActivity>()

    @Test
    fun homeScreenNavigationIsDisplayedOnLaunch() {
        composeRule.onNodeWithTag(HomeScreenTestTags.TITLE).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.SCAN_PAGE).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.NOTEBOOKS).assertIsDisplayed()
        composeRule.onNodeWithTag(HomeScreenTestTags.SMART_PAGES).assertIsDisplayed()
    }
}
