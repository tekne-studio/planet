//! A cartridge in a window through the emulator's native host, the keys
//! its buttons. A run with no window is the arcade's.

mod window;

use arcade_host::Emulator;

use crate::args::CartRun;

pub fn run(run: CartRun) -> Result<(), String> {
    let file = std::fs::read(&run.file).map_err(|e| format!("{}: {e}", run.file.display()))?;
    let title = run.file.file_stem().map_or("cartridge".into(), |stem| {
        stem.to_string_lossy().into_owned()
    });
    let emulator = Emulator::hosted(arcade::RUNNER).map_err(|e| e.to_string())?;
    window::run(emulator, &file, run.seed, title, run.scale)
}
