//! Playing a cabinet. In front of a cabinet with a game, its cover on its
//! glass, the prompt offers "Cabinet [E] Play"; a tap runs the game, lends
//! the body's controls to it and the eye with them, eased to face its
//! screen as from the player's head, and steps the body to player 1's
//! station, square on to the machine ([`step_to_station`]), or to the
//! station it holds of a game of several pads ([`crate::stations`]). While
//! it is played, what the controls press is the game's
//! buttons, held each turn, and the body's own offer is "Cabinet [E]
//! Leave". The controls come back by that offer, by the key that always
//! leaves, and whenever the game stops or the cabinet goes: nothing holds a
//! person in a cabinet. Left, the game waits for them where it stopped, and
//! the cabinet offers "Cabinet [E] Resume" while it does
//! ([`client::GAME_WAITS_S`]); past that it stops, and its cover shows
//! again. Left, the body stands where it played, free, and the eye eases
//! back to the camera the person keeps.
//!
//! Each client runs its own copy of a game of one pad from the same seed:
//! the one playing sees their game, and those near see its cover on
//! theirs. A game of several pads is played together, a player a station.

use core::f64::consts::FRAC_1_SQRT_2;

use client::{Act, Eye, Host, Look, Offer, On, PartId, Shape, Turn, cabinet};
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

/// Where the prompt hangs on a cabinet, in its box: the top of its front.
const PROMPT_AT: DVec3 = DVec3::new(0.0, -0.5, 1.0);

