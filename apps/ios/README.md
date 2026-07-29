# A2D iOS (deferred)

The iOS application UI is deferred beyond v0.1 — Android is the v0.1 delivery target.

**Swift binding generation is mandatory in CI from day one**, independent of the UI deferral. UniFFI
must produce working Swift bindings for the shared Rust core on every change, so a future iOS app can
start from a proven binding layer rather than retrofitting one. The current Swift output is generated
as a disposable smoke-test artifact; it is not checked into an Xcode project because no iOS client or
XCFramework packaging exists yet.

Run the desktop Kotlin and Swift smoke generator with:

```sh
bash tools/generate-bindings.sh
```

The permanent Rust tests also run `crates/a2d-ffi/tests/binding_generation.rs` and require both
languages to expose the expected API symbols. See `docs/BINDING_GENERATION.md` for binding ownership
and `docs/A2D_SMART_NOTEBOOK_V01_TODO.md` Milestones 1.3 and 15 for the remaining iOS-readiness work.

When iOS UI work begins, it follows the same rule as Android: Swift code calls typed Rust use cases
and MUST NOT duplicate domain rules.
