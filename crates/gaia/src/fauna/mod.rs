//! The animals of a world: as data, drawn as boxes, moved by code alone.
//!
//! A [`Kind`] is a table: a quadruped's [`Rig`], its [`Gait`]s with their
//! footfalls, its [`Coat`]s and marks, its builds, how its herd keeps
//! together and whether it carries a rider. The kinds a world has are
//! [`KINDS`], the horse first; another animal is another table and no new
//! code.
//!
//! - [`Look`]: one animal drawn from its kind and a seed.
//! - [`Animal`]: one animal's body as it is drawn: hooves planted on the
//!   ground it is handed, the body eased over them, its boxes for a frame,
//!   the saddle a rider sits in and the footfalls it struck.
//! - [`Wings`]: a pair of [`Wing`]s on an animal born with them, grown
//!   out in flight, beaten and folded away on landing.
//! - [`run`]: where an animal called is on its way to whoever called it;
//!   [`coming_s`], how long one called from afar is out of sight first.
//! - [`Region`]: where a herd may stand, the herd a world's seed gives it,
//!   and an animal's number, which names its region and place.
//! - [`Herd`]: where a herd's animals are at an instant, in closed form
//!   from a seed and a clock, so every client that asks at the same instant
//!   answers the same pose.
//!
//! It speaks animal and knows nothing of a world: the ground comes through
//! [`Ground`] and [`Land`], which whoever draws the animal answers. Its
//! sums are `libm`'s, so a herd is the same on every platform.

mod body;
mod call;
mod ease;
mod gait;
mod herd;
pub mod horse;
mod ik;
mod kind;
mod look;
mod random;
mod region;
mod rig;
mod wing;

use glam::DVec3;

pub use body::{Animal, Beat, Flank, Footfall, Glide, Going, Landing, Leap, LegPose};
pub use call::{ANSWERS_M, COMES_M, FASTEST_MPS, Run, coming_s};
pub use gait::{Gait, Gaits, Hoof};
pub use herd::{Herd, Land, Pose, grazing};
pub use kind::{
    Backing, BySize, Carry, Flight, Habitat, Herding, Jump, Kind, Pace, Spring, Winged,
};
pub use look::{Blot, Build, Coat, Face, Look, Marks, Patches, Traits};
pub use random::{Random, mix};
pub use region::{HOMES, PLACE_BITS, REGION_BITS, Region};
pub use rig::{Ears, LEGS, Leg, Neck, Part, Rig, Role, Section, Tail};
pub use wing::{Row, Wing, WingPaint, Wings};

/// What an animal stands on: the ground as whoever draws it knows it.
pub trait Ground {
    /// The ground under or over a world point, along the up there.
    fn under(&self, at: DVec3) -> Footing;
}

/// A point of the ground.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Footing {
    /// On the ground, world space.
    pub point: DVec3,
    /// Out of the ground, unit.
    pub up: DVec3,
    /// What the ground is, as whoever answers numbers it: carried to a
    /// footfall struck there, for the sound it makes.
    pub surface: u8,
}

/// Earshot: how far a pet hears its keeper's whistle, metres, as far as it
/// carries. Within it the pet runs in from where it is seen; past it, it
/// answers from out of sight, within [`ANSWERS_M`].
pub const HEARS_M: f64 = 150.0;

/// The most bytes of UTF-8 a pet's name has.
pub const NAME_BYTES: usize = 32;

/// A name as a pet keeps it: trimmed, never empty, no control character,
/// at most [`NAME_BYTES`].
pub fn named(name: &str) -> Option<String> {
    let name = name.trim();
    let fits = !name.is_empty() && name.len() <= NAME_BYTES;
    (fits && !name.chars().any(char::is_control)).then(|| name.to_owned())
}

/// Every kind of animal a world has, by its number.
pub const KINDS: [&Kind; 1] = [&horse::HORSE];
