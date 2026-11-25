//! The flashlight plugin's client half: a flashlight the body's hand
//! grasps, its beam thrown ahead. `L` switches it on and off; an agent says
//!
//! ```json
//! {"type":"flashlight.on"}
//! {"type":"flashlight.off"}
//! {"type":"flashlight.state"}
//! ```
//!
//! and hears `{"type":"flashlight.changed","on":true}` each time it
//! changes, whoever switched it; asked its state, it answers
//! `flashlight.state` with the same word.
//!
//! The flashlight is a thing the plugin gives a hand as data: the arm that
//! holds it forward, the block of the flashlight with its lens alight, and
//! the beam. The core lays the arm over whatever the body plays, draws the
//! flashlight at the wrist and throws the beam where the body faces, level
//! and falling a little to the ground ahead, never where the camera looks.
//! Peers see it, on foot, on a board, on a horse and aloft, since the
//! stance says what the hand grasps.

use client::{Host, KeyAsk, Plugin, Thing};
use glam::{DAffine3, DQuat, DVec3, Quat, Vec3};
use scene::{Beam, BoxPart, Lamp};
use serde::Serialize;
use serde_json::Value;

/// The plugin's name, its version and the name of its thing are said once,
/// by its world half.
pub use flashlight_world::{NAME, THING, VERSION};

/// `flashlight.on`: the flashlight in the hand, lit.
pub const ON: &str = "on";
/// `flashlight.off`: the hand empty.
pub const OFF: &str = "off";
/// `flashlight.changed`: what it says when the flashlight comes or goes.
pub const CHANGED: &str = "changed";
/// `flashlight.state`: asked, it answers with the same word whether the
/// flashlight is in the hand.
pub const STATE: &str = "state";

/// The names of the keys it asks for.
pub mod key {
    /// Switches the flashlight on, or off.
    pub const LIGHT: &str = "light";
}

/// How far below level the beam falls, radians: from a hand at the chest
/// it meets the ground a few metres ahead.
const FALL: f32 = 0.12;
/// The half angle the beam lights whole, and how far past it it fades out,
/// radians.
const SPREAD: f32 = 0.33;
const EDGE: f32 = 0.18;
/// How far the beam reaches, metres, and how bright it is: what it gives a
/// side facing it from a metre away, warm white, a little bluer than a
/// candle.
const REACH_M: f32 = 28.0;
const BRIGHT: Vec3 = Vec3::new(7.0, 6.4, 5.3);
/// How wide its light is taken to come from, metres: nearer than that the
/// ground is not burnt white.
const SIZE_M: f32 = 3.5;

/// The flashlight's body, dark, and the lens at its front, lit.
const BODY: Vec3 = Vec3::new(0.05, 0.05, 0.055);
const LENS: Vec3 = Vec3::new(1.0, 0.92, 0.75);

/// The plugin: whether the flashlight is in the hand is the core's to keep, so
/// a switch from anywhere reads the same.
#[derive(Default)]
pub struct Flashlight;

/// The plugin, as a shell plugs it in.
pub fn plugin() -> Box<dyn Plugin> {
    Box::new(Flashlight)
}

/// What it tells when the flashlight comes or goes, or is asked.
#[derive(Serialize)]
struct Changed {
    on: bool,
}

impl Plugin for Flashlight {
    fn name(&self) -> &'static str {
        NAME
    }

    fn version(&self) -> u32 {
        VERSION
    }

    fn command(&mut self, kind: &str, _body: Value, host: &mut Host<'_>) {
        match kind {
            ON => switch(host, true),
            OFF => switch(host, false),
            STATE => {
                let on = host.grasping().is_some();
                host.emit(STATE, &Changed { on });
            }
            _ => host.reject(format!("`{kind}` is no command of flashlight")),
        }
    }

    fn keys(&self) -> Vec<KeyAsk> {
        vec![KeyAsk::pressed(key::LIGHT, "KeyL")]
    }

    fn key(&mut self, name: &str, host: &mut Host<'_>) {
        if name == key::LIGHT {
            let on = host.grasping().is_none();
            switch(host, on);
        }
    }

    fn things(&self) -> Vec<Thing> {
        vec![thing()]
    }

    /// Switched off by the world, it lets go: the core takes the
    /// flashlight back, and whoever listens hears it went.
    fn off(&mut self, host: &mut Host<'_>) {
        if host.grasping().is_some() {
            host.emit(CHANGED, &Changed { on: false });
        }
    }
}

