//! Duplicating: the part picked copied whole (`Part::copy`) and placed
//! beside itself, one place of a part like any other, so the world checks
//! it, keeps it and takes it back as one change. The copy is picked, so the
//! next duplicate chains.
//!
//! Where a copy stands repeats the step from the last part duplicated to
//! its copy, as that copy stands now: moved by a handle or by the panel
//! after it was made, it sets the step for the next, so a row, a stack or
//! a stair grows by one step a press. With no such step, the copy stands
//! beside its part along the part's own `x`, touching it.

use client::{Host, PartId, PartPose, Seat};
use glam::{DQuat, DVec3};

use crate::{Items, Refusal, refuse};

/// The last part duplicated and its copy, by their names here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Chain {
    original: PartId,
    copy: PartId,
}

/// Under this many blocks a step is none: the copy was moved back onto
/// its part.
const STILL: f64 = 1e-6;

impl Items {
    /// Duplicates the part named, or the one picked, and picks the copy.
    pub(crate) fn duplicate(&mut self, id: Option<PartId>, host: &mut Host<'_>) {
        if !host.may_change() {
            return refuse(host, Refusal::Level);
        }
        if let Some(id) = id {
            if !self.editing {
                self.edit(true, host);
            }
            self.item = None;
            // A name nothing holds is refused by the pick.
            self.pick(Some(id), host);
            if self.picked.is_none() {
                return;
            }
        }
        let Some(gizmo) = &self.picked else {
            return refuse(host, Refusal::Gone);
        };
        let (id, seat, part) = (gizmo.id, gizmo.seat, gizmo.part);
        let step = self.step(id, seat, &part.pose, host);
        let mut copy = part.copy();
        copy.pose.at = (DVec3::from(part.pose.at) + step).to_array();
        let Some((seat, pose)) = host.settle(seat, &copy.pose) else {
            return refuse(host, Refusal::Pose);
        };
        copy.pose = pose;
        match host.place_part(seat, copy) {
            Ok(copied) => {
                self.chain = Some(Chain {
                    original: id,
                    copy: copied,
                });
                self.pick(Some(copied), host);
            }
            Err(refusal) => refuse(host, refusal.into()),
        }
    }

    /// The step from a part to its copy, in blocks in the flat frame of its
    /// seat: the one from the last part duplicated to this, its copy, when
    /// both stand on this seat and apart; else the part's own length along
    /// its own `x`.
    fn step(&self, id: PartId, seat: Seat, pose: &PartPose, host: &Host<'_>) -> DVec3 {
        let repeated = self
            .chain
            .filter(|chain| chain.copy == id)
            .and_then(|chain| host.parts().get(chain.original))
            .filter(|(original_seat, _)| *original_seat == seat)
            .map(|(_, original)| DVec3::from(pose.at) - DVec3::from(original.pose.at))
            .filter(|step| step.length() > STILL);
        repeated.unwrap_or_else(|| beside(pose.rotation, pose.scale))
    }
}

/// A part's own length along its own `x`, in the flat frame of its seat.
fn beside(rotation: DQuat, scale: [f64; 3]) -> DVec3 {
    rotation * DVec3::X * scale[0]
}

#[cfg(test)]
mod tests {
    use client::{Chord, Client, Event, Input, Level, Part, PartId, Shape};
    use glam::DVec3;
    use serde_json::Value;

    use crate::tests::{click, client, events, games, one_voxel, running, update};

    /// Every part held, the first placed first.
    fn parts(client: &Client) -> Vec<(PartId, Part)> {
        client
            .parts()
            .all()
            .map(|(id, _, part)| (id, *part))
            .collect()
    }

    /// The part last said picked.
    fn picked(said: &[Value]) -> Option<u64> {
        said.iter()
            .rev()
            .find(|e| e["type"] == "items.picked")
            .and_then(|e| e["id"].as_u64())
    }

