//! A figure: one body as it is drawn. A VRM avatar moved by locomotion clips,
//! or the box figure while nothing is loaded.
//!
//! One figure per body, the player's and every peer's, over one shared set of
//! clips. What a figure needs to pose itself is a [`Motion`]: the local
//! controller produces one, and a peer's stance interpolates into one, so the
//! figure never knows which body it is.

use std::collections::HashMap;
use std::sync::Arc;

use avatar::{Animator, Avatar, Clip};
use glam::{DAffine3, DMat3, DQuat, DVec3, Quat};
use scene::{SkinnedInstance, SkinnedMeshId};

use crate::controller::Controller;
use crate::seam::Mode;
use crate::way::{Held, Straddle};

/// How the body moves right now. Each gait is one clip.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Gait {
    Idle,
    Walk,
    Run,
    Jump,
    Fall,
    Fly,
    /// At the surface or under it. Until a swim clip is in the manifest the
    /// fly clip stands in.
    Swim,
}

impl Gait {
    /// The speed the clip was authored for, metres per second. Playback
    /// scales with the real speed: feet do not slide, and a sprint looks like one.
    fn authored_mps(self) -> Option<f64> {
        match self {
            Gait::Walk => Some(1.5),
            Gait::Run => Some(3.4),
            Gait::Swim => Some(2.4),
            _ => None,
        }
    }

    /// On the ground, where legs swing and feet stay put.
    pub fn grounded(self) -> bool {
        matches!(self, Gait::Idle | Gait::Walk | Gait::Run)
    }

    pub fn of(controller: &Controller) -> Gait {
        // Taken off on wings, it flies from the press: the spring off its
        // legs is the flight's start, and a peer draws both from when its
        // gait turned.
        if controller.flying() || controller.aloft() {
            Gait::Fly
        } else if controller.way().is_some() && controller.leaping() {
            // A jump on what it rides, from the press: a peer draws the
            // gathering from when its gait turned, and the air from where
            // it is.
            Gait::Jump
        } else if controller.way().is_some() {
            // A body in a way of moving holds the way's pose over the idle
            // clip, and sends that it stands.
            Gait::Idle
        } else if controller.swimming() {
            Gait::Swim
        } else if !controller.grounded() {
            if controller.vertical_mps() > 0.0 {
                Gait::Jump
            } else {
                Gait::Fall
            }
        } else if controller.ground_mps() < 0.1 {
            Gait::Idle
        } else if controller.ground_mps() < 2.2 {
            Gait::Walk
        } else {
            Gait::Run
        }
    }

    /// The wire's name for it.
    pub fn wire(self) -> protocol::Gait {
        match self {
            Gait::Idle => protocol::Gait::Idle,
            Gait::Walk => protocol::Gait::Walk,
            Gait::Run => protocol::Gait::Run,
            Gait::Jump => protocol::Gait::Jump,
            Gait::Fall => protocol::Gait::Fall,
            Gait::Fly => protocol::Gait::Fly,
            Gait::Swim => protocol::Gait::Swim,
        }
    }

    pub fn from_wire(gait: protocol::Gait) -> Gait {
        match gait {
            protocol::Gait::Idle => Gait::Idle,
            protocol::Gait::Walk => Gait::Walk,
            protocol::Gait::Run => Gait::Run,
            protocol::Gait::Jump => Gait::Jump,
            protocol::Gait::Fall => Gait::Fall,
            protocol::Gait::Fly => Gait::Fly,
            protocol::Gait::Swim => Gait::Swim,
        }
    }
}

