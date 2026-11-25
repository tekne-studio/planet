//! The life of a world: what lives on it besides the bodies of people and
//! agents. A system of the core, switched per world by its admin, and whole
//! in one version: a better tree or a new animal is a new [`VERSION`].
//!
//! - [`flora`]: the plants, grown from the world's seed on the ground its
//!   generator makes.
//! - [`fauna`]: the animals, their kinds as tables, their bodies and herds.
//!
//! Its sums are `libm`'s, so what lives is the same on every platform. The
//! plants read the ground from the world's generator; the animals know
//! nothing of a world, and the ground comes to them through traits whoever
//! places them answers.

pub mod fauna;
pub mod flora;

/// What a world's statement calls the life of a world.
pub const NAME: &str = "gaia";

/// The version of everything that lives: a client of another draws no life
/// rather than a different one. Raised by any change that moves a plant or
/// an animal or changes how one grows: its blocks, its branches, its crown,
/// an animal's poses; the golden hashes in its tests say when. How a plant
/// is painted, its tones and its petals, is each client's.
pub const VERSION: u32 = 1;
