//! Reads the pinned emulator's manifest into constants (`src/lib.rs`).
//! A field missing or of another kind fails the build, naming it, and so
//! does a pinned file whose bytes are not the hash it is named by.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// The pinned emulator's folder, from this crate's.
const EMULATOR: &str = "../../arcade/emulator";

/// A number of the manifest: its field, the constant it becomes, its type,
/// and what it is.
const NUMBERS: [(&str, &str, &str, &str); 12] = [
    (
        "contract",
        "CONTRACT",
        "u32",
        "The version of the contract the runner runs games of.",
    ),
    (
        "engine",
        "ENGINE",
        "u32",
        "The version of the imports the runner asks of a host's engine, module `engine`: a host refuses a runner that asks for another.",
    ),
    ("width", "WIDTH", "u32", "A screen's pixels across."),
    ("height", "HEIGHT", "u32", "A screen's pixels down."),
    (
        "ticks",
        "TICKS",
        "u32",
        "The updates a second a game runs at.",
    ),
    (
        "most",
        "MOST",
        "usize",
        "The most bytes a cartridge's file may have.",
    ),
    (
        "longest",
        "LONGEST",
        "usize",
        "The most updates a script holds, the longest session the console keeps: a script past it gets no room.",
    ),
    (
        "tones",
        "TONES",
        "usize",
        "The calls to `env.tone` an update counts: a host that hears a game apart from the runner hands it no more a call.",
    ),
    (
        "pages",
        "PAGES",
        "u32",
        "The most pages of 64 KiB a game's memory has: a host's engine lets a game hold no more, whatever its file declares.",
    ),
    (
        "table",
        "TABLE",
        "u32",
        "The most elements of a game's one table.",
    ),
    (
        "calls",
        "CALLS",
        "u32",
        "The deepest chain of calls a game reaches before its own count stops it: a host's engine holds at least as many.",
    ),
    (
        "values",
        "VALUES",
        "u32",
        "The most values that chain holds, the frame of the call that starts it beside: a host's engine holds at least as many.",
    ),
];

/// A table of names to bits: its field, its constant, and what it is.
const TABLES: [(&str, &str, &str); 2] = [
    (
        "buttons",
        "BUTTONS",
        "The contract's buttons, each by its name to its bit in the first pad's byte.",
    ),
    (
        "keys",
        "KEYS",
        "The keys one person plays the buttons by, by the names a page's keyboard events give them (`KeyboardEvent.code`), each to its button's bit: the first pad's.",
    ),
];

/// A pinned file whose bytes a host carries: its field, its constants, and what it is.
const FILES: [(&str, &str, &str); 2] = [
    (
        "runner",
        "RUNNER",
        "The runner, `runner.wasm`: the one file every host runs.",
    ),
    (
        "cases",
        "CASES",
        "The cases every host is held to, a JSON array: each a game's file in hex, its seed, its buttons as runs of `[value, count]`, each the number `update` takes, and the run a host must give. A case a host cannot run is a host that fails.",
    ),
];

fn main() {
    let folder = Path::new(&std::env::var("CARGO_MANIFEST_DIR").expect("cargo runs the build"))
        .join(EMULATOR);
    let path = folder.join("emulator.json");
    println!("cargo::rerun-if-changed={}", path.display());
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| fail(&path, &format!("not read: {error}")));
    let Ok(Value::Object(manifest)) = serde_json::from_str::<Value>(&text) else {
        fail(&path, "not a JSON object");
    };
    let mut out = String::new();
    for (field, name, ty, doc) in NUMBERS {
        let number = manifest
            .get(field)
            .and_then(Value::as_u64)
            .filter(|&n| n <= u64::from(u32::MAX))
            .unwrap_or_else(|| missing(&path, field, "a whole number"));
        writeln!(out, "#[doc = {doc:?}]\npub const {name}: {ty} = {number};").unwrap();
    }
    for (field, name, doc) in TABLES {
        let bits =
            table(&manifest, field).unwrap_or_else(|| missing(&path, field, "names to bits"));
        let rows: String = bits
            .iter()
            .map(|(key, bit)| format!("({key:?}, {bit}), "))
            .collect();
        let len = bits.len();
        writeln!(
            out,
            "#[doc = {doc:?}]\npub const {name}: [(&str, u32); {len}] = [{rows}];"
        )
        .unwrap();
    }
    for (field, name, doc) in FILES {
        let file = manifest
            .get(field)
            .and_then(Value::as_str)
            .filter(|file| is_built(file))
            .unwrap_or_else(|| missing(&path, field, "a file named by its sha256"));
        let pinned: PathBuf = folder.join(file);
        println!("cargo::rerun-if-changed={}", pinned.display());
        let bytes = std::fs::read(&pinned).unwrap_or_else(|error| {
            fail(&path, &format!("`{field}` names {file}, not read: {error}"))
        });
        let stem = file.split('.').next().expect("a name has a stem");
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if hash != stem {
            fail(
                &path,
                &format!("`{field}` names {file}, whose bytes hash to {hash}, not to {stem}"),
            );
        }
        let pinned = pinned.to_str().expect("the checkout's path is UTF-8");
        writeln!(
            out,
            "#[doc = {doc:?}]\npub const {name}: &[u8] = include_bytes!({pinned:?});\n\
             /// Its name: the sha256 of its bytes.\npub const {name}_NAME: &str = {stem:?};"
        )
        .unwrap();
    }
    let out_dir = std::env::var("OUT_DIR").expect("cargo names the output folder");
    std::fs::write(Path::new(&out_dir).join("emulator.rs"), out)
        .expect("the output folder takes a file");
}

/// The names and bits of the table at `field`, in the order of their bits.
fn table(manifest: &Map<String, Value>, field: &str) -> Option<Vec<(String, u32)>> {
    let mut rows = manifest
        .get(field)?
        .as_object()?
        .iter()
        .map(|(key, bit)| Some((key.clone(), u32::try_from(bit.as_u64()?).ok()?)))
        .collect::<Option<Vec<_>>>()?;
    rows.sort_by(|(a, x), (b, y)| (x, a).cmp(&(y, b)));
    Some(rows)
}

/// Whether `file` is named as a built file is: 64 hex digits and an extension.
fn is_built(file: &str) -> bool {
    file.split_once('.').is_some_and(|(sha, ext)| {
        sha.len() == 64
            && sha.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            && !ext.is_empty()
            && ext.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

fn missing(path: &Path, field: &str, kind: &str) -> ! {
    fail(path, &format!("no `{field}`, {kind}"))
}

fn fail(path: &Path, why: &str) -> ! {
    panic!("the pinned emulator's manifest, {}: {why}", path.display())
}
