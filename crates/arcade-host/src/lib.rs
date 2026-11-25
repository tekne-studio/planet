//! The arcade emulator's native host, the desktop's and a Raspberry Pi's:
//! it gives the runner (`arcade::RUNNER`, the file every host runs) the
//! engine it asks for, on wasmi, and gives a shell the runner's calls as
//! plain Rust, the calls `host.js` gives a page. The runner judges
//! everything; the host compiles and calls the games, keeps their prepared
//! bytes on disk ([`Kept`]), and draws a screen by looking each pixel up in
//! the colours the runner resolved. What it lets a game and the runner take
//! is in one place, [`limits`].
//!
//! The emulator's cases hold it to every other host: each case its build
//! agreed on runs here and gives the same run (`tests/cases.rs`).

mod game;
mod host;
mod kept;
pub mod limits;

pub use host::{Broken, Covered, Emulator, Said, Screen, Text, Window};
pub use kept::Kept;
