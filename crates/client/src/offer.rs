//! The action prompt: what is within reach of a body, and the one key that
//! acts on it.
//!
//! A plugin offers its actions as data each turn, as it draws: an [`Offer`]
//! is where it stands, the thing, and what a tap and a hold of the key do,
//! all in the plugin's words. The core picks one for the prompt, [`choose`],
//! times a press of the key into a tap or a hold, [`Press`], and hands the
//! act to the plugin that offered it. With nothing offered and nothing
//! pressed, nothing is picked and nothing is measured.

use glam::DVec3;
use serde::Serialize;

use crate::plugin::Standing;

/// How far from the feet an offered point is within reach, metres.
pub const REACH_M: f64 = 3.0;

/// How far from where the body faces an offered point is in front of it, as
/// the cosine of the angle: sixty degrees either way. What an agent reads;
/// the prompt goes by nearness alone.
const FRONT_COS: f64 = 0.5;

/// How near the feet, along the ground, a point is the body's own place:
/// in front of it whichever way it faces, metres.
const UNDER_M: f64 = 0.25;

/// How long a press is a tap, seconds: let go sooner, it taps; held on, the
/// ring of a hold starts to close, so a tap never shows it.
pub const TAP_S: f64 = 0.2;

/// An action a plugin offers this turn. `thing` and the verbs are keys of
/// the plugin's words, which each front end reads in its own language.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Offer {
    /// The plugin's own name for it: what [`crate::Plugin::act`] hears.
    /// Under 2^53, so it is whole wherever JSON is read.
    pub id: u64,
    pub on: On,
    /// What it is: `board`, `horse`, `cabinet`.
    pub thing: &'static str,
    /// What a tap does: `ride`, `leave`, `play`. With none, a tap does
    /// nothing.
    pub tap: Option<&'static str>,
    /// What a hold does, and how long its ring takes to close: for what is
    /// dear to undo. An offer has a tap, a hold or both.
    pub hold: Option<Hold>,
}

/// Where an offer stands, which says when it is offered.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum On {
    /// A point of the world, the top of the thing, where the prompt hangs:
    /// offered when it is within [`REACH_M`], whichever way the body faces,
    /// the nearest of several.
    Point(DVec3),
    /// What the body is in or on, to leave it: offered when no point is
    /// within reach.
    Body,
}

/// What holding the key does, and how long, seconds, the ring around the
/// prompt takes to close, from [`TAP_S`] on, before it does.
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub struct Hold {
    pub verb: &'static str,
    pub ring_s: f32,
}

/// How an offer is acted on: the key let go soon, or held until the ring
/// closes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Act {
    Tap,
    Hold,
}

/// The prompt, as the seam says it: whose offer, where it stands, the
/// thing, and its verbs.
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub struct Prompt {
    pub plugin: &'static str,
    pub id: u64,
    pub on: Stands,
    pub thing: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tap: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hold: Option<Hold>,
}

impl Prompt {
    fn of(plugin: &'static str, offer: &Offer) -> Prompt {
        Prompt {
            plugin,
            id: offer.id,
            on: match offer.on {
                On::Point(_) => Stands::Point,
                On::Body => Stands::Body,
            },
            thing: offer.thing,
            tap: offer.tap,
            hold: offer.hold,
        }
    }

    /// Whether it is the offer a plugin gave by this id.
    pub(crate) fn is(&self, plugin: &str, id: u64) -> bool {
        self.plugin == plugin && self.id == id
    }
}

/// An offer within reach, as `offers` lists it to an agent: the prompt it
/// would be, and how far a point is.
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub struct Reached {
    #[serde(flatten)]
    pub prompt: Prompt,
    /// From the feet, metres; a point's alone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub away_m: Option<f64>,
    /// Whether the body faces it: a point's alone, the others always.
    pub in_front: bool,
}

/// Where an offer stands, as `offers` says it.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stands {
    Body,
    Point,
}

