//! Every case of the pinned emulator runs through this host and gives the
//! run every other host gave when the emulator was built: the same verdict
//! of its check on the file, the same frames, the same tones, the same stop
//! at the same update, the same last screen in colour. A case this host
//! cannot run is a failure, never a skip.

use arcade_host::Emulator;
use serde_json::Value;

const ID: u32 = 1;
/// FNV-1a over 32 bits, from its offset basis.
const FNV: u32 = 0x811c_9dc5;

fn fnv(hash: u32, bytes: &[u8]) -> u32 {
    bytes.iter().fold(hash, |hash, &byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// A case's file, seed and buttons, read from its JSON, and what every
/// host gave: the verdict of its check, empty when it keeps the file, and
/// the run.
struct Case {
    test: String,
    file: Vec<u8>,
    seed: u32,
    buttons: Vec<u32>,
    check: String,
    run: String,
}

impl Case {
    fn read(case: &Value) -> Option<Case> {
        let hex = case["file"].as_str()?;
        let file = (0..hex.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
            .collect::<Option<_>>()?;
        let mut buttons = Vec::new();
        for held in case["buttons"].as_array()? {
            let [value, count] = held.as_array()?.as_slice() else {
                return None;
            };
            let value = u32::try_from(value.as_u64()?).ok()?;
            buttons.extend(std::iter::repeat_n(value, count.as_u64()? as usize));
        }
        let run = &case["run"];
        let run = format!(
            "{} {} {} {} {}",
            run["frames"].as_str()?,
            run["tones"].as_str()?,
            run["stop"].as_str()?,
            run["updates"].as_u64()?,
            run["rgba"].as_str()?
        );
        Some(Case {
            test: case["test"].as_str()?.to_owned(),
            file,
            seed: u32::try_from(case["seed"].as_u64()?).ok()?,
            buttons,
            check: case["check"].as_str()?.to_owned(),
            run,
        })
    }
}

/// `case` run through the host as every host runs it: an update at a
/// time, each update's tones and screen hashed, as the line the case keeps.
fn run(emulator: &mut Emulator, case: &Case) -> String {
    emulator
        .open(ID, &case.file, case.seed, 0.0, false)
        .unwrap();
    emulator.script(ID, &case.buttons).unwrap();
    let (mut frames, mut tones) = (FNV, FNV);
    for _ in &case.buttons {
        if emulator.stop(ID).unwrap().is_some() {
            break;
        }
        emulator.step(ID, 1).unwrap();
        let batch = emulator.sound(ID).unwrap();
        for word in emulator.tones(&batch).concat() {
            tones = fnv(tones, &word.to_le_bytes());
        }
        if emulator.stop(ID).unwrap().is_none() {
            let read = emulator.read(ID).unwrap().expect("a cart is there");
            frames = fnv(fnv(frames, &read.pixels), read.palette.as_flattened());
        }
    }
    let rgba = fnv(FNV, &emulator.rgba(ID).unwrap().expect("a cart is there"));
    let stop = emulator.stop(ID).unwrap();
    let updates = emulator.updates(ID).unwrap().expect("a cart is there");
    emulator.close(ID).unwrap();
    format!(
        "{frames:08x} {tones:08x} {} {updates} {rgba:08x}",
        stop.as_deref().unwrap_or("running")
    )
}

#[test]
fn every_case_runs_here_as_on_every_host() {
    let cases: Value = serde_json::from_slice(arcade::CASES).expect("the cases are JSON");
    let cases = cases.as_array().expect("the cases are a list");
    assert!(!cases.is_empty(), "the pinned emulator holds no case");
    let mut emulator = Emulator::hosted(arcade::RUNNER).expect("the runner is hosted");
    let mut failed = Vec::new();
    for (at, case) in cases.iter().enumerate() {
        let case = Case::read(case).unwrap_or_else(|| panic!("case {at} reads"));
        let check = emulator.check(&case.file).unwrap().unwrap_or_default();
        if check != case.check {
            failed.push(format!(
                "{} (case {at}): checked {check:?}, every host {:?}",
                case.test, case.check
            ));
        }
        let ran = run(&mut emulator, &case);
        if ran != case.run {
            failed.push(format!(
                "{} (case {at}): ran {ran}, every host {}",
                case.test, case.run
            ));
        }
    }
    assert!(
        failed.is_empty(),
        "{} cases differ:\n{}",
        failed.len(),
        failed.join("\n")
    );
}
