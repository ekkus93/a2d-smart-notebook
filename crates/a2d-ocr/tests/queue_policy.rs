use a2d_ocr::OcrQueueLimits;

/// The a2d-ocr queue record is a compatibility/provider contract, not the durable
/// queue owner. Its default must nevertheless agree with the Rust-core policy so
/// consumers cannot accidentally observe the historical 25-attempt behavior.
#[test]
fn compatibility_queue_default_matches_canonical_three_attempt_policy() {
    assert_eq!(OcrQueueLimits::default().max_attempt_count, 3);
}