/// Every plugin's offers of its last turn, by its name.
pub(crate) type Offered = Vec<(&'static str, Vec<Offer>)>;

/// Every offer of `offered`, each with the plugin that offered it.
pub(crate) fn every(offered: &Offered) -> impl Iterator<Item = (&'static str, &Offer)> + Clone {
    offered
        .iter()
        .flat_map(|(plugin, offers)| offers.iter().map(move |offer| (*plugin, offer)))
}

/// The one offer the prompt shows: the point within reach nearest the
/// feet, whichever way the body faces, so a thing beside or just passed
/// keeps its prompt; of two as near, the one nearer the middle of where
/// the body faces. Only when no point is within reach, what the body is in
/// or on.
pub(crate) fn choose<'a>(
    offers: impl Iterator<Item = (&'static str, &'a Offer)> + Clone,
    body: &Standing,
) -> Option<Prompt> {
    let nearest = offers
        .clone()
        .filter_map(|(plugin, offer)| Some((plugin, offer, sight(body, offer.on)?)))
        .min_by(|(_, _, a), (_, _, b)| {
            a.away_m
                .total_cmp(&b.away_m)
                .then(b.middle.total_cmp(&a.middle))
        })
        .map(|(plugin, offer, _)| (plugin, offer));
    nearest
        .or_else(|| offers.clone().find(|(_, offer)| offer.on == On::Body))
        .map(|(plugin, offer)| Prompt::of(plugin, offer))
}

/// Every offer within reach, whichever way the body faces: what an agent
/// reads, and may act on by name.
pub(crate) fn within_reach<'a>(
    offers: impl Iterator<Item = (&'static str, &'a Offer)>,
    body: &Standing,
) -> Vec<Reached> {
    offers
        .filter_map(|(plugin, offer)| {
            let prompt = Prompt::of(plugin, offer);
            Some(match offer.on {
                On::Body => Reached {
                    prompt,
                    away_m: None,
                    in_front: true,
                },
                On::Point(_) => {
                    let seen = sight(body, offer.on)?;
                    Reached {
                        prompt,
                        away_m: Some(seen.away_m),
                        in_front: seen.in_front(),
                    }
                }
            })
        })
        .collect()
}

/// Where the prompt's offer stands in the world, while it is a point.
pub(crate) fn point_of(offered: &Offered, prompt: &Prompt) -> Option<DVec3> {
    offered
        .iter()
        .filter(|(plugin, _)| *plugin == prompt.plugin)
        .flat_map(|(_, offers)| offers)
        .find(|offer| offer.id == prompt.id)
        .and_then(|offer| match offer.on {
            On::Point(point) => Some(point),
            On::Body => None,
        })
}

/// The key held on an offer that has a hold: a tap if it is let go within
/// [`TAP_S`], a hold once its ring closes after that, nothing if it is let
/// go between or the offer stops being what it was pressed on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) struct Press {
    pub target: Prompt,
    held_s: f64,
    /// By the key, which letting go ends. Asked at the seam, it is held
    /// until the ring closes.
    by_key: bool,
    /// Named by an agent: it holds while the offer is within reach.
    /// Otherwise while it is the prompt.
    named: bool,
}

/// What a press comes to in a turn.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Pressed {
    /// Still held: how far its ring has closed, 0 to 1, 0 until [`TAP_S`].
    Held(f32),
    /// Over: the act it made, or none when it was let go between a tap
    /// and a hold, or broken.
    Over(Option<Act>),
}

impl Press {
    pub(crate) fn new(target: Prompt, by_key: bool, named: bool) -> Press {
        Press {
            target,
            held_s: 0.0,
            by_key,
            named,
        }
    }

    /// The press `dt` seconds on: `down` while the key is held, `there`
    /// while the offer is still what it was pressed on.
    pub(crate) fn turn(&mut self, dt: f64, down: bool, there: bool) -> Pressed {
        self.held_s += dt;
        let ring_s = self.target.hold.map_or(0.0, |hold| f64::from(hold.ring_s));
        if !there {
            Pressed::Over(None)
        } else if self.by_key && !down {
            let tapped = self.held_s < TAP_S && self.target.tap.is_some();
            Pressed::Over(tapped.then_some(Act::Tap))
        } else if self.held_s >= TAP_S + ring_s {
            Pressed::Over(Some(Act::Hold))
        } else {
            Pressed::Held(((self.held_s - TAP_S) / ring_s).clamp(0.0, 1.0) as f32)
        }
    }

