//! Prepared bytes kept on disk, so the native host prepares a file once.
//! Preparing on an interpreter costs milliseconds a file; reading the bytes
//! back costs almost nothing. Entries sit in one folder, each named by the
//! runner's hash and the file's, `<runner>.<file>`: a runner prepares by its
//! own rules, so it reads only the entries its own name begins. Builds with
//! different runners run side by side over one folder that holds at most
//! [`Kept::most`] bytes, and what was used longest ago goes first, whichever
//! runner kept it. No folder is made or removed but the one: none can go
//! from under a write.
//!
//! An entry is read as hostile: its header (what it is, the body's length,
//! the body's CRC-32) is read first, and the body only when its length is
//! the entry's and within the folder's most. An entry that fails, cut short,
//! damaged, of another layout or too large, is removed and the file
//! prepared again.
//!
//! The check finds damage, never a forger. Bytes of the prepared shape that
//! preparing never gave open as a game that may run unmetered, an update
//! that never ends, so the folder is as trusted as the host's own program:
//! one only the user who runs the host can write, as the platform's cache
//! folder is. A folder others can write, shared or synced, is never given.
//! A tag over the body keyed by what the host holds in the open (the
//! runner's hash) would only name the writer's runner, and one keyed by a
//! secret beside the cache is readable by whoever can write the cache;
//! checking that the code is metered is walking it again, the cost the
//! cache saves.
//!
//! Keeping never stands in the way: an entry another process removed
//! first, or one this host may not remove, is passed over.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What an entry starts with: what it is, and its layout's version.
const MAGIC: [u8; 8] = *b"prepare1";
/// The header: [`MAGIC`], the body's length (eight bytes) and its CRC-32
/// (four), little end first.
const HEADER: usize = MAGIC.len() + 8 + 4;
/// What a file being written ends with until it is whole.
const PART: &str = "part";