/// One of the figure's own clips asked for over what the body moves by, by
/// a plugin or at the seam: `{"type":"play","clip":"fall","for_s":0.45}`.
/// Peers see it as the body's gait while it plays; then the body plays
/// what it moves by again. The figure fades into it and out of it as into
/// any clip.
///
/// It ends sooner when the body starts to move otherwise than when it was
/// asked, a passage takes it, the world changes, or the plugin that asked
/// rests.
#[derive(Clone, Copy, PartialEq, Debug, serde::Deserialize)]
pub struct Play {
    /// The clip, by its gait: `idle`, `walk`, `run`, `jump`, `fall`, `fly`
    /// or `swim`.
    pub clip: Gait,
    /// How long it plays, seconds, ten at most; left out, once through.
    #[serde(default)]
    pub for_s: Option<f64>,
    /// Whether moving ends it sooner: a wave does, a fall into a way of
    /// moving does not.
    #[serde(default)]
    pub until_moved: bool,
}

/// The longest a clip asked for plays, seconds: a gesture, never a state of
/// the body. Longer is asked again.
pub(crate) const LONGEST_S: f64 = 10.0;

/// A clip not loaded plays once through in this long, seconds.
const ONCE_S: f64 = 1.0;

/// A clip asked for, playing since a second of the clock.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Playing {
    play: Play,
    since_s: f64,
    /// The plugin that asked, or none at the seam.
    pub(crate) by: Option<&'static str>,
    /// How the body moved when it was asked.
    pub(crate) mode: Mode,
}

impl Playing {
    /// `play`, asked by `by` at `now_s` of a body moving as `mode`. None
    /// for a length that is no length of time a clip plays.
    pub(crate) fn new(
        play: Play,
        now_s: f64,
        by: Option<&'static str>,
        mode: Mode,
    ) -> Option<Playing> {
        if play
            .for_s
            .is_some_and(|for_s| !(0.0..=LONGEST_S).contains(&for_s))
        {
            return None;
        }
        Some(Playing {
            play,
            since_s: now_s,
            by,
            mode,
        })
    }

    /// The clip it plays.
    pub(crate) fn clip(&self) -> Gait {
        self.play.clip
    }

    /// Whether it still plays at `now_s`: for as long as it was asked, or
    /// once through the clip, and only while the body is still if moving
    /// ends it.
    pub(crate) fn plays(&self, now_s: f64, clips: &Clips, moved: bool) -> bool {
        let length_s = self.play.for_s.unwrap_or_else(|| {
            clips
                .get(&self.play.clip)
                .map_or(ONCE_S, |clip| f64::from(clip.duration_s))
        });
        now_s - self.since_s < length_s && !(moved && self.play.until_moved)
    }
}

/// Clearance between the top of a body and the label hung over it.
const LABEL_M: f64 = 0.1;

/// What a figure needs to stand and move: where the body is, which way it
/// faces and stands, what it is doing and how fast.
#[derive(Clone, Copy, Debug)]
pub struct Motion {
    /// Feet, world space.
    pub position: DVec3,
    /// The body's frame, columns right, up, back.
    pub basis: DMat3,
    pub gait: Gait,
    /// What the clip plays at: along the ground on foot, through the water
    /// when swimming, through the air in flight.
    pub speed_mps: f64,
    /// Running on foot, flying flat out.
    pub sprint: bool,
    /// Distance walked, for the walk cycle of the box figure.
    pub stride_m: f64,
}

impl Motion {
    /// How `controller`'s body moves, shown in `gait`: its own, or a clip
    /// played over it, which plays at the speed that gait follows.
    pub fn of(controller: &Controller, gait: Gait) -> Motion {
        Motion {
            position: controller.drawn_position(),
            basis: controller.body_basis(),
            gait,
            speed_mps: speed_for(gait, controller.ground_mps(), controller.speed_mps()),
            sprint: controller.sprinting(),
            stride_m: controller.stride_m(),
        }
    }
}

/// The speed a clip of this gait follows: swimmers and flyers move in three
/// dimensions, so a dive is speed too; on foot only the ground counts.
fn speed_for(gait: Gait, ground_mps: f64, speed_mps: f64) -> f64 {
    match gait {
        Gait::Swim | Gait::Fly => speed_mps,
        _ => ground_mps,
    }
}

/// A body's frame from its up and the way it faces: columns right, up, back.
pub fn basis(up: DVec3, facing: DVec3) -> DMat3 {
    let flat = facing - up * facing.dot(up);
    let back = -flat.normalize_or(up.any_orthonormal_vector());
    DMat3::from_cols(up.cross(back), up, back)
}

