use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use a2d_domain::AssetKind;
use a2d_storage::AssetStore;

const ENOSPC: i32 = 28;
const FILL_CHUNK_BYTES: usize = 256 * 1024;
const RELEASE_BYTES: u64 = 512 * 1024;
const ASSET_BYTES: usize = 2 * 1024 * 1024;

#[test]
#[ignore = "requires A2D_ENOSPC_ROOT on a dedicated small filesystem"]
fn asset_commit_reports_real_enospc_before_finalization() {
    let root = PathBuf::from(
        std::env::var("A2D_ENOSPC_ROOT")
            .expect("A2D_ENOSPC_ROOT must identify the dedicated CI ENOSPC filesystem"),
    );
    std::fs::create_dir_all(&root).unwrap();
    let store = AssetStore::open(&root).unwrap();

    let filler_path = root.join("enospc-filler.bin");
    let mut filler = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&filler_path)
        .unwrap();
    let chunk = vec![0x5a; FILL_CHUNK_BYTES];
    let mut written = 0u64;
    loop {
        match filler.write_all(&chunk) {
            Ok(()) => written += chunk.len() as u64,
            Err(error) if error.raw_os_error() == Some(ENOSPC) => break,
            Err(error) => panic!("expected ENOSPC while filling dedicated filesystem: {error}"),
        }
    }
    drop(filler);
    assert!(written > RELEASE_BYTES, "dedicated filesystem was unexpectedly tiny");

    let filler = OpenOptions::new()
        .write(true)
        .open(&filler_path)
        .unwrap();
    filler.set_len(written - RELEASE_BYTES).unwrap();
    filler.sync_all().unwrap();
    drop(filler);

    let error = store
        .commit(
            &vec![0x33; ASSET_BYTES],
            AssetKind::Original,
            "image/jpeg",
        )
        .unwrap_err();

    assert_eq!(
        error
            .details
            .get("asset_commit_failure_stage")
            .map(String::as_str),
        Some("before_finalization")
    );
    assert_eq!(
        error.details.get("final_file_created").map(String::as_str),
        Some("false")
    );
    assert_eq!(error.details.get("io_error_kind").map(String::as_str), Some("StorageFull"));
    assert!(error.retryable);
    assert!(
        !root.join("assets/originals").read_dir().unwrap().any(|entry| entry.is_ok()),
        "a real ENOSPC failure before finalization must not create a final original asset"
    );

    std::fs::remove_file(filler_path).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
