//! The host's own calls past what a shell asks: a file too large, a script
//! too long, a step of every update there is, a window past a memory's
//! end. Each answers, and the emulator runs on.

mod support;

use arcade_host::Emulator;
use support::shipped;

/// The emulator, and a shipped game's file.
fn hosted() -> (Emulator, Vec<u8>) {
    let (_, game) = shipped().into_iter().next().expect("a shipped game");
    (Emulator::hosted(arcade::RUNNER).unwrap(), game)
}

/// `game` opened, run a few updates and closed: the emulator still whole.
fn runs_on(emulator: &mut Emulator, game: &[u8]) {
    emulator.open(9, game, 1, 0.0, false).unwrap();
    assert_eq!(emulator.step(9, 3).unwrap(), 3);
    assert_eq!(emulator.stop(9).unwrap(), None);
    emulator.close(9).unwrap();
}

#[test]
fn a_file_past_the_most_is_refused_by_its_size() {
    let (mut emulator, game) = hosted();
    let large = vec![0; arcade::MOST + 1];
    assert_eq!(emulator.check(&large).unwrap().as_deref(), Some("size"));
    assert_eq!(emulator.prepare(&large).unwrap(), None);
    emulator.open(1, &large, 1, 0.0, false).unwrap();
    let why = emulator.stop(1).unwrap().expect("it stops");
    assert!(why.contains("KB"), "{why}");
    assert_eq!(emulator.cover(&large, "ab").unwrap().said, None);
    runs_on(&mut emulator, &game);
}

#[test]
fn a_script_past_the_longest_gets_no_room() {
    let (mut emulator, game) = hosted();
    emulator.open(4, &game, 1, 0.0, false).unwrap();
    emulator.script(4, &[1, 2]).unwrap();
    let broken = emulator
        .script(4, &vec![0; arcade::LONGEST + 1])
        .unwrap_err();
    assert!(
        broken
            .0
            .ends_with(&format!("past the {} one holds", arcade::LONGEST)),
        "{broken}"
    );
    // What it plays plays on, and a script of the longest is one.
    assert_eq!(emulator.scripted(4).unwrap(), Some(2));
    emulator.script(4, &vec![0; arcade::LONGEST]).unwrap();
    assert_eq!(
        emulator.scripted(4).unwrap(),
        Some(2 + arcade::LONGEST as u32)
    );
    emulator.close(4).unwrap();
    runs_on(&mut emulator, &game);
}

#[test]
fn a_step_runs_no_more_updates_than_a_cart_keeps_the_tones_of() {
    let (mut emulator, game) = hosted();
    emulator.open(1, &game, 1, 0.0, false).unwrap();
    assert_eq!(emulator.step(1, u32::MAX).unwrap(), emulator.kept);
    runs_on(&mut emulator, &game);
}

#[test]
fn a_window_past_the_end_is_cut_where_memory_ends() {
    let (mut emulator, game) = hosted();
    emulator.open(1, &game, 1, 0.0, true).unwrap();
    emulator.step(1, 1).unwrap();
    let size = emulator.size(1).unwrap().expect("watched") as usize;
    let window = emulator.window(1, 100, u32::MAX).unwrap().expect("watched");
    assert_eq!(window.bytes.len(), size - 100);
    let past = emulator
        .window(1, u32::MAX, u32::MAX)
        .unwrap()
        .expect("watched");
    assert!(past.bytes.is_empty());
    runs_on(&mut emulator, &game);
}

#[test]
fn a_file_says_the_pads_it_plays_and_one_refused_none() {
    let mut emulator = Emulator::hosted(arcade::RUNNER).unwrap();
    for (name, file) in shipped() {
        let pads = emulator
            .plays(&file)
            .unwrap()
            .expect("a shipped game plays");
        assert!((1..=arcade::PADS).contains(&pads), "{name} plays {pads}");
    }
    assert_eq!(emulator.plays(&vec![0; arcade::MOST + 1]).unwrap(), None);
    assert_eq!(emulator.plays(b"no game").unwrap(), None);
    let (_, game) = shipped().into_iter().next().expect("a shipped game");
    runs_on(&mut emulator, &game);
}
