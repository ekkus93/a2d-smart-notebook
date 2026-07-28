from pathlib import Path

path = Path("apps/android/app/src/main/kotlin/com/a2d/notebook/feature/scanner/singlepage/SinglePageScannerViewModel.kt")
text = path.read_text()


def replace_once(old: str, new: str) -> None:
    global text
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected one match, found {count}: {old[:120]!r}")
    text = text.replace(old, new)


replace_once(
    '''        if (mutableState.value.processing || mutableState.value.activeNotebook?.id == notebook.id) {
            return
        }
''',
    '''        if (
            mutableState.value.processing ||
                mutableState.value.registrationInProgress ||
                mutableState.value.activeNotebook?.id == notebook.id
        ) {
            return
        }
''',
)

replace_once(
    '''        val generation = current.cameraGeneration
        update { it.copy(registrationInProgress = true, error = null) }
''',
    '''        update { it.copy(registrationInProgress = true, error = null) }
''',
)

replace_once(
    '''                    if (
                        generation != mutableState.value.cameraGeneration ||
                            mutableState.value.reviewArtifact?.stagingPath != artifact.stagingPath
                    ) {
                        return@launch
                    }
                    pendingStagingFile = null
''',
    '''                    pendingStagingFile = null
''',
)

replace_once(
    '''        if (registrationJob?.isActive != true) {
            pendingStagingFile?.let(::safeDelete)
            pendingStagingFile = null
            pendingCapturedAtMs = null
        }
        if (deleteReview && registrationJob?.isActive != true) {
            mutableState.value.reviewArtifact?.let { safeDelete(File(it.stagingPath)) }
        }
''',
    '''        if (!mutableState.value.registrationInProgress) {
            pendingStagingFile?.let(::safeDelete)
            pendingStagingFile = null
            pendingCapturedAtMs = null
            if (deleteReview) {
                mutableState.value.reviewArtifact?.let { safeDelete(File(it.stagingPath)) }
            }
        }
''',
)

path.write_text(text)
# Trigger the default-branch pull-request applicator.
