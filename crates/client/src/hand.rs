//! Things a plugin gives a body's hand. A plugin says each as data, once, as
//! it is plugged in, the way it says its ways: a name, the pose of the arm
//! that holds it, what shows in the hand and the light it throws. The core
//! holds the thing: it lays the arm over whatever poses the body, draws the
//! thing at the hand and lights its lamp, on this client's body and on every
//! peer's whose stance names it. A thing is known by its full name, the
//! plugin's and its own, `<plugin>.<thing>`, and only while its plugin is
//! on.

use glam::{DAffine3, DVec3, Quat};
use scene::{BoxPart, Lamp};

use crate::figure::Motion;
use crate::given::{Gift, Given};

/// How many bodies' lamps light a view: this client's own, and the peers'
/// nearest the camera. Each lights every pixel near it, on a phone too.
pub(crate) const LAMPS: usize = 4;

/// The right hand, which grasps a thing; the left keeps the reins.
pub(crate) const BONE: avatar::Bone = avatar::Bone::RIGHT_HAND;

/// Where the box figure, drawn before an avatar lands, holds a thing: to
/// its right, ahead, at the height of its middle, in the body's frame.
const BOX_WRIST: DVec3 = DVec3::new(0.25, 1.0, -0.3);

/// The box figure's hand frame: its body's axes at its right wrist, at
/// least `out_m` from its middle when it sits astride.
pub(crate) fn box_hand(motion: &Motion, out_m: Option<f64>) -> DAffine3 {
    let wrist = BOX_WRIST.with_x(BOX_WRIST.x.max(out_m.unwrap_or(0.0)));
    DAffine3::from_mat3_translation(motion.basis, motion.position + motion.basis * wrist)
}

/// A thing a body holds in its hand, as a plugin gives it. The hand's
/// frame is the body's (`x` right, `y` up, `z` back) with its origin at the
/// wrist of the right hand.
#[derive(Clone, Debug)]
pub struct Thing {
    /// Its name in the plugin, the word after the dot.
    pub name: &'static str,
    /// The arm that holds it: bones of the humanoid by the names VRM gives
    /// them, each turned from rest in its parent's frame, laid over the
    /// clip and the way's pose. The rest of the body plays on.
    pub arm: Vec<(&'static str, Quat)>,
    /// What shows of it, in the hand's frame.
    pub boxes: Vec<BoxPart>,
    /// The light it throws, in the hand's frame, its beam too.
    pub lamp: Option<Lamp>,
}

impl Thing {
    /// Sets it in a hand, the hand's frame in world space: its boxes go
    /// into `boxes` as `drawn` has them drawn, and its lamp comes back,
    /// world space.
    pub(crate) fn placed(
        &self,
        hand: DAffine3,
        boxes: &mut Vec<BoxPart>,
        drawn: impl Fn(BoxPart) -> BoxPart,
    ) -> Option<Lamp> {
        boxes.extend(self.boxes.iter().map(|part| {
            drawn(BoxPart {
                transform: hand * part.transform,
                ..*part
            })
        }));
        self.lamp.map(|lamp| Lamp {
            position: hand.transform_point3(lamp.position),
            beam: lamp.beam.map(|beam| scene::Beam {
                direction: (hand.matrix3 * beam.direction.as_dvec3())
                    .normalize()
                    .as_vec3(),
                ..beam
            }),
            ..lamp
        })
    }
}

/// Every thing the plugins plugged in gave a hand.
pub(crate) type Things = Given<Thing>;

impl Gift for Thing {
    const NOUN: &'static str = "thing";
    const BYTES: usize = protocol::Limit::HandBytes as usize;

    fn name(&self) -> &'static str {
        self.name
    }

    fn wrong(&self) -> Option<String> {
        if !self.boxes.iter().all(|part| part.transform.is_finite()) {
            return Some("has a box out of the numbers".into());
        }
        let lamp = self.lamp.as_ref()?;
        let whole = lamp.position.is_finite()
            && lamp.color.is_finite()
            && lamp.reach_m.is_finite()
            && lamp.reach_m > 0.0
            && lamp.size_m.is_finite()
            && lamp.size_m >= 0.0;
        if !whole {
            return Some("has a lamp out of the numbers".into());
        }
        let beam = lamp.beam.as_ref()?;
        // Wider than a half turn, the cone would turn inside out and leave
        // dark what it means to light.
        let whole = beam.direction.is_finite()
            && beam.direction.length_squared() > 1e-6
            && beam.spread >= 0.0
            && beam.edge >= 0.0
            && beam.spread + beam.edge < core::f32::consts::PI;
        (!whole).then(|| "has a beam with no direction, or wider than a half turn".into())
    }
}

#[cfg(test)]
mod tests {
    use glam::{DMat3, Vec3};
    use scene::Beam;

    use super::*;

    fn lantern() -> Thing {
        Thing {
            name: "lantern",
            arm: Vec::new(),
            boxes: vec![BoxPart {
                transform: DAffine3::from_translation(DVec3::new(0.0, 0.0, -0.1)),
                color: Vec3::ONE,
                shine: 0.0,
                shadow_only: false,
                sheet: false,
            }],
            lamp: Some(Lamp {
                position: DVec3::new(0.0, 0.0, -0.2),
                color: Vec3::ONE,
                reach_m: 20.0,
                size_m: 0.1,
                beam: Some(Beam {
                    direction: Vec3::NEG_Z,
                    spread: 0.3,
                    edge: 0.1,
                }),
            }),
        }
    }

    #[test]
    fn a_thing_stands_in_the_hand_and_throws_its_light_where_the_hand_points() {
        // Facing +x: the body's back is -x.
        let basis = DMat3::from_cols(DVec3::Z, DVec3::Y, DVec3::NEG_X);
        let hand = DAffine3::from_mat3_translation(basis, DVec3::new(5.0, 1.0, 0.0));
        let mut boxes = Vec::new();
        let lamp = lantern().placed(hand, &mut boxes, |part| part).unwrap();
        assert_eq!(boxes.len(), 1);
        assert!(
            boxes[0]
                .transform
                .translation
                .distance(DVec3::new(5.1, 1.0, 0.0))
                < 1e-9
        );
        assert!(lamp.position.distance(DVec3::new(5.2, 1.0, 0.0)) < 1e-9);
        assert!(lamp.beam.unwrap().direction.distance(Vec3::X) < 1e-6);
    }

    #[test]
    fn a_thing_whose_beam_would_turn_inside_out_or_read_no_number_is_refused() {
        let with = |spread: f32, direction: Vec3| {
            let mut thing = lantern();
            let lamp = thing.lamp.as_mut().unwrap();
            lamp.beam = Some(Beam {
                direction,
                spread,
                edge: 0.2,
            });
            thing
        };
        assert!(with(0.3, Vec3::NEG_Z).wrong().is_none());
        assert!(with(3.0, Vec3::NEG_Z).wrong().is_some());
        assert!(with(f32::NAN, Vec3::NEG_Z).wrong().is_some());
        assert!(with(0.3, Vec3::ZERO).wrong().is_some());
        let mut things = Things::default();
        assert_eq!(
            things
                .give("flashlight", vec![with(3.0, Vec3::NEG_Z)])
                .len(),
            1
        );
    }
}
