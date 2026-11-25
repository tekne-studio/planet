//! The shipped ROMs, as the tests take them.

use std::fs;
use std::path::PathBuf;

/// Where the shipped ROMs are kept, beside their list.
pub const ROMS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../arcade/roms");

/// Every shipped ROM and its name, the sha256 of its bytes, in
/// the name's order.
pub fn shipped() -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<PathBuf> = fs::read_dir(ROMS)
        .expect("the ROMs folder")
        .map(|entry| entry.expect("an entry of the ROMs folder").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "wasm"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path.file_stem().unwrap_or_default().to_string_lossy();
            (name.into_owned(), fs::read(&path).expect("a shipped ROM"))
        })
        .collect()
}
