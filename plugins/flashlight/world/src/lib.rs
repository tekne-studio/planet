//! The flashlight plugin's world half. The flashlight is a thing in a
//! body's hand, and a hand is said with the body's stance, which the server relays
//! unread: this half offers the world no op and keeps nothing.
//!
//! What both halves of the flashlight say once lives here: the plugin's
//! name and version, and the name of the thing it gives a hand.

use world::{Op, Room, Who};

/// What the world's statement calls the plugin.
pub const NAME: &str = "flashlight";
/// The version of its seam.
pub const VERSION: u32 = 1;
/// The flashlight as a thing in the hand, the name after the dot: what
/// the body's hand grasps while it is on.
pub const THING: &str = "flashlight";

/// The plugin: it holds nothing of a world.
#[derive(Default)]
pub struct Flashlight;

/// The plugin as the server's module holds it.
pub fn plugin() -> Box<dyn world::Plugin> {
    Box::new(Flashlight)
}

impl world::Plugin for Flashlight {
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
