//! A quadruped's rig as a table: four legs, a trunk, a neck, a head, ears
//! and a tail, at size 1 and build 1.
//!
//! The rig's own space is a body's: `x` right, `y` up, `z` back, the ground
//! under its middle at the origin. Its boxes are placed there, widened by
//! the build, then carried into the world by where it stands, scaled by its
//! size.

use glam::DVec3;

/// Legs in the order every array here keeps: left fore, right fore, left
/// hind, right hind.
pub const LEGS: usize = 4;

/// Which of an animal's colours a box takes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// The coat's hair.
    Hair,
    /// The muzzle: skin, or paler hair.
    Muzzle,
    /// An eye.
    Eye,
    /// A white mark: a star, a stripe, a blaze.
    Mark,
}

/// A box in its part's frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Part {
    pub centre: DVec3,
    pub size: DVec3,
    pub role: Role,
}

/// A leg as it hangs from the body at rest.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Leg {
    /// Where its upper bone meets the body: the elbow of a fore leg, the
    /// stifle of a hind one. For the left legs; the right mirror them.
    pub root: DVec3,
    /// Its foot at rest, on the ground.
    pub foot: DVec3,
    /// The upper bone, and from the knee or hock to the fetlock.
    pub upper_m: f64,
    pub lower_m: f64,
    /// Width and depth of the upper bone and of the lower.
    pub upper: [f64; 2],
    pub lower: [f64; 2],
    /// How much of a foot's travel the shoulder or the hip takes, swinging
    /// the root of the leg along: what keeps a fore leg straight under the
    /// body.
    pub sweep: f64,
}

/// A section of the trunk, front to back.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Section {
    /// Where it runs along the body, from and to; its bottom and top; its
    /// width.
    pub from_z: f64,
    pub to_z: f64,
    pub bottom: f64,
    pub top: f64,
    pub width: f64,
    /// Into how many boxes a coat in patches cuts it, along and up.
    pub cuts: [usize; 2],
}

/// The neck: a box from its root to the poll, the head's joint.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Neck {
    pub root: DVec3,
    pub length_m: f64,
    /// How far it runs back into the chest, so no gap opens as it nods.
    pub into_m: f64,
    pub width_m: f64,
    pub depth_m: f64,
    /// Into how many boxes a coat in patches cuts it.
    pub cuts: usize,
    /// The neck lowered to the grass, and the head to it, radians.
    pub graze_neck: f64,
    pub graze_head: f64,
}

/// The ears, a pair: the left one's place on the head, its box, how far it
/// lies back.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ears {
    pub at: DVec3,
    pub size: DVec3,
    pub back: f64,
}

/// The tail: a dock, then its hair.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tail {
    pub root: DVec3,
    pub dock_m: f64,
    pub dock_width_m: f64,
    /// The hair's width and depth; its length is the animal's own.
    pub hair: [f64; 2],
    /// How far the dock and the hair hang from straight back, radians.
    pub hang: f64,
    pub hair_hang: f64,
}

/// A quadruped, as a table.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Rig {
    /// The left fore and the left hind: the right mirror them.
    pub fore: Leg,
    pub hind: Leg,
    /// From the fetlock to the ground, standing: the pastern, then the foot.
    pub pastern_m: f64,
    pub hoof_m: f64,
    /// The foot's width and depth.
    pub hoof: [f64; 2],
    /// How far the pastern and foot fold back at most in the swing,
    /// radians, and how much more for each metre a foot lifts.
    pub fold: f64,
    pub fold_per_lift: f64,
    pub trunk: &'static [Section],
    /// What the body pitches and rolls about.
    pub pivot: DVec3,
    /// How much of the slope across it the body takes: it stands upright and
    /// lets its legs take the rest.
    pub roll_follow: f64,
    pub neck: Neck,
    /// The head's boxes, in the poll's frame, `-z` ahead.
    pub head: &'static [Part],
    pub ears: Ears,
    /// The left eye, in the poll's frame: the right mirrors it.
    pub eye: Part,
    pub tail: Tail,
}

impl Rig {
    /// A leg at rest, by its index.
    pub fn leg(&self, leg: usize) -> Leg {
        let left = match leg {
            0 | 1 => self.fore,
            _ => self.hind,
        };
        let side = if leg % 2 == 0 { 1.0 } else { -1.0 };
        let mirror = |p: DVec3| DVec3::new(p.x * side, p.y, p.z);
        Leg {
            root: mirror(left.root),
            foot: mirror(left.foot),
            ..left
        }
    }

    /// Whether a leg is a hind one.
    pub fn hind(leg: usize) -> bool {
        leg >= 2
    }

    /// How far a leg stands from fore to hind, and from left to right, at
    /// size 1 and build 1: what the slope under the body is read across.
    pub fn span(&self) -> (f64, f64) {
        (self.hind.foot.z - self.fore.foot.z, -2.0 * self.fore.foot.x)
    }
}