/// Puts the flashlight in the hand or takes it away, and says so when it
/// changed. Refused while the hand holds another plugin's thing.
fn switch(host: &mut Host<'_>, on: bool) {
    if (host.grasping() == Some(THING)) == on {
        return;
    }
    if !host.grasp(on.then_some(THING)) {
        host.reject("the hand holds something else".into());
        return;
    }
    host.emit(CHANGED, &Changed { on });
}

/// The flashlight as a hand grasps it, in the hand's frame: the body's axes,
/// `x` right, `y` up, `z` back, at the right wrist.
pub fn thing() -> Thing {
    // Down the beam's line: ahead, falling a little.
    let aim = DQuat::from_rotation_x(-f64::from(FALL));
    let along = |middle: f64, size: DVec3| {
        DAffine3::from_quat(aim)
            * DAffine3::from_translation(DVec3::new(0.0, 0.0, -middle))
            * DAffine3::from_scale(size)
    };
    let boxes = vec![
        BoxPart {
            transform: along(0.08, DVec3::new(0.06, 0.06, 0.22)),
            color: BODY,
            shine: 0.0,
            shadow_only: false,
            sheet: false,
        },
        // The head, a little wider, and the lens in it.
        BoxPart {
            transform: along(0.2, DVec3::new(0.085, 0.085, 0.06)),
            color: BODY,
            shine: 0.0,
            shadow_only: false,
            sheet: false,
        },
        BoxPart {
            transform: along(0.232, DVec3::new(0.07, 0.07, 0.01)),
            color: LENS,
            shine: 1.0,
            shadow_only: false,
            sheet: false,
        },
    ];
    let lens = DAffine3::from_quat(aim).transform_point3(DVec3::new(0.0, 0.0, -0.24));
    Thing {
        name: THING,
        arm: arm(),
        boxes,
        lamp: Some(Lamp {
            position: lens,
            color: BRIGHT,
            reach_m: REACH_M,
            size_m: SIZE_M,
            beam: Some(Beam {
                direction: Quat::from_rotation_x(-FALL) * Vec3::NEG_Z,
                spread: SPREAD,
                edge: EDGE,
            }),
        }),
    }
}

/// Where the upper arm and the forearm point grasping the flashlight, in the
/// figure's frame: the upper arm down by the side and a little ahead, the
/// forearm out ahead at the height of the chest, a little in.
const UPPER: Vec3 = Vec3::new(0.12, -0.8, -0.58);
const FOREARM: Vec3 = Vec3::new(-0.12, 0.18, -1.0);

/// The right arm holding the flashlight out ahead. A humanoid's right arm rests
/// along `x`; each bone turns from there, the forearm in its upper arm's
/// frame.
pub fn arm() -> Vec<(&'static str, Quat)> {
    let upper = Quat::from_rotation_arc(Vec3::X, UPPER.normalize());
    let forearm = Quat::from_rotation_arc(Vec3::X, upper.inverse() * FOREARM.normalize());
    vec![
        ("rightUpperArm", upper),
        ("rightLowerArm", forearm),
        ("rightHand", Quat::IDENTITY),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arm_holds_the_forearm_out_ahead() {
        let bones = arm();
        let upper = bones[0].1;
        let forearm = bones[1].1;
        let pointing = upper * forearm * Vec3::X;
        assert!(pointing.distance(FOREARM.normalize()) < 1e-4, "{pointing}");
    }

    #[test]
    fn the_beam_falls_a_little_ahead_from_the_lens() {
        let lamp = thing().lamp.unwrap();
        let beam = lamp.beam.unwrap();
        assert!(beam.direction.z < -0.95 && beam.direction.y < -0.1);
        assert!(lamp.position.z < -0.2);
    }
}
