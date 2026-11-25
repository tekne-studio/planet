//! Playing a cabinet. In front of a cabinet with a game, its cover on its
//! glass, the prompt offers "Cabinet [E] Play"; a tap runs the game, lends
//! the body's controls to it and the eye with them, eased to face its
//! screen. While it is played, what the controls press is the game's
//! buttons, held each turn, and the body's own offer is "Cabinet [E]
//! Leave". The controls come back by that offer, by the key that always
//! leaves, and whenever the game stops or the cabinet goes: nothing holds a
//! person in a cabinet. Left, the game waits for them where it stopped, and
//! the cabinet offers "Cabinet [E] Resume" while it does
//! ([`client::GAME_WAITS_S`]); past that it stops, and its cover shows
//! again.
//!
//! Each client runs its own copy of a game from the same seed: the one
//! playing sees their game, and those near see its cover on theirs.

use client::{Act, Host, Look, Offer, On, PartId, Shape, Turn, cabinet};
use glam::{DAffine3, DMat3, DQuat, DVec3};

use crate::Items;

/// The words its offers say, keys of the panel's words.
pub mod word {
    pub const CABINET: &str = "cabinet";
    pub const PLAY: &str = "play";
    pub const RESUME: &str = "resume";
    pub const LEAVE: &str = "leave";
}

/// Leaving the cabinet played, as the prompt offers it and an act names
/// it. A cabinet to play is offered as one more than its place among those
/// near.
pub const OFFER_LEAVE: u64 = 0;

/// How far from the body a cabinet is offered, metres: the prompt takes
/// those within its reach of these.
const NEAR_M: f64 = 8.0;

/// How much of the view the screen fills, across or down, whichever
/// binds first.
const SCREEN_FILLS: f64 = 0.7;

/// How far over the view's middle the screen sits, of the view's height:
/// under it the prompt's card offers leaving, clear of the game.
const SCREEN_RISES: f64 = 0.06;

/// Where the prompt hangs on a cabinet, in its box: the top of its front.
const PROMPT_AT: DVec3 = DVec3::new(0.0, -0.5, 1.0);

impl Items {
    /// The turn of playing: the game played runs and gets what the
    /// controls press, and the eye follows its screen; with none played,
    /// every cabinet near whose game may be played is offered.
    pub(crate) fn play_turn(&mut self, turn: &Turn<'_>, host: &mut Host<'_>) {
        if let Some(played) = self.played
            && !(host.holds_controls() && host.game_playable(played))
        {
            self.leave(host);
        }
        let offers = match self.played {
            Some(played) => {
                host.play_game(played);
                if let Some(pad) = turn.controls {
                    host.press_game(played, pad);
                }
                // The cabinet may move while played, and the view change
                // shape: the eye goes with them.
                if let Some(look) = self.look(host, played) {
                    host.hold_controls(Some(look));
                }
                vec![Offer {
                    id: OFFER_LEAVE,
                    on: On::Body,
                    thing: word::CABINET,
                    tap: Some(word::LEAVE),
                    hold: None,
                }]
            }
            None => {
                self.near = cabinets_near(host);
                self.near
                    .iter()
                    .zip(1..)
                    .filter_map(|(&(id, top), offer)| {
                        let tap = match host.game_waits(id) {
                            true => word::RESUME,
                            false => word::PLAY,
                        };
                        host.game_playable(id).then_some(Offer {
                            id: offer,
                            on: On::Point(top),
                            thing: word::CABINET,
                            tap: Some(tap),
                            hold: None,
                        })
                    })
                    .collect()
            }
        };
        host.offer(offers);
    }

    /// An act on one of its offers: a cabinet played, again where its game
    /// waits, or left.
    pub(crate) fn play_act(&mut self, id: u64, act: Act, host: &mut Host<'_>) {
        if act != Act::Tap {
            return;
        }
        if id == OFFER_LEAVE {
            return self.leave(host);
        }
        let Some(&(cabinet, _)) = self.near.get(id as usize - 1) else {
            return;
        };
        if !host.game_playable(cabinet) {
            return host.reject("its game does not play".into());
        }
        if host.hold_controls(self.look(host, cabinet)) {
            host.play_game(cabinet);
            self.played = Some(cabinet);
        }
    }

