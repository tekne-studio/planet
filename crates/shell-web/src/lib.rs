//! The browser shell. What touches the browser lives behind `cfg(wasm32)`, so
//! `cargo test --workspace` stays native; the fingers, the games the
//! cartridges' worker knows and what it said are plain logic, built
//! everywhere so their tests run there.
//!
//! The JS side sees one class, [`web::Engine`], and talks to it through the
//! command/event seam as JSON (`client::Command`, `client::Event`), plus the
//! keys it presses and the stick the shell itself says.

#[cfg(target_arch = "wasm32")]
mod audio;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod board;
#[cfg(target_arch = "wasm32")]
mod carts;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod games;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod heard;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod keys;
#[cfg(target_arch = "wasm32")]
mod moving;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod touch;
#[cfg(target_arch = "wasm32")]
mod web;
