//! A game opened through prepared bytes kept on disk runs as the game
//! prepared at once, whether the bytes were kept, damaged or missing; bytes
//! kept that preparing never gave stop that game alone.

mod support;

use arcade_host::{Emulator, Kept};
use support::shipped;

/// The frames hash of a game's first 120 updates, opened by `open`.
fn frames(emulator: &mut Emulator, open: impl FnOnce(&mut Emulator)) -> String {
    open(emulator);
    assert_eq!(emulator.step(1, 120).unwrap(), 120);
    let frames = emulator.frames(1).unwrap().expect("watched");
    emulator.close(1).unwrap();
    frames
}

#[test]
fn a_game_runs_the_same_from_bytes_kept_damaged_or_missing() {
    let (name, file) = shipped().into_iter().next().expect("a shipped game");
    let mut emulator = Emulator::hosted(arcade::RUNNER).unwrap();
    let alone = frames(&mut emulator, |e| e.open(1, &file, 1, 0.0, true).unwrap());

    let root = std::env::temp_dir().join(format!("kept-host-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let kept = Kept::new(&root, arcade::RUNNER_NAME, 1 << 24).unwrap();
    let through = |emulator: &mut Emulator| {
        frames(emulator, |e| {
            e.open_kept(&kept, 1, &file, 1, 0.0, true).unwrap();
        })
    };
    // Prepared and kept, then read back.
    assert_eq!(through(&mut emulator), alone);
    let entry = root.join(format!("{}.{name}", arcade::RUNNER_NAME));
    assert!(entry.exists());
    assert_eq!(through(&mut emulator), alone);
    // Damaged: removed, prepared again, kept again.
    let mut bytes = std::fs::read(&entry).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    std::fs::write(&entry, &bytes).unwrap();
    assert_eq!(through(&mut emulator), alone);
    assert_ne!(std::fs::read(&entry).unwrap(), bytes);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bytes_kept_that_preparing_never_gave_stop_that_game_alone() {
    let (name, file) = shipped().into_iter().next().expect("a shipped game");
    let root = std::env::temp_dir().join(format!("kept-forged-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let kept = Kept::new(&root, arcade::RUNNER_NAME, 1 << 24).unwrap();
    // The file as it came, whole by its header, kept as if prepared.
    kept.put(&name, &file).unwrap();
    let mut emulator = Emulator::hosted(arcade::RUNNER).unwrap();
    emulator.open_kept(&kept, 1, &file, 1, 0.0, false).unwrap();
    let why = emulator.stop(1).unwrap().expect("it stops");
    assert!(why.contains("not prepared bytes"), "{why}");
    emulator.open(2, &file, 1, 0.0, false).unwrap();
    assert_eq!(emulator.step(2, 3).unwrap(), 3);
    assert_eq!(emulator.stop(2).unwrap(), None);
    std::fs::remove_dir_all(root).unwrap();
}