/// The clips every figure shares, one per gait, retargeted at load.
pub type Clips = HashMap<Gait, Clip>;

const LEAN_CRUISE: f64 = 0.35;
const LEAN_SPRINT: f64 = core::f64::consts::FRAC_PI_4;
const LEAN_SWIM: f64 = 1.25;
const LEAN_PER_S: f64 = 5.0;
/// How fast a way's pose comes on as the body takes the way, and goes as
/// it leaves, per second.
const HOLD_PER_S: f64 = 9.0;

/// How far over what a worn avatar sits on its hip joints are, as a share
/// of their height at rest: where its seat is.
const SEAT: f64 = 0.12;

/// How long a leap between its feet and the saddle takes, seconds; how
/// high over the straight way between them it springs, metres; how far
/// through a leap on it springs on the jump clip before its legs come round
/// the back; and how far through a leap off it lands on its feet, the jump
/// clip's second half played by then.
const LEAP_S: f64 = 0.6;
const LEAP_OVER_M: f64 = 0.6;
const LEAP_SPRING: f64 = 0.35;
const LEAP_LANDS: f64 = 0.85;

/// A figure leaping between its feet and a seat: where it sprang from,
/// whether onto the seat or off it, and how far along, 0 to 1.
#[derive(Clone, Copy, Debug)]
struct Leap {
    from: DAffine3,
    onto: bool,
    done: f64,
}

impl Leap {
    /// How far along the way from where it sprang it is drawn, 0 to 1,
    /// eased: a leap off is on its feet as it lands.
    fn along(&self) -> f64 {
        let done = match self.onto {
            true => self.done,
            false => (self.done / LEAP_LANDS).min(1.0),
        };
        done * done * (3.0 - 2.0 * done)
    }
}

/// An avatar on a body: the mesh the renderer holds, and the skeleton that
/// poses it. Shared by every body wearing the same file.
#[derive(Clone)]
pub struct Worn {
    pub mesh: SkinnedMeshId,
    pub avatar: Arc<Avatar>,
}

