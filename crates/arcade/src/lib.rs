//! The arcade's emulator as the engine carries it: the console's build,
//! pinned in `arcade/emulator/` as files named by their hash, never built here.
//! Its manifest, `emulator.json`, is read when this crate is built
//! (`build.rs`), so what a host needs before it runs the runner is a
//! constant: the versions it speaks, a screen's size, the buttons and the
//! keys that play them, and what a host holds for a game. A manifest
//! missing one of them is a build that fails, naming it.
//!
//! The runner's bytes ([`RUNNER`]) and the cases every host is held to
//! ([`CASES`]) are the pinned files themselves: the native host runs the
//! same bytes the browser's does.

include!(concat!(env!("OUT_DIR"), "/emulator.rs"));