/// Whether `name` is a hash as the emulator names files: 64 hex digits, so
/// a name never reaches outside the folder.
fn is_hash(name: &str) -> bool {
    name.len() == 64
        && name
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `name` is one a [`Kept`] writes: an entry, `<runner>.<file>`,
/// or one being written, `<runner>.<file>.<process>.part`. Nothing else in
/// the folder is its to remove.
fn is_kept(name: &str) -> bool {
    let mut parts = name.split('.');
    let entry = parts.next().is_some_and(is_hash) && parts.next().is_some_and(is_hash);
    let rest: Vec<&str> = parts.collect();
    entry
        && match rest[..] {
            [] => true,
            [process, part] => {
                !process.is_empty()
                    && process.bytes().all(|byte| byte.is_ascii_digit())
                    && part == PART
            }
            _ => false,
        }
}

/// The prepared bytes one runner keeps, by the hash of each file.
#[derive(Clone, Debug)]
pub struct Kept {
    folder: PathBuf,
    runner: String,
    most: u64,
}

impl Kept {
    /// `runner`'s prepared bytes in `folder`, made when it is not there,
    /// which holds at most `most` bytes: brought within them now.
    pub fn new(folder: &Path, runner: &str, most: u64) -> io::Result<Kept> {
        if !is_hash(runner) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("a runner is named by its hash, not {runner}"),
            ));
        }
        fs::create_dir_all(folder)?;
        let kept = Kept {
            folder: folder.to_owned(),
            runner: runner.to_owned(),
            most,
        };
        kept.evict();
        Ok(kept)
    }

    /// The most bytes the folder holds.
    pub fn most(&self) -> u64 {
        self.most
    }

    /// Where the entry of the file whose hash is `file` sits.
    fn entry(&self, file: &str) -> PathBuf {
        self.folder.join(format!("{}.{file}", self.runner))
    }

    /// The prepared bytes of the file whose hash is `file`, when they are
    /// kept and whole; an entry that is not is removed.
    pub fn get(&self, file: &str) -> Option<Vec<u8>> {
        if !is_hash(file) {
            return None;
        }
        let path = self.entry(file);
        // Written to as well: its time is set as it is used.
        let mut entry = File::options().read(true).write(true).open(&path).ok()?;
        let Some(body) = self.body(&mut entry) else {
            let _ = fs::remove_file(&path);
            return None;
        };
        // Used now: the last to go.
        let _ = entry.set_modified(SystemTime::now());
        Some(body)
    }

    /// The body of `entry`, read only when its header holds: [`MAGIC`],
    /// the length the entry has, within [`Kept::most`], and its CRC-32.
    fn body(&self, entry: &mut File) -> Option<Vec<u8>> {
        let mut header = [0; HEADER];
        entry.read_exact(&mut header).ok()?;
        let (magic, rest) = header.split_at(MAGIC.len());
        let (len, crc) = rest.split_at(8);
        let len = u64::from_le_bytes(len.try_into().ok()?);
        let crc = u32::from_le_bytes(crc.try_into().ok()?);
        let size = entry.metadata().ok()?.len();
        let fits =
            magic == MAGIC && size <= self.most && size.checked_sub(HEADER as u64) == Some(len);
        if !fits {
            return None;
        }
        let mut body = vec![0; usize::try_from(len).ok()?];
        entry.read_exact(&mut body).ok()?;
        (crc32fast::hash(&body) == crc).then_some(body)
    }

    /// Keeps `prepared` as the file whose hash is `file`, whole or not at
    /// all: written aside, then moved into place. Then what was used
    /// longest ago goes, until the folder holds at most [`Kept::most`]
    /// bytes.
    pub fn put(&self, file: &str, prepared: &[u8]) -> io::Result<()> {
        if !is_hash(file) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("a file is named by its hash, not {file}"),
            ));
        }
        let size = (HEADER + prepared.len()) as u64;
        if size > self.most {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{size} bytes, past the most kept, {}", self.most),
            ));
        }
        let path = self.entry(file);
        let process = std::process::id();
        let part = self
            .folder
            .join(format!("{}.{file}.{process}.{PART}", self.runner));
        let mut entry = File::create(&part)?;
        entry.write_all(&MAGIC)?;
        entry.write_all(&(prepared.len() as u64).to_le_bytes())?;
        entry.write_all(&crc32fast::hash(prepared).to_le_bytes())?;
        entry.write_all(prepared)?;
        entry.sync_all()?;
        fs::rename(&part, &path)?;
        self.evict();
        Ok(())
    }

    /// Removes what was used longest ago while the folder holds more than
    /// [`Kept::most`] bytes: every runner's entries, and writes cut short,
    /// whose time is when they stopped. A write going on in another process
    /// may go too, when the folder is that full: that write fails, and its
    /// file is prepared again at its next open.
    fn evict(&self) {
        let Ok(entries) = fs::read_dir(&self.folder) else {
            return;
        };
        let mut held: Vec<(SystemTime, u64, PathBuf)> = entries
            .flatten()
            .filter(|entry| is_kept(&entry.file_name().to_string_lossy()))
            .filter_map(|entry| {
                let meta = entry.metadata().ok()?;
                meta.is_file()
                    .then_some((meta.modified().ok()?, meta.len(), entry.path()))
            })
            .collect();
        let mut size: u64 = held.iter().map(|(_, len, _)| len).sum();
        held.sort();
        for (_, len, path) in held {
            if size <= self.most {
                break;
            }
            // Removed, gone already or not this host's to remove: either
            // way, no longer its to hold within the most.
            let _ = fs::remove_file(path);
            size -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNER: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const ROCKS: &str = "2222222222222222222222222222222222222222222222222222222222222222";
    const KINGS: &str = "3333333333333333333333333333333333333333333333333333333333333333";
    const HUES: &str = "4444444444444444444444444444444444444444444444444444444444444444";

    fn root(test: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("kept-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    /// Sets when `path` was last used to `secs` after the epoch.
    fn used(path: &Path, secs: u64) {
        let at = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        let entry = File::options().write(true).open(path).unwrap();
        entry.set_modified(at).unwrap();
    }

    /// The bytes a [`Kept`] holds in `root`.
    fn held(root: &Path) -> u64 {
        fs::read_dir(root)
            .unwrap()
            .flatten()
            .filter(|entry| is_kept(&entry.file_name().to_string_lossy()))
            .map(|entry| entry.metadata().unwrap().len())
            .sum()
    }

    #[test]
    fn bytes_kept_come_back_whole() {
        let root = root("whole");
        let kept = Kept::new(&root, RUNNER, 1 << 20).unwrap();
        assert_eq!(kept.get(ROCKS), None);
        kept.put(ROCKS, b"prepared rocks").unwrap();
        assert_eq!(kept.get(ROCKS).as_deref(), Some(&b"prepared rocks"[..]));
        assert!(root.join(format!("{RUNNER}.{ROCKS}")).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_damaged_entry_is_removed_and_never_used() {
        let root = root("damaged");
        let kept = Kept::new(&root, RUNNER, 1 << 20).unwrap();
        kept.put(ROCKS, b"prepared rocks").unwrap();
        let path = kept.entry(ROCKS);
        let mut bytes = fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert_eq!(kept.get(ROCKS), None);
        assert!(!path.exists());
        // Cut short, or of another layout: the same.
        for broken in [&bytes[..HEADER - 1], b"prepare2 and the rest of it"] {
            fs::write(&path, broken).unwrap();
            assert_eq!(kept.get(ROCKS), None);
            assert!(!path.exists());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_entry_past_the_most_is_removed_unread() {
        let root = root("large");
        let kept = Kept::new(&root, RUNNER, 1 << 20).unwrap();
        kept.put(ROCKS, b"prepared rocks").unwrap();
        let path = kept.entry(ROCKS);
        // A terabyte, sparse: read whole, it would never come back.
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(1 << 40)
            .unwrap();
        assert_eq!(kept.get(ROCKS), None);
        assert!(!path.exists());
        // A header that claims more than the folder holds.
        let mut claims = MAGIC.to_vec();
        claims.extend(u64::MAX.to_le_bytes());
        claims.extend([0; 4]);
        fs::write(&path, &claims).unwrap();
        assert_eq!(kept.get(ROCKS), None);
        assert!(!path.exists());
        assert!(kept.put(ROCKS, &vec![0; 1 << 20]).is_err(), "past the most");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runners_side_by_side_keep_their_own_bytes() {
        let root = root("side");
        let one = Kept::new(&root, RUNNER, 1 << 20).unwrap();
        one.put(ROCKS, b"prepared by one").unwrap();
        let other = Kept::new(&root, KINGS, 1 << 20).unwrap();
        other.put(ROCKS, b"prepared by another").unwrap();
        // Each starts again, beside the other.
        let one = Kept::new(&root, RUNNER, 1 << 20).unwrap();
        let other = Kept::new(&root, KINGS, 1 << 20).unwrap();
        assert_eq!(one.get(ROCKS).as_deref(), Some(&b"prepared by one"[..]));
        assert_eq!(
            other.get(ROCKS).as_deref(),
            Some(&b"prepared by another"[..])
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_entries_used_longest_ago_go_first() {
        let root = root("evict");
        let entry = (HEADER + 100) as u64;
        let kept = Kept::new(&root, RUNNER, 2 * entry).unwrap();
        kept.put(ROCKS, &[1; 100]).unwrap();
        kept.put(KINGS, &[2; 100]).unwrap();
        // Rocks used first, then Kings, long ago; then Rocks used again now.
        used(&kept.entry(ROCKS), 1);
        used(&kept.entry(KINGS), 2);
        assert!(kept.get(ROCKS).is_some());
        kept.put(HUES, &[3; 100]).unwrap();
        assert!(kept.get(KINGS).is_none(), "used longest ago");
        assert!(kept.get(ROCKS).is_some() && kept.get(HUES).is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_folder_holds_its_most_across_runners_eldest_first() {
        let root = root("across");
        let entry = (HEADER + 100) as u64;
        let other = Kept::new(&root, KINGS, 2 * entry).unwrap();
        other.put(ROCKS, &[1; 100]).unwrap();
        used(&other.entry(ROCKS), 2);
        // A write cut short, longer ago, counts as what it holds; what a
        // Kept never writes is never its to remove.
        let part = root.join(format!("{KINGS}.{HUES}.7.{PART}"));
        fs::write(&part, [0; 200]).unwrap();
        used(&part, 1);
        fs::write(root.join("notes"), [0; 1000]).unwrap();
        let kept = Kept::new(&root, RUNNER, 2 * entry).unwrap();
        assert!(!part.exists(), "the folder held more than its most");
        assert!(root.join("notes").exists());
        assert!(other.get(ROCKS).is_some());
        used(&other.entry(ROCKS), 2);
        kept.put(ROCKS, &[2; 100]).unwrap();
        kept.put(KINGS, &[3; 100]).unwrap();
        assert!(
            other.get(ROCKS).is_none(),
            "another runner's, used longest ago"
        );
        assert!(kept.get(ROCKS).is_some() && kept.get(KINGS).is_some());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn starts_and_keeps_at_once_never_fail_the_cache() {
        let root = root("race");
        let most = 3 * (HEADER + 64) as u64;
        let runners: Vec<String> = (0..8).map(|n| format!("{n}").repeat(64)).collect();
        std::thread::scope(|scope| {
            for runner in &runners {
                let root = &root;
                scope.spawn(move || {
                    for _ in 0..20 {
                        let kept = Kept::new(root, runner, most).expect("a start always keeps");
                        // A write another's eviction took fails; no other does.
                        if let Err(error) = kept.put(ROCKS, runner.as_bytes()) {
                            assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
                        }
                        // Another's eviction may have taken it already.
                        if let Some(bytes) = kept.get(ROCKS) {
                            assert_eq!(bytes, runner.as_bytes());
                        }
                    }
                });
            }
        });
        assert!(held(&root) <= most, "{} bytes held", held(&root));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_name_that_is_not_a_hash_reaches_nothing() {
        let root = root("names");
        assert!(Kept::new(&root, "../up", 1 << 20).is_err());
        let kept = Kept::new(&root, RUNNER, 1 << 20).unwrap();
        assert!(kept.put("../../escape", b"x").is_err());
        assert_eq!(kept.get("../../escape"), None);
        assert!(!is_kept(&format!("{RUNNER}.{ROCKS}.x.{PART}")));
        assert!(!is_kept(&format!("{RUNNER}.{ROCKS}.7.part.more")));
        assert!(!is_kept(&format!("{RUNNER}.notes")));
        fs::remove_dir_all(root).unwrap();
    }
}