pub struct Figure {
    worn: Option<Worn>,
    animator: Animator<Gait>,
    /// Forward lean in flight, radians, eased toward its target.
    lean: f64,
    /// The pose of the way of moving the body is in, kept while it eases
    /// out after the body leaves, and how far the figure holds it, 0 to 1.
    held: Option<Held>,
    holding: f64,
    /// What it sits astride, metres, and the pose it holds with its legs
    /// round it.
    straddle: Option<Straddle>,
    fitted: Option<Held>,
    /// The arm that holds a thing in the hand, laid over all the rest, kept
    /// while it eases out after the thing is put down, and how far the
    /// figure holds it, 0 to 1; and the arm as held, swung out clear of
    /// what it sits astride.
    arm: Vec<(&'static str, Quat)>,
    grasping: f64,
    reached: Vec<(&'static str, Quat)>,
    /// How far out its right wrist keeps astride, metres, as fitted.
    hand_out_m: Option<f64>,
    /// Where it was drawn on its feet the frame before and this one: what
    /// a leap into the saddle springs from; and where it was drawn seated,
    /// what a leap off it springs from.
    stood: [Option<DAffine3>; 2],
    sat: Option<DAffine3>,
    leap: Option<Leap>,
    /// Where it came down on its feet this frame from a leap off, if it did.
    alighted: Option<DVec3>,
}

impl Default for Figure {
    fn default() -> Figure {
        Figure {
            worn: None,
            animator: Animator::new(Gait::Idle),
            lean: 0.0,
            held: None,
            holding: 0.0,
            straddle: None,
            fitted: None,
            arm: Vec::new(),
            grasping: 0.0,
            reached: Vec::new(),
            hand_out_m: None,
            stood: [None; 2],
            sat: None,
            leap: None,
            alighted: None,
        }
    }
}

impl Figure {
    pub fn wear(&mut self, worn: Worn) {
        self.worn = Some(worn);
        self.fit();
    }

    /// Sits it astride `straddle` on `saddle`, or on nothing: fitted only
    /// when that changes, so a frame on the same back costs a comparison.
    /// From its feet onto a back, it leaps there from where it stood before;
    /// off one, it leaps down from where it sat.
    pub fn straddle(&mut self, straddle: Option<&Straddle>, saddle: Option<DAffine3>) {
        match (self.straddle.is_some(), straddle.is_some()) {
            (false, true) => {
                self.leap = self.stood[0].map(|from| Leap {
                    from,
                    onto: true,
                    done: 0.0,
                });
            }
            (true, false) => {
                self.leap = self.sat.map(|from| Leap {
                    from,
                    onto: false,
                    done: 0.0,
                });
            }
            _ => {}
        }
        if self.straddle.as_ref() != straddle {
            self.straddle = straddle.cloned();
            self.fit();
        }
        let seat = straddle.and(saddle).map(|saddle| self.seat(saddle));
        self.sat = seat.map(|seat| self.drawn(Some(seat), seat));
    }

    /// Where it sits on `saddle`: its hips over it as far as its seat is
    /// under them in the pose it holds.
    pub fn seat(&self, saddle: DAffine3) -> DAffine3 {
        let hips_m = self.hips_m().unwrap_or(0.0) - self.seat_m();
        DAffine3::from_mat3_translation(
            saddle.matrix3,
            saddle.translation - saddle.matrix3.y_axis * hips_m,
        )
    }

    /// The pose it holds, its legs round what it sits astride, and the arm
    /// that holds a thing out past its side.
    fn fit(&mut self) {
        let rest_m = self.hips_rest_m();
        let seat = (self.seat_m() / rest_m) as f32;
        let straddle = (self.straddle.as_ref()).map(|straddle| straddle.in_units(rest_m as f32));
        self.fitted = match (&self.held, &straddle) {
            (Some(held), Some(straddle)) => Some(held.astride(straddle, seat)),
            (held, _) => held.clone(),
        };
        self.reached = match &straddle {
            Some(straddle) => straddle.reach(&self.arm),
            None => self.arm.clone(),
        };
        self.hand_out_m =
            (straddle.as_ref()).map(|straddle| f64::from(straddle.hand_out()) * rest_m);
    }

    /// How far out from the middle its right wrist keeps astride, metres:
    /// what the box figure, which has no arm to swing, holds a thing at.
    /// None on nothing.
    pub fn hand_out_m(&self) -> Option<f64> {
        self.hand_out_m
    }

    /// How high its hips stand at rest, metres.
    fn hips_rest_m(&self) -> f64 {
        self.worn.as_ref().map_or(crate::box_figure::LEG_M, |worn| {
            f64::from(worn.avatar.hips_height_m())
        })
    }

    /// How far over what it sits on its hips are, metres: a body's seat is
    /// under its hip joints, and the box figure has none.
    pub fn seat_m(&self) -> f64 {
        match &self.worn {
            Some(worn) => SEAT * f64::from(worn.avatar.hips_height_m()),
            None => 0.0,
        }
    }

    /// The mesh worn, if any.
    pub fn mesh(&self) -> Option<SkinnedMeshId> {
        self.worn.as_ref().map(|worn| worn.mesh)
    }

    /// Where a label hangs, metres above the feet: over the top of the
    /// avatar worn, whatever its height, or of the box figure before one lands.
    pub fn label_m(&self) -> f64 {
        let top = self.worn.as_ref().map_or(crate::box_figure::TOP_M, |worn| {
            f64::from(worn.avatar.height_m())
        });
        top + LABEL_M
    }

    /// Plays the body's motion for `dt`, in the way of moving whose pose is
    /// `held`, if it is in one.
    pub fn update(&mut self, dt: f64, motion: &Motion, clips: &Clips, held: Option<&Held>) {
        let feet = DAffine3::from_mat3_translation(motion.basis, motion.position);
        self.stood = [self.stood[1], self.straddle.is_none().then_some(feet)];
        // Off the saddle, it comes down through the jump clip from its
        // middle: the spring was the saddle's.
        if let Some(Leap {
            onto: false, done, ..
        }) = self.leap
            && done == 0.0
            && let Some(jump) = clips.get(&Gait::Jump)
        {
            self.animator.cut_to(Gait::Jump, jump.duration_s / 2.0);
        }
        self.alighted = None;
        if let Some(leap) = &mut self.leap {
            let was = leap.done;
            leap.done += dt / LEAP_S;
            if !leap.onto && was < LEAP_LANDS && leap.done >= LEAP_LANDS {
                self.alighted = Some(motion.position);
            }
        }
        self.leap = self.leap.filter(|leap| leap.done < 1.0);
        let springs = self.leap.is_some_and(|leap| match leap.onto {
            true => leap.done < LEAP_SPRING,
            false => leap.done < LEAP_LANDS,
        });
        // In a way, the pose is held in flight and in a jump too: a rider
        // on what it rides sits, and only says it flies or jumps. Leaping
        // into the saddle, it springs first; off it, it is in the air until
        // it lands.
        let gait = match (motion.gait, held) {
            _ if springs => Gait::Jump,
            (Gait::Fly | Gait::Jump, Some(_)) => Gait::Idle,
            (gait, _) => gait,
        };
        let speed = gait.authored_mps().map_or(1.0, |authored| {
            (motion.speed_mps / authored).clamp(0.6, 2.2)
        });
        self.animator.update(dt as f32, gait, speed as f32);

        // Superman leans into the flight: a little when cruising, 45 degrees
        // at a sprint, upright when hovering. Eased, so Shift reads as a dive
        // into speed and not a snap.
        let moving = motion.speed_mps > 0.1;
        let target = match (gait, moving, motion.sprint) {
            (Gait::Fly, true, true) => LEAN_SPRINT,
            (Gait::Fly, true, false) => LEAN_CRUISE,
            // A swim clip lies in the water by itself. The fly clip standing
            // in for it is upright, so the body leans instead.
            (Gait::Swim, true, _) if !clips.contains_key(&Gait::Swim) => LEAN_SWIM,
            _ => 0.0,
        };
        self.lean += (target - self.lean) * (1.0 - (-LEAN_PER_S * dt).exp());

        // A way's pose holds over the idle clip its way plays: it comes on
        // as the body takes the way, and goes as it leaves, kept until it
        // has gone. Any other clip, asked for over the way, plays alone.
        if let Some(held) = held
            && self.held.as_ref() != Some(held)
        {
            self.held = Some(held.clone());
            self.fit();
        }
        let target = if held.is_some() && gait == Gait::Idle {
            1.0
        } else {
            0.0
        };
        self.holding += (target - self.holding) * (1.0 - (-HOLD_PER_S * dt).exp());
        if held.is_none() && self.holding < 1e-3 {
            self.held = None;
            self.holding = 0.0;
            self.fitted = None;
        }
    }

    /// Holds a thing in its hand with this arm for `dt`, or puts it down:
    /// the arm comes on and goes as a way's pose does, over whatever else
    /// poses the body.
    pub fn grasp(&mut self, dt: f64, arm: Option<&[(&'static str, Quat)]>) {
        if let Some(arm) = arm
            && self.arm != arm
        {
            self.arm.clear();
            self.arm.extend_from_slice(arm);
            self.fit();
        }
        let target = if arm.is_some() { 1.0 } else { 0.0 };
        self.grasping += (target - self.grasping) * (1.0 - (-HOLD_PER_S * dt).exp());
        if arm.is_none() && self.grasping < 1e-3 {
            self.arm.clear();
            self.reached.clear();
            self.grasping = 0.0;
        }
    }

    /// Whether the hand holds what it was given: once the arm is more than
    /// half way to it.
    pub fn grasped(&self) -> bool {
        self.grasping > 0.5
    }

    /// How high its hips stand over its feet in the pose it holds, metres:
    /// the avatar's worn, or the box figure's. None holding no pose.
    pub fn hips_m(&self) -> Option<f64> {
        let held = self.fitted.as_ref()?;
        Some(match &self.worn {
            Some(worn) => f64::from(held.hips.y * worn.avatar.hips_height_m()),
            None => crate::box_figure::hips_m(held),
        })
    }

    /// Where it is drawn: on `seat` when it sits on one, else on its
    /// `feet`; along an arc from where it sprang while it leaps between
    /// them, turning as it goes.
    pub fn drawn(&self, seat: Option<DAffine3>, feet: DAffine3) -> DAffine3 {
        let to = seat.unwrap_or(feet);
        let Some(leap) = self.leap.filter(|leap| leap.onto == seat.is_some()) else {
            return to;
        };
        let (from, along) = (leap.from, leap.along());
        let up = from.matrix3.y_axis.normalize_or(to.matrix3.y_axis);
        let at = from.translation.lerp(to.translation, along)
            + up * (LEAP_OVER_M * 4.0 * along * (1.0 - along));
        let turn = DQuat::from_mat3(&from.matrix3).slerp(DQuat::from_mat3(&to.matrix3), along);
        DAffine3::from_mat3_translation(DMat3::from_quat(turn), at)
    }

    /// Where it came down on its feet this frame from a leap off a back,
    /// world space: what a landing is heard at.
    pub fn alighted(&self) -> Option<DVec3> {
        self.alighted
    }

    /// The way's pose the figure holds, as far as boxes can show it: whole,
    /// once it holds it more than half.
    pub fn held(&self) -> Option<&Held> {
        self.fitted.as_ref().filter(|_| self.holding > 0.5)
    }

    /// The posed avatar, when one is worn, holding its way's pose over what
    /// it plays as far as it holds it, and the arm over both; and the frame
    /// a thing in its hand is held in: the body's axes, leaning as it
    /// leans, at its right wrist, world space.
    pub fn instance(&self, motion: &Motion, clips: &Clips) -> Option<(SkinnedInstance, DAffine3)> {
        let worn = self.worn.as_ref()?;
        // VRM 0.x looks down its `-Z`, the same way the body frame does.
        let mut transform = DAffine3::from_mat3_translation(motion.basis, motion.position);
        if self.lean > 1e-4 {
            let hips = DVec3::new(0.0, 0.9, 0.0);
            transform = transform
                * DAffine3::from_translation(hips)
                * DAffine3::from_quat(DQuat::from_rotation_x(-self.lean))
                * DAffine3::from_translation(-hips);
        }
        let pose = self.animator.pose(|gait| {
            let stand_in = (gait == Gait::Swim)
                .then(|| clips.get(&Gait::Fly))
                .flatten();
            clips.get(&gait).or(stand_in)
        });
        // Clips arrive after the avatar over a network: until then, the rest pose.
        let mut pose = pose.unwrap_or_else(avatar::Pose::rest);
        if let Some(held) = &self.fitted {
            let holding = self.holding as f32;
            for (bone, rotation) in &held.bones {
                pose.set_toward(bone, *rotation, holding);
            }
            pose.hips_toward(held.hips, holding);
        }
        for (bone, rotation) in &self.reached {
            pose.set_toward(bone, *rotation, self.grasping as f32);
        }
        let (joints, wrist) = worn.avatar.posed(&pose, crate::hand::BONE);
        let instance = SkinnedInstance {
            mesh: worn.mesh,
            transform,
            joints,
            shine: 0.0,
            shadow_only: false,
        };
        let hand = DAffine3::from_mat3_translation(
            transform.matrix3,
            transform.transform_point3(wrist.as_dvec3()),
        );
        Some((instance, hand))
    }
}

#[cfg(test)]
mod tests {
    use glam::{Quat, Vec3};

    use super::*;

    fn moving(gait: Gait) -> Motion {
        Motion {
            position: DVec3::ZERO,
            basis: DMat3::IDENTITY,
            gait,
            speed_mps: 0.0,
            sprint: false,
            stride_m: 0.0,
        }
    }

    #[test]
    fn a_way_s_pose_eases_in_as_the_body_takes_it_and_out_as_it_leaves() {
        let held = Held {
            bones: vec![("hips", Quat::from_rotation_y(1.0))],
            hips: Vec3::new(0.0, 0.8, 0.0),
        };
        let (clips, dt) = (Clips::default(), 1.0 / 60.0);
        let mut figure = Figure::default();
        // In the way, the pose comes on within a few tenths of a second,
        // not at once.
        figure.update(dt, &moving(Gait::Idle), &clips, Some(&held));
        assert_eq!(figure.held(), None);
        assert!(
            figure.holding > 0.0 && figure.holding < 0.3,
            "{}",
            figure.holding
        );
        for _ in 0..30 {
            figure.update(dt, &moving(Gait::Idle), &clips, Some(&held));
        }
        assert_eq!(figure.held(), Some(&held));
        // Hopped off, it is kept while it goes, and gone soon after.
        let mut frames = 0;
        while figure.held().is_some() {
            figure.update(dt, &moving(Gait::Jump), &clips, None);
            frames += 1;
            assert!(frames < 30);
        }
        assert!(frames > 2, "{frames}");
        for _ in 0..60 {
            figure.update(dt, &moving(Gait::Idle), &clips, None);
        }
        assert!(figure.held.is_none());
    }

    /// From its feet onto a back, it springs up and over into the seat in
    /// under a second, its legs round the back by then; a frame astride
    /// never springs it again.
    #[test]
    fn it_leaps_into_the_saddle_from_where_it_stood() {
        let held = Held {
            bones: vec![("hips", Quat::from_rotation_y(1.0))],
            hips: Vec3::new(0.0, 0.8, 0.0),
        };
        let (clips, dt) = (Clips::default(), 1.0 / 60.0);
        let mut figure = Figure::default();
        let stood = DVec3::new(2.0, 0.0, 0.0);
        let seat = DAffine3::from_translation(DVec3::new(0.0, 1.2, 0.0));
        let on_foot = Motion {
            position: stood,
            ..moving(Gait::Idle)
        };
        figure.update(dt, &on_foot, &clips, None);
        figure.straddle(None, None);
        // Mounted, the body is under the seat by the next frame.
        let mounted = Motion {
            position: DVec3::ZERO,
            ..moving(Gait::Idle)
        };
        figure.update(dt, &mounted, &clips, Some(&held));
        figure.straddle(Some(&Straddle { steps: Vec::new() }), Some(seat));
        assert!((figure.drawn(Some(seat), seat).translation - stood).length() < 1e-9);
        let mut highest = f64::MIN;
        let mut frames = 0;
        while (figure.drawn(Some(seat), seat).translation - seat.translation).length() > 1e-9 {
            figure.update(dt, &mounted, &clips, Some(&held));
            figure.straddle(Some(&Straddle { steps: Vec::new() }), Some(seat));
            highest = highest.max(figure.drawn(Some(seat), seat).translation.y);
            frames += 1;
            assert!(frames < 60, "it leaps in under a second");
        }
        assert!(
            highest > seat.translation.y + 0.1,
            "over, not across: {highest}"
        );
        assert!(
            figure.holding > 0.8,
            "astride as it lands: {}",
            figure.holding
        );
    }

    /// Off a back, it springs up and away from where it sat, comes down on
    /// its feet beside it in under a second, landing once, heard where it
    /// stands; it plays the jump clip until it lands, then stands.
    #[test]
    fn it_leaps_off_the_saddle_onto_its_feet_beside_it() {
        let held = Held {
            bones: vec![("hips", Quat::from_rotation_y(1.0))],
            hips: Vec3::new(0.0, 0.8, 0.0),
        };
        let (clips, dt) = (Clips::default(), 1.0 / 60.0);
        let astride = Straddle { steps: Vec::new() };
        let saddle = DAffine3::from_translation(DVec3::new(0.0, 1.2, 0.0));
        let mounted = moving(Gait::Idle);
        let mut figure = Figure::default();
        for _ in 0..60 {
            figure.update(dt, &mounted, &clips, Some(&held));
            figure.straddle(Some(&astride), Some(saddle));
        }
        let sat = figure.drawn(Some(figure.seat(saddle)), saddle);
        // Stepped off, the body stands beside the horse by the next frame.
        let stands = DVec3::new(2.0, 0.0, 0.0);
        let on_foot = Motion {
            position: stands,
            ..moving(Gait::Idle)
        };
        let feet = DAffine3::from_translation(stands);
        figure.straddle(None, None);
        assert!((figure.drawn(None, feet).translation - sat.translation).length() < 1e-9);
        let (mut highest, mut frames, mut landed) = (f64::MIN, 0, Vec::new());
        while (figure.drawn(None, feet).translation - stands).length() > 1e-9 {
            figure.update(dt, &on_foot, &clips, None);
            figure.straddle(None, None);
            assert!(
                figure.animator.playing() == Gait::Jump || figure.alighted().is_some(),
                "in the air until it lands"
            );
            highest = highest.max(figure.drawn(None, feet).translation.y);
            landed.extend(figure.alighted());
            frames += 1;
            assert!(frames < 60, "it leaps off in under a second");
        }
        assert!(highest > sat.translation.y, "up, not only down: {highest}");
        assert_eq!(landed, vec![stands], "it lands once, on its feet");
        for _ in 0..20 {
            figure.update(dt, &on_foot, &clips, None);
            landed.extend(figure.alighted());
        }
        assert_eq!(landed.len(), 1);
        assert_eq!(figure.animator.playing(), Gait::Idle, "it stands");
        // Seated again, a leap off never draws it on the seat.
        assert_eq!(figure.drawn(Some(saddle), feet), saddle);
    }

    #[test]
    fn a_clip_asked_for_over_a_way_plays_alone_then_the_way_s_pose_comes_on() {
        let held = Held {
            bones: vec![("hips", Quat::from_rotation_y(1.0))],
            hips: Vec3::new(0.0, 0.8, 0.0),
        };
        let (clips, dt) = (Clips::default(), 1.0 / 60.0);
        let mut figure = Figure::default();
        for _ in 0..30 {
            figure.update(dt, &moving(Gait::Fall), &clips, Some(&held));
        }
        assert_eq!(figure.held(), None);
        for _ in 0..30 {
            figure.update(dt, &moving(Gait::Idle), &clips, Some(&held));
        }
        assert_eq!(figure.held(), Some(&held));
    }

    #[test]
    fn a_clip_asked_for_plays_its_time_or_once_through_or_until_the_body_moves() {
        let clips = Clips::default();
        let play = |for_s, until_moved| Play {
            clip: Gait::Fall,
            for_s,
            until_moved,
        };
        let asked = |play, now_s| Playing::new(play, now_s, None, Mode::Walk);
        let timed = asked(play(Some(0.45), false), 10.0).unwrap();
        assert_eq!(timed.clip(), Gait::Fall);
        assert!(timed.plays(10.4, &clips, true));
        assert!(!timed.plays(10.46, &clips, false));
        // Once through: a clip not loaded plays for a second.
        let once = asked(play(None, false), 10.0).unwrap();
        assert!(once.plays(10.9, &clips, false));
        assert!(!once.plays(11.0, &clips, false));
        // Moving ends one that may be ended so, at once.
        let wave = asked(play(Some(3.0), true), 10.0).unwrap();
        assert!(wave.plays(11.0, &clips, false));
        assert!(!wave.plays(11.0, &clips, true));
        // A length that is no length of time a clip plays is refused.
        assert!(asked(play(Some(LONGEST_S), false), 0.0).is_some());
        for bad in [-1.0, f64::NAN, f64::INFINITY, LONGEST_S + 0.1, 1e9] {
            assert_eq!(asked(play(Some(bad), false), 0.0), None);
        }
    }
}