    /// The look at a cabinet's screen, framed in the eye last seen.
    fn look(&self, host: &Host<'_>, cabinet: PartId) -> Option<Look> {
        let (fov_y, aspect) = self.eye?;
        let look = facing(screen_of(host, cabinet)?, fov_y, aspect)?;
        // Once there, the game is the person's screen, laid as drawn.
        Some(Look {
            shows: Some(cabinet),
            ..look
        })
    }

    /// Gives the controls back, and lets the game go.
    pub(crate) fn leave(&mut self, host: &mut Host<'_>) {
        self.played = None;
        host.let_controls_go();
    }
}

/// Every cabinet near the body holding a cartridge, with where its prompt
/// hangs.
fn cabinets_near(host: &Host<'_>) -> Vec<(PartId, DVec3)> {
    let body = host.body().position;
    let feet = host.feet();
    host.parts()
        .near(feet.seat, feet.point, NEAR_M)
        .filter_map(|(id, _, part)| {
            let Shape::Cabinet(held) = part.shape else {
                return None;
            };
            held.cart?;
            let world = host.part_transform(id)?;
            let top = world.transform_point3(PROMPT_AT);
            ((top - body).length() <= NEAR_M).then_some((id, top))
        })
        .collect()
}

/// Where a cabinet's game shows in the world: the unit quad over its
/// screen, facing out of it.
fn screen_of(host: &Host<'_>, cabinet: PartId) -> Option<DAffine3> {
    Some(host.part_transform(cabinet)? * cabinet::screen())
}

/// The look from straight in front of a screen, far enough back that it
/// fills [`SCREEN_FILLS`] of a view `fov_y` tall and `aspect` wide, and a
/// little under it, so it sits [`SCREEN_RISES`] over the middle.
fn facing(screen: DAffine3, fov_y: f64, aspect: f64) -> Option<Look> {
    let across = screen.matrix3.x_axis;
    let up = screen.matrix3.y_axis.normalize_or_zero();
    let out = across.cross(screen.matrix3.y_axis).normalize_or_zero();
    if out == DVec3::ZERO || up == DVec3::ZERO {
        return None;
    }
    let half_tall = libm::tan(fov_y / 2.0);
    let away_m = f64::max(
        screen.matrix3.y_axis.length() / 2.0 / half_tall,
        across.length() / 2.0 / (half_tall * aspect.max(f64::EPSILON)),
    ) / SCREEN_FILLS;
    let right = across.normalize();
    let tall_m = 2.0 * away_m * half_tall;
    Some(Look {
        position: screen.translation + out * away_m - up * tall_m * SCREEN_RISES,
        rotation: DQuat::from_mat3(&DMat3::from_cols(right, out.cross(right), out)),
        shows: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_look_faces_the_screen_and_fills_most_of_the_view() {
        // A screen half a metre wide and 0.375 tall, facing +y: its across
        // runs toward -x, as the right of one looking at it from +y.
        let screen = DAffine3 {
            matrix3: DMat3::from_cols(
                DVec3::new(-0.5, 0.0, 0.0),
                DVec3::new(0.0, 0.0, 0.375),
                DVec3::new(0.0, 0.01, 0.0),
            ),
            translation: DVec3::new(0.0, 0.0, 1.2),
        };
        let fov_y = 1.0;
        let look = facing(screen, fov_y, 16.0 / 9.0).unwrap();
        // In front of it, looking straight at its face, upright.
        assert!(look.position.y > 0.0);
        let ahead = look.rotation * DVec3::NEG_Z;
        assert!(ahead.dot(DVec3::NEG_Y) > 0.9999, "{ahead}");
        assert!((look.rotation * DVec3::Y).z > 0.99, "upright");
        // Its height fills SCREEN_FILLS of the view's, and its middle sits
        // SCREEN_RISES of it over the view's.
        let away_m = look.position.y - screen.translation.y;
        let tall_m = 2.0 * away_m * libm::tan(fov_y / 2.0);
        assert!((0.375 / tall_m - SCREEN_FILLS).abs() < 1e-9);
        let rise = (screen.translation.z - look.position.z) / tall_m;
        assert!((rise - SCREEN_RISES).abs() < 1e-9, "{rise}");
        // A tall narrow view is bound by its width.
        let narrow = facing(screen, fov_y, 0.5).unwrap();
        assert!(narrow.position.y - screen.translation.y > away_m);
    }
}
