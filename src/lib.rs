//! Ycopy: copy files to two destinations while reading each source only once.
//!
//! * [`copier`]  – the copy engine (no UI, no language dependency)
//! * [`i18n`]    – translations embedded from `locales/*.txt`
//! * [`cli`]     – command-line front-end
//! * [`gui`]     – Windows dialog front-end

pub mod cli;
pub mod copier;
pub mod format;
pub mod gui;
pub mod i18n;
pub mod platform;

use std::path::Path;

/// Whether two folder paths designate the same folder (case-insensitive, trailing
/// separators ignored; resolved through the file system when both exist).
pub fn same_folder(a: &Path, b: &Path) -> bool {
    if let (Ok(ca), Ok(cb)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        return ca == cb;
    }
    let norm = |p: &Path| p.to_string_lossy().trim_end_matches(['\\', '/']).to_lowercase();
    norm(a) == norm(b)
}