impl Items {
    /// The turn of playing: the game played runs and gets what the
    /// controls press, and the eye follows its screen; with none played,
    /// every cabinet near whose game may be played is offered.
    pub(crate) fn play_turn(&mut self, turn: &Turn<'_>, host: &mut Host<'_>) {
        self.stations_turn(turn, host);
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
                        let tap = match host.game_waits(id) || self.sits_at(host, id) {
                            true => word::RESUME,
                            false => word::PLAY,
                        };
                        let open = host.game_playable(id) && self.station_open(host, id);
                        open.then_some(Offer {
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
        let Some(station) = self.sit(host, cabinet) else {
            return host.reject("every station of its game is held".into());
        };
        if host.hold_controls(self.look(host, cabinet)) {
            step_to_station(host, cabinet, station);
            host.play_game(cabinet);
            self.played = Some(cabinet);
        }
    }

    /// The look at a cabinet's screen, framed in the eye last seen.
    fn look(&self, host: &Host<'_>, cabinet: PartId) -> Option<Look> {
        let look = facing(screen_of(host, cabinet)?, &self.eye?)?;
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

/// Steps the body to the station `index` of the cabinet `cabinet`, square
/// on to its screen ([`cabinet::station`]), as a step of its own
/// ([`Host::step_to`]). False, and the body where it stood, past the last
/// station, for a cabinet more on a side than standing, or where the step
/// is stopped, by a wall or a volume: whoever plays plays all the same.
pub(crate) fn step_to_station(host: &mut Host<'_>, cabinet: PartId, index: usize) -> bool {
    let (Some(world), Some(station)) = (host.part_transform(cabinet), cabinet::station(index))
    else {
        return false;
    };
    let standing = world.matrix3.z_axis.normalize_or_zero().dot(host.body().up);
    if standing < FRAC_1_SQRT_2 {
        return false;
    }
    host.step_to(
        world.transform_point3(station.feet),
        world.transform_vector3(station.facing),
    )
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
/// fills the part of the view `eye` leaves clear, across or down,
/// whichever binds first, and moved off its middle so the screen's middle
/// is the clear part's: the game is laid there, as large as it is drawn,
/// and nothing laid over the view covers it.
fn facing(screen: DAffine3, eye: &Eye) -> Option<Look> {
    let across = screen.matrix3.x_axis;
    let up = screen.matrix3.y_axis.normalize_or_zero();
    let out = across.cross(screen.matrix3.y_axis).normalize_or_zero();
    if out == DVec3::ZERO || up == DVec3::ZERO {
        return None;
    }
    let [left, top, right, bottom] = eye.clear;
    let half_tall = libm::tan(eye.fov_y / 2.0);
    let half_wide = half_tall * eye.aspect.max(f64::EPSILON);
    let away_m = f64::max(
        screen.matrix3.y_axis.length() / 2.0 / (half_tall * (bottom - top).max(f64::EPSILON)),
        across.length() / 2.0 / (half_wide * (right - left).max(f64::EPSILON)),
    );
    // Where the clear part's middle is on the view, -1 to 1 from its
    // middle, up and right: the eye stands that far the other way.
    let [x, y] = [left + right - 1.0, 1.0 - top - bottom];
    let right_way = across.normalize();
    Some(Look {
        position: screen.translation + out * away_m
            - right_way * (x * half_wide * away_m)
            - up * (y * half_tall * away_m),
        rotation: DQuat::from_mat3(&DMat3::from_cols(right_way, out.cross(right_way), out)),
        shows: None,
    })
}

#[cfg(test)]
mod tests {
    use client::{Client, Input, Key, Pad, Running};
    use topology::SurfacePoint;

    use super::*;
    use crate::tests::{client, games, placed, shown, update};

    /// A screen half a metre wide and 0.375 tall, facing +y: its across
    /// runs toward -x, as the right of one looking at it from +y.
    fn screen() -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::from_cols(
                DVec3::new(-0.5, 0.0, 0.0),
                DVec3::new(0.0, 0.0, 0.375),
                DVec3::new(0.0, 0.01, 0.0),
            ),
            translation: DVec3::new(0.0, 0.0, 1.2),
        }
    }

    /// An eye a radian tall on a view `aspect` wide, leaving `clear` clear.
    fn eye(aspect: f64, clear: [f64; 4]) -> Eye {
        Eye {
            position: DVec3::ZERO,
            rotation: DQuat::IDENTITY,
            fov_y: 1.0,
            aspect,
            clear,
            pointer: [0.5, 0.5],
        }
    }

    /// Where the screen's edges land on the view `eye` sees from `look`,
    /// fractions from its top left: `[left, top, right, bottom]`.
    fn seen(look: &Look, eye: &Eye) -> [f64; 4] {
        let screen = screen();
        let half_tall = libm::tan(eye.fov_y / 2.0);
        let on_view = |point: DVec3| {
            let seen = look.rotation.inverse() * (point - look.position);
            let ndc = [
                seen.x / -seen.z / (half_tall * eye.aspect),
                seen.y / -seen.z / half_tall,
            ];
            [(ndc[0] + 1.0) / 2.0, (1.0 - ndc[1]) / 2.0]
        };
        let [left, top] = on_view(screen.transform_point3(DVec3::new(-0.5, 0.5, 0.0)));
        let [right, bottom] = on_view(screen.transform_point3(DVec3::new(0.5, -0.5, 0.0)));
        [left, top, right, bottom]
    }

    #[test]
    fn the_look_faces_the_screen_and_fills_the_clear_part() {
        // A wide view whose middle is left clear, seven tenths of it, raised.
        let wide = eye(16.0 / 9.0, [0.15, 0.09, 0.85, 0.79]);
        let look = facing(screen(), &wide).unwrap();
        // In front of it, looking straight at its face, upright.
        assert!(look.position.y > 0.0);
        let ahead = look.rotation * DVec3::NEG_Z;
        assert!(ahead.dot(DVec3::NEG_Y) > 0.9999, "{ahead}");
        assert!((look.rotation * DVec3::Y).z > 0.99, "upright");
        // Bound by its height, it fills the clear part down, its middle
        // on the clear part's.
        let [left, top, right, bottom] = seen(&look, &wide);
        assert!(
            (top - 0.09).abs() < 1e-9 && (bottom - 0.79).abs() < 1e-9,
            "{top} {bottom}"
        );
        assert!(((left + right) / 2.0 - 0.5).abs() < 1e-9);
        assert!(right - left < 0.7);
    }

    #[test]
    fn a_phone_upright_fills_the_width_over_its_pad() {
        // 390 by 844, a pad over the lowest 260: bound by its width.
        let upright = eye(390.0 / 844.0, [0.0, 0.0, 1.0, 584.0 / 844.0]);
        let look = facing(screen(), &upright).unwrap();
        let [left, top, right, bottom] = seen(&look, &upright);
        assert!(
            left.abs() < 1e-9 && (right - 1.0).abs() < 1e-9,
            "{left} {right}"
        );
        assert!(
            ((top + bottom) / 2.0 - 292.0 / 844.0).abs() < 1e-9,
            "{top} {bottom}"
        );
        // Sideways, the pad at its two sides, 544 clear between: bound by
        // its height.
        let sideways = eye(844.0 / 390.0, [150.0 / 844.0, 0.0, 694.0 / 844.0, 1.0]);
        let look = facing(screen(), &sideways).unwrap();
        let [left, top, right, bottom] = seen(&look, &sideways);
        assert!(
            top.abs() < 1e-9 && (bottom - 1.0).abs() < 1e-9,
            "{top} {bottom}"
        );
        assert!(((left + right) / 2.0 - 0.5).abs() < 1e-9);
    }

    /// A client before a cabinet it may play, its hand put away.
    fn before_a_cabinet() -> (Client, Input) {
        let mut client = client();
        games(&mut client);
        placed(&mut client);
        client.command_json(r#"{"type":"items.edit","on":false}"#);
        let mut input = Input::default();
        update(&mut client, &mut input);
        (client, input)
    }

    fn cabinet_of(client: &Client) -> PartId {
        client
            .parts()
            .all()
            .find(|(_, _, part)| matches!(part.shape, Shape::Cabinet(_)))
            .map(|(id, ..)| id)
            .unwrap()
    }

    fn feet(client: &mut Client) -> SurfacePoint {
        client.host("items").feet().point
    }

    #[test]
    fn played_the_body_steps_to_player_one_s_station_facing_the_screen_and_stays_there_left() {
        let (mut client, mut input) = before_a_cabinet();
        let id = cabinet_of(&client);
        let host = client.host("items");
        let world = host.part_transform(id).unwrap();
        let station = cabinet::station(0).unwrap();
        let spot = world.transform_point3(station.feet);
        let toward = world.transform_vector3(station.facing);
        let up = host.body().up;
        let flat = |v: DVec3| v - up * up.dot(v);
        assert!(
            flat(host.body().position - spot).length() > 0.5,
            "it stood away"
        );

        client.command_json(r#"{"type":"act"}"#);
        update(&mut client, &mut input);
        assert!(client.controls_lent());
        let body = client.host("items").body();
        let off_m = flat(body.position - spot).length();
        assert!(off_m < 0.05, "{off_m} metres off its spot");
        let square = body.facing.dot(flat(toward).normalize());
        assert!(square > 0.9999, "faces the screen: {square}");

        // Left, it stands where it played, free.
        client.command_json(r#"{"type":"leave"}"#);
        for _ in 0..30 {
            update(&mut client, &mut input);
        }
        assert!(!client.controls_lent());
        let off_m = flat(client.host("items").body().position - spot).length();
        assert!(off_m < 0.05, "{off_m} metres off where it played");
    }

    #[test]
    fn with_no_world_a_game_of_two_pads_plays_at_once_from_player_one_s_station() {
        let mut client = client();
        games(&mut client);
        let cover = placed(&mut client);
        client.running_plays(cover, 2);
        let mut input = Input::default();
        client.command_json(r#"{"type":"items.edit","on":false}"#);
        update(&mut client, &mut input);
        client.command_json(r#"{"type":"act"}"#);
        update(&mut client, &mut input);
        shown(&mut client).expect("the game opens");
        input.key(Key::Right, true);
        let mut fed = Vec::new();
        for _ in 0..60 {
            update(&mut client, &mut input);
            for run in client.drain_running() {
                if let Running::Feed { buttons, .. } = run {
                    fed.extend(buttons);
                }
            }
        }
        assert!(fed.len() > 50, "it runs at its pace: {} updates", fed.len());
        let pad = |pad| {
            fed.iter()
                .fold(0, |all, &word| all | Pad::of_pad(word, pad).bits())
        };
        assert_ne!(pad(0), 0, "player one's pad is the player's");
        assert_eq!(pad(1), 0, "player two's is released");
    }

    #[test]
    fn a_station_walled_off_leaves_the_body_where_it_stood_and_the_game_plays() {
        // Where the body steps to, as a twin finds by playing.
        let (mut twin, mut input) = before_a_cabinet();
        let from = feet(&mut twin);
        twin.command_json(r#"{"type":"act"}"#);
        update(&mut twin, &mut input);
        let to = feet(&mut twin);
        assert_ne!(from, to);

        // A wall halfway there, two metres tall.
        let (mut client, mut input) = before_a_cabinet();
        assert_eq!(feet(&mut client), from);
        let h = client.parts().all().next().unwrap().2.pose.at[2];
        let [u, v] = [(from.u + to.u) / 2.0, (from.v + to.v) / 2.0];
        client.command_json(r#"{"type":"items.take","item":"cube","ahead":true}"#);
        client.command_json(&format!(
            r#"{{"type":"items.set","at":[{u},{v},{h}],"scale":[1,1,4]}}"#
        ));
        client.command_json(r#"{"type":"items.edit","on":false}"#);
        update(&mut client, &mut input);
        client.command_json(r#"{"type":"act"}"#);
        update(&mut client, &mut input);
        assert!(client.controls_lent(), "the game plays");
        assert_eq!(feet(&mut client), from, "the body stood still");
    }

    #[test]
    fn a_cabinet_tipped_more_on_a_side_than_standing_has_no_station_and_plays_where_one_stands() {
        let mut client = client();
        games(&mut client);
        placed(&mut client);
        client.command_json(r#"{"type":"items.set","rotation":[-60,0,0]}"#);
        client.command_json(r#"{"type":"items.edit","on":false}"#);
        let mut input = Input::default();
        update(&mut client, &mut input);
        let from = feet(&mut client);
        client.command_json(r#"{"type":"act"}"#);
        update(&mut client, &mut input);
        assert!(client.controls_lent(), "the game plays");
        assert_eq!(feet(&mut client), from);
    }

    #[test]
    fn played_here_its_sticks_and_buttons_are_drawn_apart_and_move_with_what_is_held() {
        let (mut client, mut input) = before_a_cabinet();
        let stations = (0..).take_while(|&i| cabinet::station(i).is_some()).count();
        let idle = client.update(1.0 / 60.0, &mut input).models;
        client.command_json(r#"{"type":"act"}"#);
        update(&mut client, &mut input);
        update(&mut client, &mut input);
        let rest = client.update(1.0 / 60.0, &mut input).models;
        // A stick and three buttons a station, over the body bare of them.
        assert_eq!(rest.len(), idle.len() + 4 * stations);
        // Held left, player 1's stick leans, and nothing else moves.
        input.key(client::Key::Left, true);
        update(&mut client, &mut input);
        update(&mut client, &mut input);
        let held = client.update(1.0 / 60.0, &mut input).models;
        let moved: Vec<usize> = (0..rest.len())
            .filter(|&i| held[i].transform != rest[i].transform)
            .collect();
        assert_eq!(moved.len(), 1, "{moved:?}");
        assert_eq!(held[moved[0]].mesh, rest[moved[0]].mesh);
        // Left, the cabinet is drawn whole again.
        client.command_json(r#"{"type":"leave"}"#);
        update(&mut client, &mut input);
        update(&mut client, &mut input);
        assert_eq!(
            client.update(1.0 / 60.0, &mut input).models.len(),
            idle.len()
        );
    }
}
