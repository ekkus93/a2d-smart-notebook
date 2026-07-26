# A2D iOS (deferred)

The iOS application UI is deferred beyond v0.1 — Android is the v0.1 delivery target.

**Swift binding generation is mandatory in CI from day one**, independent of the UI deferral. UniFFI
must produce working Swift bindings for the shared Rust core on every change, so a future iOS app can
start from a proven binding layer rather than retrofitting one. See
`docs/A2D_SMART_NOTEBOOK_V01_SPEC.md` §31 and `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` Milestone 1.3 for
the CI gate.

When iOS UI work begins, it follows the same rule as Android: Swift code calls typed Rust use cases
and MUST NOT duplicate domain rules.
