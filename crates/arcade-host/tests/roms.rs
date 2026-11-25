//! The ROMs this version ships, as a cabinet receives them: each read for
//! its cover as a host reads one, then opened, which prepares it, and run
//! through the native host, a while on its title, started and played, with
//! no stop. What ships is what ran here. The `.wasm` files beside the list
//! are the list's, every one and no other: `scripts/roms.test.ts` holds it.

mod support;

use arcade_host::Emulator;
use support::{ROMS, shipped};

/// The updates each game runs.
const UPDATES: usize = 900;

/// The contract's button of `name`, as the emulator's manifest names it.
fn button(name: &str) -> u32 {
    arcade::BUTTONS
        .iter()
        .find(|(named, _)| *named == name)
        .map(|(_, bit)| *bit)
        .unwrap_or_else(|| panic!("no button {name}"))
}

/// A while on the title, start, then the first choice taken a few times
/// and the stick held each way.
fn buttons() -> Vec<u32> {
    let [start, a, left, right] = ["start", "a", "left", "right"].map(button);
    (0..UPDATES)
        .map(|tick| match tick {
            300 => start,
            330 | 360 | 390 | 420 => a,
            500..560 => left,
            600..660 => right,
            _ => 0,
        })
        .collect()
}

#[test]
fn every_shipped_game_shows_its_cover_and_plays() {
    let roms = shipped();
    assert!(!roms.is_empty(), "no ROM in {ROMS}");
    let mut emulator = Emulator::hosted(arcade::RUNNER).expect("the runner is hosted");
    for (name, file) in roms {
        let covered = emulator.cover(&file, &name).unwrap();
        let title = covered.said.map(|said| said.title);
        assert!(
            title.is_some_and(|title| !title.is_empty()),
            "{name}: no cover with a title"
        );
        emulator.open(1, &file, 1, 0.0, false).unwrap();
        emulator.script(1, &buttons()).unwrap();
        // A step runs at most the updates a cart keeps the tones of.
        let mut ran = 0;
        while ran < UPDATES as u32 {
            let most = emulator.kept.min(UPDATES as u32 - ran);
            assert_eq!(emulator.step(1, most).unwrap(), most, "{name}");
            ran += most;
        }
        assert_eq!(emulator.stop(1).unwrap(), None, "{name} stopped");
        emulator.close(1).unwrap();
    }
}
