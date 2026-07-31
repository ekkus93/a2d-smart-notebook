//! Private support macros for the integrity checker.

/// `sha2`'s digest array does not implement `LowerHex` under the pinned dependency graph. Keep the
/// checker's existing `format!("{:x}", digest.finalize())` call sites readable while forwarding all
/// other formatting to the standard macro.
macro_rules! format {
    ("{:x}", $value:expr) => {{
        let bytes = $value;
        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        encoded
    }};
    ($($tokens:tt)*) => {
        std::format!($($tokens)*)
    };
}
