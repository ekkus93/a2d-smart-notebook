//! The asset filesystem commit protocol (TODO 3.3, spec §16.3): create-new temp write →
//! userspace flush → file-content and metadata `sync_all` → close and verify → atomic no-replace
//! finalization → destination-directory sync → temp-link removal → temp-directory sync. A
//! successful `flush()` alone is never treated as durable. Only after this function returns an
//! `Asset` may the caller attempt the separate SQLite transaction.