    /// Whether the offer is still what it was pressed on.
    pub(crate) fn there<'a>(
        &self,
        offers: impl Iterator<Item = (&'static str, &'a Offer)> + Clone,
        body: &Standing,
    ) -> bool {
        let Prompt { plugin, id, .. } = self.target;
        match self.named {
            true => within_reach(offers, body)
                .iter()
                .any(|reached| reached.prompt.is(plugin, id)),
            false => choose(offers, body).is_some_and(|prompt| prompt.is(plugin, id)),
        }
    }
}

/// An offered point as the body sees it.
#[derive(Clone, Copy)]
struct Sight {
    /// From the feet, metres.
    away_m: f64,
    /// How near the middle of where the body faces, along the ground: the
    /// cosine of the angle off it, one at the feet.
    middle: f64,
}

impl Sight {
    fn in_front(self) -> bool {
        self.middle >= FRONT_COS
    }
}

/// How an offered point is seen from the body: `None` past reach, and for
/// an offer that stands at no point.
fn sight(body: &Standing, on: On) -> Option<Sight> {
    let On::Point(point) = on else {
        return None;
    };
    let toward = point - body.position;
    let away_m = toward.length();
    if away_m > REACH_M {
        return None;
    }
    let along = toward - body.up * toward.dot(body.up);
    let length = along.length();
    let middle = match length < UNDER_M {
        true => 1.0,
        false => along.dot(body.facing) / length,
    };
    Some(Sight { away_m, middle })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body at the origin, up `+y`, facing `-z`.
    fn body() -> Standing {
        Standing {
            position: DVec3::ZERO,
            up: DVec3::Y,
            facing: DVec3::NEG_Z,
        }
    }

    fn offer(id: u64, on: On) -> Offer {
        Offer {
            id,
            on,
            thing: "thing",
            tap: Some("use"),
            hold: None,
        }
    }

    fn chosen(offered: &Offered) -> Option<(&'static str, u64)> {
        choose(every(offered), &body()).map(|prompt| (prompt.plugin, prompt.id))
    }

    #[test]
    fn nothing_offered_is_no_prompt() {
        assert_eq!(chosen(&Vec::new()), None);
        assert_eq!(chosen(&vec![("a", Vec::new())]), None);
    }

    /// A point on the ground `away_m` from the feet, `deg` off the facing.
    fn at(away_m: f64, deg: f64) -> On {
        let rad = deg.to_radians();
        On::Point(DVec3::new(rad.sin(), 0.0, -rad.cos()) * away_m)
    }

    #[test]
    fn a_point_within_reach_comes_before_what_the_body_is_on() {
        let offered = vec![
            ("body", vec![offer(2, On::Body)]),
            ("point", vec![offer(1, at(2.0, 30.0))]),
        ];
        assert_eq!(chosen(&offered), Some(("point", 1)));
        assert_eq!(chosen(&offered[..1].to_vec()), Some(("body", 2)));
    }

    #[test]
    fn the_nearest_point_is_offered_and_the_facing_breaks_a_tie() {
        let offered = vec![
            ("near_aside", vec![offer(1, at(1.0, 80.0))]),
            ("far_ahead", vec![offer(2, at(2.8, 5.0))]),
        ];
        assert_eq!(chosen(&offered), Some(("near_aside", 1)));
        let offered = vec![
            ("off", vec![offer(1, at(2.0, 40.0))]),
            ("ahead", vec![offer(2, at(2.0, 10.0))]),
        ];
        assert_eq!(chosen(&offered), Some(("ahead", 2)));
    }

    #[test]
    fn a_point_beside_or_behind_is_offered_and_one_past_reach_is_not() {
        for on in [at(2.0, 90.0), at(1.0, 180.0), at(REACH_M - 0.1, -120.0)] {
            let offered = vec![("p", vec![offer(1, on)])];
            assert_eq!(chosen(&offered), Some(("p", 1)), "{on:?}");
        }
        let offered = vec![("p", vec![offer(1, at(REACH_M + 0.1, 0.0))])];
        assert_eq!(chosen(&offered), None);
    }

    #[test]
    fn turning_keeps_the_prompt_on_what_is_near() {
        let offered = vec![
            (
                "near",
                vec![offer(1, On::Point(DVec3::new(0.0, 0.0, -1.5)))],
            ),
            ("far", vec![offer(2, On::Point(DVec3::new(2.5, 0.0, 0.0)))]),
            ("body", vec![offer(3, On::Body)]),
        ];
        for deg in [0.0, 45.0, 90.0, 180.0, 270.0] {
            let rad = f64::to_radians(deg);
            let facing = DVec3::new(rad.sin(), 0.0, -rad.cos());
            let body = Standing { facing, ..body() };
            let plugin = choose(every(&offered), &body).map(|prompt| prompt.plugin);
            assert_eq!(plugin, Some("near"), "{deg}");
        }
    }

    #[test]
    fn an_agent_reads_every_offer_within_reach_whichever_way_it_faces() {
        let offered = vec![(
            "p",
            vec![
                offer(1, On::Point(DVec3::new(0.0, 0.0, 2.0))),
                offer(2, On::Point(DVec3::new(0.0, 0.0, 5.0))),
                offer(3, On::Body),
            ],
        )];
        let reached = within_reach(every(&offered), &body());
        let listed: Vec<_> = reached
            .iter()
            .map(|r| (r.prompt.id, r.prompt.on, r.in_front))
            .collect();
        assert_eq!(
            listed,
            vec![(1, Stands::Point, false), (3, Stands::Body, true)]
        );
        assert_eq!(
            serde_json::to_string(&reached[1]).unwrap(),
            r#"{"plugin":"p","id":3,"on":"body","thing":"thing","tap":"use","in_front":true}"#
        );
    }

    #[test]
    fn a_press_taps_when_let_go_soon_and_holds_once_its_ring_closes() {
        let target = |tap, ring_s| Prompt {
            hold: Some(Hold {
                verb: "take",
                ring_s,
            }),
            tap,
            ..Prompt::of("p", &offer(1, On::Body))
        };
        let both = target(Some("use"), 1.0);
        // Let go before the tap ends: a tap, the ring never shown.
        let mut press = Press::new(both, true, false);
        assert_eq!(press.turn(0.1, true, true), Pressed::Held(0.0));
        assert_eq!(press.turn(0.05, false, true), Pressed::Over(Some(Act::Tap)));
        // Held on: the ring closes over its seconds, then it holds.
        let mut press = Press::new(both, true, false);
        assert_eq!(press.turn(TAP_S + 0.5, true, true), Pressed::Held(0.5));
        assert_eq!(press.turn(0.5, true, true), Pressed::Over(Some(Act::Hold)));
        // Let go with the ring half closed: nothing.
        let mut press = Press::new(both, true, false);
        press.turn(TAP_S + 0.5, true, true);
        assert_eq!(press.turn(0.01, false, true), Pressed::Over(None));
        // No tap offered: a short press is nothing.
        let mut press = Press::new(target(None, 1.0), true, false);
        assert_eq!(press.turn(0.05, false, true), Pressed::Over(None));
        // Asked at the seam, nothing lets it go but the ring closing.
        let mut press = Press::new(both, false, true);
        assert_eq!(press.turn(TAP_S, false, true), Pressed::Held(0.0));
        assert_eq!(press.turn(1.0, false, true), Pressed::Over(Some(Act::Hold)));
        // The offer gone: broken, whatever is held.
        let mut press = Press::new(both, true, false);
        assert_eq!(press.turn(0.3, true, false), Pressed::Over(None));
    }
}
