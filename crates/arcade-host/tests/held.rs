//! A game held while its player is gone, then run on, as a cabinet's shell
//! does: held at its last tick, its clock at pace 0, and run on at pace 1
//! between two ticks. On a fixed clock it plays the same frames and tones
//! as the same game never left, a hold's length later: held, it runs no
//! update and plays nothing; run on, it makes no update up and keeps its
//! pace to the last tick.

mod support;

use std::collections::HashMap;

use arcade_host::Emulator;
use support::shipped;

/// A frame of a page drawing at 60 Hz, milliseconds.
const FRAME_MS: f64 = 1000.0 / 60.0;
/// The tick the game is held after: mid play, past its title.
const HELD_AT: u64 = 450;
/// How many ticks it is held for.
const HELD_FOR: u64 = 120;
/// Where between two ticks it is run on, of a frame: a shell runs it on as
/// its player sits again, at no tick of the game's.
const ON_BETWEEN: f64 = 0.37;
/// How many ticks it runs after: long enough for a drift to show.
const AFTER: u64 = 3000;

/// When tick `k` runs: half a frame past a whole one, so each tick finds
/// its update due by half of one, never on the edge.
fn tick_ms(k: u64) -> f64 {
    (k as f64 + 0.5) * FRAME_MS
}

/// The contract's button of `name`.
fn button(name: &str) -> u8 {
    arcade::BUTTONS
        .iter()
        .find(|(named, _)| *named == name)
        .map(|(_, bit)| *bit as u8)
        .unwrap_or_else(|| panic!("no button {name}"))
}

/// A while on the title, start, then played: the first choice taken now
/// and then, the stick swept each way, a byte an update, past every update
/// the test runs.
fn buttons() -> Vec<u8> {
    let [start, a, left, right] = ["start", "a", "left", "right"].map(button);
    (0..(HELD_AT + AFTER + HELD_FOR) * 2)
        .map(|update| match update {
            300 => start,
            _ if update > 300 && update % 45 == 0 => a,
            _ if update > 300 && update % 240 < 40 => left,
            _ if update > 300 && update % 240 >= 120 && update % 240 < 160 => right,
            _ => 0,
        })
        .collect()
}

/// What a run of one game showed and played: its screen after each tick,
/// by how many updates it had run, and each update's tones, in order.
#[derive(Default)]
struct Run {
    frames: HashMap<u64, Vec<u8>>,
    tones: Vec<Vec<u32>>,
}

impl Run {
    /// Takes what `id` showed and played since the last tick: how many
    /// updates it ran.
    fn take(&mut self, emulator: &mut Emulator, id: u32) -> u64 {
        let batch = emulator.sound(id).unwrap();
        let ran = emulator.tones(&batch).len() as u64;
        self.tones
            .extend(emulator.tones(&batch).into_iter().map(<[u32]>::to_vec));
        let updates = emulator.updates(id).unwrap().expect("it runs");
        self.frames
            .insert(updates, emulator.rgba(id).unwrap().unwrap());
        ran
    }
}

#[test]
fn a_game_held_then_run_on_plays_as_one_never_left_a_hold_later() {
    let roms = shipped();
    assert!(!roms.is_empty());
    for (name, file) in roms {
        let mut emulator = Emulator::hosted(arcade::RUNNER).unwrap();
        // Never left, and held: the same game from the same seed, at once.
        let (never, held) = (1, 2);
        for id in [never, held] {
            emulator.open(id, &file, 7, 0.0, false).unwrap();
            emulator.script(id, &buttons()).unwrap();
        }
        let mut runs = [Run::default(), Run::default()];
        let mut k = 1;
        while k <= HELD_AT {
            emulator.tick(tick_ms(k)).unwrap();
            runs[0].take(&mut emulator, never);
            assert_eq!(runs[1].take(&mut emulator, held), 1, "{name}: one a tick");
            k += 1;
        }
        // Held as its last tick left it.
        emulator.pace(held, 0.0, tick_ms(HELD_AT)).unwrap();
        let at = emulator.updates(held).unwrap().unwrap();
        while k <= HELD_AT + HELD_FOR {
            let changed = emulator.tick(tick_ms(k)).unwrap();
            assert!(!changed.contains(&held), "{name}: its screen holds");
            runs[0].take(&mut emulator, never);
            assert_eq!(runs[1].take(&mut emulator, held), 0, "{name}: held");
            k += 1;
        }
        assert_eq!(emulator.updates(held).unwrap(), Some(at));
        // Run on between two ticks, as its player sits again.
        let on_ms = tick_ms(k - 1) + ON_BETWEEN * FRAME_MS;
        emulator.pace(held, 1.0, on_ms).unwrap();
        while k <= HELD_AT + HELD_FOR + AFTER {
            emulator.tick(tick_ms(k)).unwrap();
            runs[0].take(&mut emulator, never);
            // None made up, and the pace kept: one update a tick, a hold's
            // length behind the game never left.
            assert_eq!(runs[1].take(&mut emulator, held), 1, "{name}: tick {k}");
            assert_eq!(
                emulator.updates(held).unwrap(),
                Some(k - HELD_FOR),
                "{name}"
            );
            k += 1;
        }
        let [never_run, held_run] = runs;
        assert_eq!(
            held_run.tones,
            never_run.tones[..held_run.tones.len()],
            "{name}: the same tones, update by update"
        );
        assert!(
            held_run.tones.iter().any(|tones| !tones.is_empty()),
            "{name}: it played"
        );
        for (updates, frame) in &held_run.frames {
            assert!(
                never_run.frames.get(updates) == Some(frame),
                "{name}: the screen after {updates} updates"
            );
        }
        let shown: std::collections::HashSet<&Vec<u8>> = held_run.frames.values().collect();
        assert!(shown.len() > 100, "{name}: its screen moved");
    }
}
