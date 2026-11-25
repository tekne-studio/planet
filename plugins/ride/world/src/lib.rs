//! The ride plugin's world half. Riding is a way of moving a body, and a
//! body moves in the client: this half offers the world no op of its own.
//! A rider's way rides the core's stance, and the server relays it unread.
//!
//! What both halves of riding say once lives here: the plugin's name and
//! version, and the names of its ways of moving.

use world::{Op, Room, Who};

/// What the world's statement calls the plugin.
pub const NAME: &str = "ride";
/// The version of its seam.
pub const VERSION: u32 = 1;
/// The board: a plank that hovers over the ground and the sea.
pub const BOARD: &str = "board";
/// The horse: an animal of a world with life, ridden at its paces.
pub const HORSE: &str = "horse";

/// The plugin: it holds nothing of a world.
#[derive(Default)]
pub struct Ride;

/// The plugin as the server's module holds it.
pub fn plugin() -> Box<dyn world::Plugin> {
    Box::new(Ride)
}

impl world::Plugin for Ride {
    fn name(&self) -> &'static str {
        NAME
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn ops(&self) -> Vec<Op> {
        Vec::new()
    }

    fn op(&mut self, _kind: &str, _payload: &[u8], _who: &Who, _room: &mut dyn Room) {}

    fn gone(&mut self, _session: u32) {}
}
