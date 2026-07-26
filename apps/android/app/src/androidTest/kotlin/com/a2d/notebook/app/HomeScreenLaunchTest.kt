package com.a2d.notebook.app

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
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

    /**
     * Milestone 2's still-open acceptance criterion: "Android calls Rust and renders a typed
     * response." Asserts the rendered text is a real 26-character canonical Crockford Base32
     * PageId -- a2d-domain's actual encoder, crossing the UniFFI/JNA boundary -- not merely that
     * some text is present.
     */
    @Test
    fun homeScreenRendersARealPageIdGeneratedByRust() {
        val node = composeRule.onNodeWithTag(HomeScreenTestTags.RUST_GENERATED_ID)
        node.assertIsDisplayed()
        val text = node.fetchSemanticsNode().config
            .getOrNull(SemanticsProperties.Text)
            ?.joinToString(separator = "") { it.text }
            ?: error("Rust-generated-id node has no text")
        val idPattern = Regex("Rust-generated Page ID: [0-9A-HJKMNP-TV-Z]{26}$")
        assert(idPattern.containsMatchIn(text)) {
            "expected a canonical 26-char Crockford Base32 PageId, got: $text"
        }
    }
}