    fn duplicate(client: &mut Client) {
        client.command_json(r#"{"type":"items.duplicate"}"#);
    }

    fn at(part: &Part) -> DVec3 {
        DVec3::from(part.pose.at)
    }

    /// A cube placed and turned, picked.
    fn turned_cube() -> Client {
        let mut client = client();
        client.command_json(r#"{"type":"items.take","item":"cube"}"#);
        click(&mut client, [0.5, 0.7]);
        client.command_json(r#"{"type":"items.set","rotation":[0,0,30],"scale":[4,1,3]}"#);
        events(&mut client);
        client
    }

    #[test]
    fn a_part_duplicated_three_times_stands_in_a_row_and_goes_back_one_at_a_time() {
        let mut client = turned_cube();
        let (_, original) = parts(&client)[0];
        for _ in 0..3 {
            duplicate(&mut client);
        }
        let all = parts(&client);
        assert_eq!(all.len(), 4);
        // Each beside the last along its own x, touching it: its own length.
        let step = original.pose.rotation * DVec3::X * 4.0;
        for (k, (_, part)) in all.iter().enumerate() {
            let want = at(&original) + step * k as f64;
            assert!((at(part) - want).length() < 1e-9, "{k}: {:?}", part.pose);
            assert_eq!(
                Part {
                    pose: original.pose,
                    ..*part
                },
                original,
                "{k}: the whole record but where it stands"
            );
            assert_eq!(part.pose.rotation, original.pose.rotation);
            assert_eq!(part.pose.scale, original.pose.scale);
        }
        // The copy is picked each time, so the next chains from it.
        assert_eq!(picked(&events(&mut client)), Some(all[3].0.0));
        // One change a copy: taken back one at a time, and put back.
        for left in [3, 2, 1] {
            client.take_back();
            assert_eq!(parts(&client).len(), left);
        }
        assert_eq!(parts(&client)[0].1, original);
        client.put_back();
        assert_eq!(parts(&client).len(), 2);
        assert!((at(&parts(&client)[1].1) - (at(&original) + step)).length() < 1e-9);
    }

    #[test]
    fn a_copy_moved_sets_the_step_the_next_repeats() {
        let mut client = turned_cube();
        let (_, original) = parts(&client)[0];
        duplicate(&mut client);
        // The copy moved by the panel to stand three blocks over its part:
        // the next copies stack by that step.
        let over = at(&original) + DVec3::new(0.0, 0.0, 3.0);
        client.command_json(&format!(
            r#"{{"type":"items.set","at":[{},{},{}]}}"#,
            over.x, over.y, over.z
        ));
        duplicate(&mut client);
        duplicate(&mut client);
        let heights: Vec<f64> = parts(&client).iter().map(|(_, p)| p.pose.at[2]).collect();
        let h = original.pose.at[2];
        assert_eq!(heights, [h, h + 3.0, h + 6.0, h + 9.0]);
        // Another part picked starts again from beside it.
        let first = parts(&client)[0].0;
        client.command_json(&format!(r#"{{"type":"items.pick","id":{}}}"#, first.0));
        duplicate(&mut client);
        let last = parts(&client).last().unwrap().1;
        let beside = at(&original) + original.pose.rotation * DVec3::X * 4.0;
        assert!((at(&last) - beside).length() < 1e-9);
    }

    #[test]
    fn command_d_duplicates_and_d_alone_walks() {
        let mut client = turned_cube();
        let mut input = Input::default();
        input.code("KeyD", Chord::default(), true);
        update(&mut client, &mut input);
        input.code("KeyD", Chord::default(), false);
        assert_eq!(parts(&client).len(), 1, "D alone is no duplicate");
        let command = Chord {
            command: true,
            shift: false,
        };
        assert!(
            client.asks("KeyD", command),
            "the world takes it from the page"
        );
        input.code("KeyD", command, true);
        update(&mut client, &mut input);
        assert_eq!(parts(&client).len(), 2);
    }

    #[test]
    fn a_session_below_a_builder_copies_nothing() {
        let mut client = turned_cube();
        let id = parts(&client)[0].0;
        client.rehearse(Level::SignedIn);
        events(&mut client);
        client.command_json(&format!(r#"{{"type":"items.duplicate","id":{}}}"#, id.0));
        assert_eq!(events(&mut client).last().unwrap()["reason"], "level");
        assert_eq!(parts(&client).len(), 1);
    }

    #[test]
    fn an_agent_duplicates_a_part_by_its_name_and_nothing_gone() {
        let mut client = turned_cube();
        let id = parts(&client)[0].0;
        client.command_json(r#"{"type":"items.edit","on":false}"#);
        events(&mut client);
        client.command_json(&format!(r#"{{"type":"items.duplicate","id":{}}}"#, id.0));
        let all = parts(&client);
        assert_eq!(all.len(), 2);
        assert_eq!(picked(&events(&mut client)), Some(all[1].0.0));
        client.command_json(r#"{"type":"items.duplicate","id":999}"#);
        let said = events(&mut client);
        let refused = said.iter().find(|e| e["type"] == "items.refused");
        assert_eq!(refused.unwrap()["reason"], "gone");
        assert_eq!(parts(&client).len(), 2);
    }

    /// Each kind by its rule: a name moves on, a portal leads where its
    /// part does, a file is named by the same hash and asked for no more.
    #[test]
    fn each_kind_is_copied_by_its_rule() {
        let mut client = client();
        let named = |client: &Client, k: usize| match parts(client)[k].1.shape {
            Shape::Landmark(name) => name.to_string(),
            shape => panic!("{shape:?}"),
        };
        client.command_json(
            r#"{"type":"items.take","item":"landmark","name":"Espaço TRI","ahead":true}"#,
        );
        let star = parts(&client)[0].0;
        duplicate(&mut client);
        duplicate(&mut client);
        assert_eq!(named(&client, 1), "Espaço TRI 2");
        assert_eq!(named(&client, 2), "Espaço TRI 3");
        // A portal's copy leads to the same star.
        client.command_json(&format!(
            r#"{{"type":"items.take","item":"portal","to":{},"ahead":true}}"#,
            star.0
        ));
        duplicate(&mut client);
        let all = parts(&client);
        let [.., (_, portal), (_, copy)] = all.as_slice() else {
            panic!()
        };
        assert!(matches!(portal.shape, Shape::Portal(p) if p.to == Some(star)));
        assert_eq!(copy.shape, portal.shape);
        // A picture, a sound and a model name their files by the same hash.
        let hex = |byte: &str| byte.repeat(32);
        for take in [
            format!(
                r#"{{"type":"items.take","item":"picture","media":{{"width":3,"height":2,"thumb":"asset://{}.webp","low":"asset://{}.webp","full":"asset://{}.jpg"}},"ahead":true}}"#,
                hex("01"),
                hex("02"),
                hex("03")
            ),
            format!(
                r#"{{"type":"items.take","item":"sound","clip":{{"file":"asset://{}.mp3","millis":4000}},"ahead":true}}"#,
                hex("04")
            ),
        ] {
            client.command_json(&take);
            let before = parts(&client).len();
            duplicate(&mut client);
            let all = parts(&client);
            assert_eq!(all.len(), before + 1);
            assert_eq!(all[before].1.shape, all[before - 1].1.shape);
        }
        client.drain_asset_requests();
        let asset = format!("asset://{}.vox", hex("ab"));
        client.command_json(&format!(
            r#"{{"type":"items.take","item":"model","asset":"{asset}","ahead":true}}"#
        ));
        let asked = client.drain_asset_requests();
        let asked = asked.iter().find(|r| r.path == asset).expect("asked once");
        client.asset_loaded(asked.id, Ok(one_voxel()));
        update(&mut client, &mut Input::default());
        let before = parts(&client).len();
        duplicate(&mut client);
        update(&mut client, &mut Input::default());
        let all = parts(&client);
        assert_eq!(all[before].1.shape, all[before - 1].1.shape);
        assert!(
            client
                .drain_asset_requests()
                .iter()
                .all(|r| r.path != asset),
            "its file is held: nothing is asked again"
        );
        assert!(
            !client
                .drain_events()
                .iter()
                .any(|e| matches!(e, Event::Rejected { .. }))
        );
    }

    /// A cabinet's copy holds the same cartridge, and the game played on
    /// its part goes on alone: the copy shows the cartridge's cover, one
    /// picture every idle cabinet of it shares, until it is played.
    #[test]
    fn a_cabinet_copied_while_played_shows_the_cover_and_its_game_goes_on() {
        let mut client = client();
        games(&mut client);
        let game = running(&mut client);
        let id = parts(&client)[0].0;
        client.command_json(&format!(r#"{{"type":"items.duplicate","id":{}}}"#, id.0));
        let all = parts(&client);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].1.shape, all[0].1.shape);
        let mut input = Input::default();
        for _ in 0..30 {
            update(&mut client, &mut input);
            for run in client.drain_running() {
                match run {
                    client::Running::Close(picture) => assert_ne!(picture, game, "still played"),
                    client::Running::Open { .. } => panic!("the copy runs no game of its own"),
                    _ => {}
                }
            }
        }
        let frame = client.update(1.0 / 60.0, &mut input);
        let drawn: Vec<_> = frame.pictures.iter().filter_map(|d| d.picture).collect();
        assert!(drawn.contains(&game), "the game runs on its cabinet");
        assert!(drawn.iter().any(|p| *p != game), "the copy shows the cover");
    }
}
