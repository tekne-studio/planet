//! The pets of a world over a room a test holds: two accounts and a
//! visitor, one riding a horse.

use topology::{BLOCK_M, Sector, SurfacePoint};
use worldgen::{Generator, Recipe};

use super::*;
use crate::Moment;
use crate::stage::Stage;

/// A horse's number, and another's.
const BAY: u64 = (1 << 45) | 7;
const GREY: u64 = (1 << 45) | 8;

fn spot(point: SurfacePoint, ahead_m: f64) -> wire::Spot {
    wire::Spot {
        sector: point.sector.index() as u32,
        u: point.u + ahead_m / BLOCK_M,
        v: point.v,
        heading: 0.5,
        at_ms: 1,
    }
}

fn body(session: u32, user: &str, point: SurfacePoint, carrier: u64) -> Who {
    Who {
        session,
        user: user.to_owned(),
        name: format!("body {session}"),
        level: Level::Anonymous,
        stance: Some(Stance {
            sector: point.sector.index() as u32,
            u: point.u as f32,
            v: point.v as f32,
            height_m: 0.0,
            carrier,
            ..Default::default()
        }),
    }
}

/// A world with Ana riding the bay, Bo beside her on foot, and a visitor.
fn stage() -> (Stage, SurfacePoint) {
    let generator = Generator::new(Recipe::new(1)).unwrap();
    let side = f64::from(generator.sphere().blocks().side());
    let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
    let stage = Stage {
        here: vec![
            body(1, "ana", point, BAY),
            body(2, "bo", point, 0),
            body(3, "", point, 0),
        ],
        now: Moment(5_000),
        ..Stage::of(generator, true)
    };
    (stage, point)
}

/// Asks an op as the session of that number, and says the code it was
/// refused with, if it was.
fn ask<M: Message>(
    life: &mut Life,
    stage: &mut Stage,
    session: u32,
    kind: &str,
    op: &M,
) -> Option<String> {
    stage.refused = None;
    let who = stage
        .here
        .iter()
        .find(|who| who.session == session)
        .cloned();
    life.op(kind, &op.encode_to_vec(), &who.unwrap(), stage);
    stage.refused.clone()
}

/// The last pet told to a session.
fn told_to(stage: &Stage, session: u32) -> Option<wire::Pet> {
    let mut told = stage.told.iter().rev();
    let (_, payload, _) = told.find(|(kind, _, to)| kind == wire::PET && to.contains(&session))?;
    wire::Told::decode(payload.as_slice()).ok()?.pet
}

fn tame(number: u64, point: SurfacePoint) -> wire::Tame {
    wire::Tame {
        number,
        spot: Some(spot(point, 1.0)),
    }
}

#[test]
fn a_body_tames_the_animal_it_rides_and_names_it() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    let refused = ask(&mut life, &mut stage, 1, wire::TAME, &tame(GREY, point));
    assert_eq!(refused.as_deref(), Some("riding"), "she rides the bay");
    let far = wire::Tame {
        spot: Some(spot(point, 40.0)),
        ..tame(BAY, point)
    };
    let refused = ask(&mut life, &mut stage, 1, wire::TAME, &far);
    assert_eq!(refused.as_deref(), Some("far"));

    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    let hers = told_to(&stage, 1).unwrap();
    assert!(hers.mine && hers.number == BAY && hers.name.is_empty());
    assert_eq!(hers.spot.unwrap().at_ms, 5_000, "on the server's clock");
    let theirs = told_to(&stage, 2).unwrap();
    assert!(!theirs.mine && theirs.owner == "body 1" && theirs.session == 0);
    assert!(stage.store.contains_key(&pet_key("ana")));

    for bad in [
        "",
        "   ",
        "a\u{7}b",
        &"x".repeat(gaia::fauna::NAME_BYTES + 1),
    ] {
        let name = wire::Name { name: bad.into() };
        let refused = ask(&mut life, &mut stage, 1, wire::NAME, &name);
        assert_eq!(refused.as_deref(), Some("name"), "{bad:?}");
    }
    let name = wire::Name {
        name: "  Bucephalus ".into(),
    };
    assert_eq!(ask(&mut life, &mut stage, 1, wire::NAME, &name), None);
    assert_eq!(told_to(&stage, 2).unwrap().name, "Bucephalus");
    let refused = ask(&mut life, &mut stage, 2, wire::NAME, &name);
    assert_eq!(refused.as_deref(), Some("none"), "Bo has no pet");
}

#[test]
fn nobody_tames_what_another_rides_or_keeps() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    stage.here[1] = body(2, "bo", point, BAY);
    let refused = ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point));
    assert_eq!(refused.as_deref(), Some("ridden"), "two claimants");

    stage.here[1] = body(2, "bo", point, 0);
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    stage.here[0] = body(1, "ana", point, 0);
    stage.here[1] = body(2, "bo", point, BAY);
    let refused = ask(&mut life, &mut stage, 2, wire::TAME, &tame(BAY, point));
    assert_eq!(refused.as_deref(), Some("anothers"));
}

#[test]
fn taming_another_frees_the_first() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    stage.here[0] = body(1, "ana", point, GREY);
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(GREY, point)),
        None
    );
    let freed = stage.told.iter().find(|(kind, _, _)| kind == wire::FREED);
    let (_, payload, to) = freed.expect("the bay is freed");
    assert_eq!(wire::Freed::decode(payload.as_slice()).unwrap().number, BAY);
    assert_eq!(to, &[1, 2, 3]);
    assert_eq!(told_to(&stage, 2).unwrap().number, GREY);

    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::RELEASE, &wire::Release {}),
        None
    );
    assert!(!stage.store.contains_key(&pet_key("ana")));
    let refused = ask(&mut life, &mut stage, 1, wire::RELEASE, &wire::Release {});
    assert_eq!(refused.as_deref(), Some("none"));
}

#[test]
fn a_pet_is_kept_across_instances_and_told_to_whoever_asks() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    let left = wire::Leave {
        spot: Some(spot(point, 2.0)),
    };
    assert_eq!(ask(&mut life, &mut stage, 1, wire::LEAVE, &left), None);

    // A new instance reads what the store kept.
    let mut life = Life::default();
    stage.told.clear();
    for session in [1, 2] {
        assert_eq!(
            ask(&mut life, &mut stage, session, wire::PETS, &wire::Ask {}),
            None
        );
    }
    let answers: Vec<wire::Pets> = stage
        .told
        .iter()
        .map(|(kind, payload, _)| {
            assert_eq!(kind, wire::PETS);
            wire::Pets::decode(payload.as_slice()).unwrap()
        })
        .collect();
    let (hers, his) = (&answers[0].pets[0], &answers[1].pets[0]);
    assert!(hers.mine && !his.mine);
    assert_eq!(
        hers.spot,
        Some(spot(point, 2.0)).map(|s| wire::Spot { at_ms: 5_000, ..s })
    );
    assert_eq!(stage.told[0].2, [1], "an answer is the asker's alone");
}

#[test]
fn a_call_is_heard_within_the_whistles_reach_to_beside_the_caller() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    // Seen 300 ms before the world hears it, at 5 s, going 4 m/s.
    let seen = |spot: wire::Spot| wire::Spot {
        at_ms: 4_700,
        ..spot
    };
    let call = |from_m: f64, to_m: f64| wire::Call {
        from: Some(seen(spot(point, from_m))),
        to: Some(seen(spot(point, to_m))),
        enter: None,
        from_mps: 4.0,
    };
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call(20.0, 1.0));
    assert_eq!(refused.as_deref(), Some("none"));
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call(1.0, 30.0));
    assert_eq!(refused.as_deref(), Some("far"));
    // Said to be seen where it never went: a client cannot call it from
    // anywhere by saying so.
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call(140.0, 1.5));
    assert_eq!(refused.as_deref(), Some("unheard"), "it stands a metre off");

    // Ana walks off: 140 m away it hears her, 5 m past earshot it does not.
    let walked = |ahead_m: f64| SurfacePoint {
        u: point.u + ahead_m / BLOCK_M,
        ..point
    };
    let far = gaia::fauna::HEARS_M + 5.0;
    stage.here[0] = body(1, "ana", walked(far), 0);
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call(1.0, far));
    assert_eq!(refused.as_deref(), Some("unheard"));
    stage.here[0] = body(1, "ana", walked(140.0), 0);
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::CALL, &call(1.0, 141.0)),
        None
    );
    // It sets off from where it was seen, when it was seen there, at the
    // speed it had: a peer draws it on from there.
    let called = told_to(&stage, 2).unwrap();
    assert_eq!(called.from, Some(seen(spot(point, 1.0))));
    assert_eq!(called.spot, Some(seen(spot(point, 141.0))));
    assert_eq!(called.from_mps, 4.0);
    // Seen long before or after the world hears it: within a second
    // before now.
    let at = |at_ms: u64| {
        let stamped = |spot: wire::Spot| wire::Spot { at_ms, ..spot };
        wire::Call {
            from: Some(stamped(spot(point, 1.0))),
            to: Some(stamped(spot(point, 141.0))),
            ..call(1.0, 141.0)
        }
    };
    for (said_ms, kept_ms) in [(0, 4_000), (9_000, 5_000)] {
        assert_eq!(
            ask(&mut life, &mut stage, 1, wire::CALL, &at(said_ms)),
            None
        );
        let called = told_to(&stage, 2).unwrap();
        assert_eq!(called.spot.map(|spot| spot.at_ms), Some(kept_ms));
    }
    // A speed no animal sets off at is no call.
    for from_mps in [-1.0, f64::NAN, gaia::fauna::FASTEST_MPS + 1.0] {
        let wild = wire::Call {
            from_mps,
            ..call(1.0, 141.0)
        };
        let refused = ask(&mut life, &mut stage, 1, wire::CALL, &wild);
        assert_eq!(refused.as_deref(), Some("message"));
    }

    // Called again on its way, it is seen anywhere along it.
    stage.here[0] = body(1, "ana", point, 0);
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::CALL, &call(70.0, 1.0)),
        None
    );
}

#[test]
fn a_call_from_past_the_reach_brings_it_out_of_sight_into_sight_near_the_caller() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    // Ana walks 400 m off, past earshot and within how far a pet answers,
    // and whistles.
    let away_m = 400.0;
    let there = SurfacePoint {
        u: point.u + away_m / BLOCK_M,
        ..point
    };
    stage.here[0] = body(1, "ana", there, 0);
    let call = |enter_m: f64, to_m: f64| wire::Call {
        from: None,
        to: Some(spot(there, to_m)),
        enter: Some(spot(there, enter_m)),
        from_mps: 0.0,
    };
    let both = wire::Call {
        from: Some(spot(there, -40.0)),
        ..call(-40.0, 1.5)
    };
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &both);
    assert_eq!(
        refused.as_deref(),
        Some("message"),
        "seen or coming, not both"
    );
    let far_off = gaia::fauna::COMES_M + NEAR_M + 2.0;
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call(-far_off, 1.5));
    assert_eq!(
        refused.as_deref(),
        Some("far"),
        "it comes into sight near her"
    );
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call(-40.0, 30.0));
    assert_eq!(refused.as_deref(), Some("far"), "it stands beside her");
    // Bo has no pet to call.
    stage.here[1] = body(2, "bo", there, 0);
    let refused = ask(&mut life, &mut stage, 2, wire::CALL, &call(-40.0, 1.5));
    assert_eq!(refused.as_deref(), Some("none"));

    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::CALL, &call(-40.0, 1.5)),
        None
    );
    // It left the bay's spot as she whistled, and comes into sight 40 m
    // behind her when the wait for 400 m is over.
    let called = told_to(&stage, 2).unwrap();
    let from = called.from.unwrap();
    let to = called.spot.unwrap();
    assert_eq!((from.u, from.at_ms), (spot(there, -40.0).u, 5_000));
    assert_eq!(to.u, spot(there, 1.5).u);
    // 400 m along the planet, as near as a sector's blocks say.
    let wait_ms = |far_m: f64| (gaia::fauna::coming_s(far_m) * 1000.0).round() as u64;
    let waited_ms = to.at_ms - from.at_ms;
    assert!((wait_ms(0.9 * away_m)..=wait_ms(1.1 * away_m)).contains(&waited_ms));
    let kept = life
        .pet_of(&Keeper::Account("ana".into()), &mut stage)
        .unwrap();
    assert_eq!(
        kept,
        wire::Pet {
            mine: false,
            ..called
        }
    );
}

/// From the moon nothing a pet does is asked: its spots are the planet's,
/// and no measure of the moon's says how far they are.
#[test]
fn a_pet_is_not_called_tamed_or_left_from_the_moon() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    // Ana on the moon, over the same sector and blocks as the bay's spot.
    let mut ana = body(1, "ana", point, 0);
    ana.stance.as_mut().unwrap().set_body(protocol::Body::Moon);
    stage.here[0] = ana;
    let call = wire::Call {
        from: None,
        to: Some(spot(point, 1.5)),
        enter: Some(spot(point, -40.0)),
        from_mps: 0.0,
    };
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call);
    assert_eq!(refused.as_deref(), Some("far"), "called");
    let seen = wire::Call {
        from: Some(spot(point, 0.0)),
        enter: None,
        from_mps: 0.0,
        ..call
    };
    let refused = ask(&mut life, &mut stage, 1, wire::CALL, &seen);
    assert_eq!(refused.as_deref(), Some("far"), "called in sight");
    let refused = ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY + 1, point));
    assert!(refused.is_some(), "tamed");
}

#[test]
fn a_pet_answers_a_whistle_within_its_reach_and_no_farther() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    // Ana whistles from `away_m` off, the bay at its spot each time.
    let mut whistle = |away_m: f64| {
        let there = SurfacePoint {
            u: point.u + away_m / BLOCK_M,
            ..point
        };
        stage.here[0] = body(1, "ana", there, 0);
        let call = wire::Call {
            from: None,
            to: Some(spot(there, 1.5)),
            enter: Some(spot(there, -40.0)),
            from_mps: 0.0,
        };
        let mut kept = life
            .pet_of(&Keeper::Account("ana".into()), &mut stage)
            .unwrap();
        kept.spot = Some(spot(point, 0.0));
        life.keep(Keeper::Account("ana".into()), kept, &mut stage);
        ask(&mut life, &mut stage, 1, wire::CALL, &call)
    };
    let reach = gaia::fauna::ANSWERS_M;
    assert_eq!(whistle(reach - 10.0), None, "within it, it comes");
    assert_eq!(
        whistle(reach + NEAR_M + 10.0).as_deref(),
        Some("too_far"),
        "past it, it does not"
    );
}

#[test]
fn a_visitors_pet_is_held_for_its_session_and_an_accounts_never_handed() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    let kept = wire::Keep {
        pet: Some(wire::Pet {
            number: GREY,
            name: "Dapple".into(),
            spot: Some(spot(point, 3.0)),
            ..Default::default()
        }),
    };
    let refused = ask(&mut life, &mut stage, 2, wire::KEEP, &kept);
    assert_eq!(
        refused.as_deref(),
        Some("account"),
        "an account's is the world's"
    );
    // What another rides is not handed over, as it is not tamed.
    let ridden = wire::Keep {
        pet: Some(wire::Pet {
            number: BAY,
            ..kept.pet.clone().unwrap()
        }),
    };
    let refused = ask(&mut life, &mut stage, 3, wire::KEEP, &ridden);
    assert_eq!(refused.as_deref(), Some("ridden"), "Ana rides the bay");
    // One kept in the place of another frees the first.
    let other = wire::Keep {
        pet: Some(wire::Pet {
            number: GREY + 1,
            ..kept.pet.clone().unwrap()
        }),
    };
    assert_eq!(ask(&mut life, &mut stage, 3, wire::KEEP, &other), None);
    stage.told.clear();
    assert_eq!(ask(&mut life, &mut stage, 3, wire::KEEP, &kept), None);
    let freed = stage.told.iter().find(|(kind, _, _)| kind == wire::FREED);
    let (_, payload, _) = freed.expect("the first is freed");
    assert_eq!(
        wire::Freed::decode(payload.as_slice()).unwrap().number,
        GREY + 1
    );
    let theirs = told_to(&stage, 1).unwrap();
    assert!(!theirs.mine && theirs.session == 3 && theirs.name == "Dapple");
    assert!(told_to(&stage, 3).unwrap().mine);
    assert!(stage.store.is_empty(), "the world keeps nothing of it");

    stage.here[0] = body(1, "ana", point, GREY);
    let refused = ask(&mut life, &mut stage, 1, wire::TAME, &tame(GREY, point));
    assert_eq!(refused.as_deref(), Some("anothers"));

    life.gone(3);
    stage.told.clear();
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::PETS, &wire::Ask {}),
        None
    );
    let (_, payload, _) = &stage.told[0];
    assert!(
        wire::Pets::decode(payload.as_slice())
            .unwrap()
            .pets
            .is_empty()
    );
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(GREY, point)),
        None
    );
}

#[test]
fn life_is_named_as_the_wire_names_its_owner() {
    assert_eq!(gaia::NAME, wire::OWNER);
}

/// A spot that is no place on the planet is refused by every op that
/// names one, before any session is told it.
#[test]
fn a_spot_that_is_no_place_is_refused() {
    let (mut stage, point) = stage();
    let mut life = Life::default();
    let side = f64::from(stage.measure.sphere.blocks().side());
    let good = spot(point, 1.0);
    let wild = [
        wire::Spot {
            u: f64::NAN,
            ..good
        },
        wire::Spot {
            v: f64::INFINITY,
            ..good
        },
        wire::Spot { u: -1.0, ..good },
        wire::Spot {
            v: side + 1.0,
            ..good
        },
        wire::Spot {
            heading: f64::NAN,
            ..good
        },
        wire::Spot { sector: 6, ..good },
        wire::Spot {
            sector: u32::MAX,
            ..good
        },
    ];
    for bad in wild {
        let tame = wire::Tame {
            number: BAY,
            spot: Some(bad),
        };
        let refused = ask(&mut life, &mut stage, 1, wire::TAME, &tame);
        assert_eq!(refused.as_deref(), Some("place"), "tame at {bad:?}");
        let keep = wire::Keep {
            pet: Some(wire::Pet {
                number: GREY,
                spot: Some(bad),
                ..Default::default()
            }),
        };
        let refused = ask(&mut life, &mut stage, 3, wire::KEEP, &keep);
        assert_eq!(refused.as_deref(), Some("place"), "keep at {bad:?}");
        let keep = wire::Keep {
            pet: Some(wire::Pet {
                number: GREY,
                spot: Some(good),
                from: Some(bad),
                ..Default::default()
            }),
        };
        let refused = ask(&mut life, &mut stage, 3, wire::KEEP, &keep);
        assert_eq!(refused.as_deref(), Some("place"), "keep from {bad:?}");
    }
    assert!(
        stage.told.is_empty(),
        "nobody is told of a spot that is none"
    );

    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(BAY, point)),
        None
    );
    for bad in wild {
        let leave = wire::Leave { spot: Some(bad) };
        let refused = ask(&mut life, &mut stage, 1, wire::LEAVE, &leave);
        assert_eq!(refused.as_deref(), Some("place"), "leave at {bad:?}");
        for call in [
            wire::Call {
                from: Some(bad),
                to: Some(good),
                enter: None,
                from_mps: 0.0,
            },
            wire::Call {
                from: Some(good),
                to: Some(bad),
                enter: None,
                from_mps: 0.0,
            },
            wire::Call {
                from: None,
                to: Some(good),
                enter: Some(bad),
                from_mps: 0.0,
            },
        ] {
            let refused = ask(&mut life, &mut stage, 1, wire::CALL, &call);
            assert_eq!(refused.as_deref(), Some("place"), "call {call:?}");
        }
    }
}

/// The number of a winged horse of the world's herds, and of a wingless
/// one of the same herd.
fn winged_and_not(stage: &Stage) -> (u64, &'static str, u64) {
    let seed = stage.ground().unwrap().recipe().seed;
    let sector = Sector::new(0).unwrap();
    (0..64)
        .flat_map(|i| {
            (0..64).map(move |j| Region {
                kind: 1,
                sector,
                i,
                j,
            })
        })
        .find_map(|region| {
            let (herd, _) = region.herd(seed)?;
            let winged = (0..herd.len()).find(|&place| herd.wings_of(place).is_some())?;
            let not = (0..herd.len()).find(|&place| herd.wings_of(place).is_none())?;
            let coat = herd.wings_of(winged)?;
            Some((region.number(winged), coat, region.number(not)))
        })
        .unwrap()
}

/// What the herd gives the animal of `number` now, with wings in `wings`.
fn drawn(number: u64, wings: &str, stage: &Stage) -> Traits {
    let (region, place) = Region::of(number).unwrap();
    let seed = stage.ground().unwrap().recipe().seed;
    let (herd, _) = region.herd(seed).unwrap();
    let wings = Some(wings).filter(|wings| !wings.is_empty());
    Traits::drawn(region.kind().unwrap(), herd.seed_of(place), wings)
}

/// A pet keeps what its herd gave it when it was tamed, its wings and
/// every other trait, so no later rule draws it anew; a row kept before
/// the world kept them takes its wings and traits by the rules of now,
/// kept back; a visitor's comes with its own, wings none or a coat's name
/// and traits an animal of its kind can be drawn in, else it is refused.
#[test]
fn a_pet_keeps_what_it_was_born_with() {
    let (mut stage, point) = stage();
    let (winged, coat, wingless) = winged_and_not(&stage);
    let mut life = Life::default();
    stage.here[0] = body(1, "ana", point, winged);
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(winged, point)),
        None
    );
    let told = told_to(&stage, 2).unwrap();
    assert_eq!(told.wings.as_deref(), Some(coat));
    assert_eq!(told.traits, Some(wire_of(&drawn(winged, coat, &stage))));
    assert!(traits_of(&told.traits.unwrap(), true).wings);
    stage.here[0] = body(1, "ana", point, wingless);
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(wingless, point)),
        None
    );
    let told = told_to(&stage, 2).unwrap();
    assert_eq!(told.wings.as_deref(), Some(""));
    let kept = told.traits.unwrap();
    assert_eq!(traits_of(&kept, false), drawn(wingless, "", &stage));
    // Tamed again, it keeps what it kept, whatever the herd says now.
    let mut row = wire::Pet::decode(stage.store[&pet_key("ana")].as_slice()).unwrap();
    let other = wire_of(&drawn(winged, "", &stage));
    row.traits = Some(other.clone());
    stage.store.insert(pet_key("ana"), row.encode_to_vec());
    let mut life = Life::default();
    assert_eq!(
        ask(&mut life, &mut stage, 1, wire::TAME, &tame(wingless, point)),
        None
    );
    assert_eq!(told_to(&stage, 2).unwrap().traits, Some(other));

    // An old row: read, it takes its wings and traits by the rules of now,
    // and is kept so.
    let before = wire::Pet {
        number: winged,
        spot: Some(spot(point, 2.0)),
        ..Default::default()
    };
    stage.store.insert(pet_key("bo"), before.encode_to_vec());
    let mut life = Life::default();
    assert_eq!(
        ask(&mut life, &mut stage, 2, wire::PETS, &wire::Ask {}),
        None
    );
    let row = wire::Pet::decode(stage.store[&pet_key("bo")].as_slice()).unwrap();
    assert_eq!(row.wings.as_deref(), Some(coat));
    assert_eq!(row.traits, Some(wire_of(&drawn(winged, coat, &stage))));

    // A visitor's, of the last place of the same herd's region.
    let visitors = (winged & !63) | 63;
    let kept = |wings: Option<&str>, traits: Option<wire::Traits>| wire::Keep {
        pet: Some(wire::Pet {
            number: visitors,
            spot: Some(spot(point, 3.0)),
            wings: wings.map(str::to_owned),
            traits,
            ..Default::default()
        }),
    };
    let long = "a".repeat(NAME_BYTES + 1);
    let refused = ask(
        &mut life,
        &mut stage,
        3,
        wire::KEEP,
        &kept(Some(&long), None),
    );
    assert_eq!(refused.as_deref(), Some("wings"));
    // Wings in a coat today's table no longer gives are its own still.
    let silver = wire::Traits {
        coat: "silver".into(),
        ..wire_of(&drawn(winged, "white", &stage))
    };
    let gone = kept(Some("silver"), Some(silver));
    assert_eq!(ask(&mut life, &mut stage, 3, wire::KEEP, &gone), None);
    let own = wire_of(&drawn(winged, "black", &stage));
    let handed = kept(Some("black"), Some(own.clone()));
    assert_eq!(ask(&mut life, &mut stage, 3, wire::KEEP, &handed), None);
    let told = told_to(&stage, 3).unwrap();
    assert_eq!(
        (told.wings.as_deref(), told.traits),
        (Some("black"), Some(own))
    );
    assert_eq!(
        ask(&mut life, &mut stage, 3, wire::KEEP, &kept(None, None)),
        None
    );
    let seed = stage.ground().unwrap().recipe().seed;
    let now = Region::wings(visitors, seed).unwrap_or_default();
    let told = told_to(&stage, 3).unwrap();
    assert_eq!(told.wings.as_deref(), Some(now));
    assert_eq!(told.traits, Some(wire_of(&drawn(visitors, now, &stage))));
}

/// A visitor's front end may hand any bytes: traits past what its kind
/// bears are refused, so no other session draws them.
#[test]
fn a_visitors_traits_are_checked() {
    let (mut stage, point) = stage();
    let (_, _, wingless) = winged_and_not(&stage);
    let born = wire_of(&drawn(wingless, "", &stage));
    let blot = wire::Blot {
        y: 1.2,
        z: 0.0,
        radius_m: 0.3,
    };
    let refused: [(&str, wire::Traits); 11] = [
        (
            "a voice an octave off",
            wire::Traits {
                voice_cents: Some(1200.0),
                ..born.clone()
            },
        ),
        (
            "a voice not a number",
            wire::Traits {
                voice_cents: Some(f64::NAN),
                ..born.clone()
            },
        ),
        (
            "a size not a number",
            wire::Traits {
                size: f64::NAN,
                ..born.clone()
            },
        ),
        (
            "an endless tail",
            wire::Traits {
                tail_m: f64::INFINITY,
                ..born.clone()
            },
        ),
        (
            "a missing colour",
            wire::Traits {
                hair: None,
                ..born.clone()
            },
        ),
        (
            "17 blots",
            wire::Traits {
                blots: vec![blot; 17],
                ..born.clone()
            },
        ),
        (
            "a 33-byte name",
            wire::Traits {
                face: "a".repeat(33),
                ..born.clone()
            },
        ),
        (
            "size 4",
            wire::Traits {
                size: 4.0,
                ..born.clone()
            },
        ),
        (
            "two socks",
            wire::Traits {
                socks: vec![0, 1],
                ..born.clone()
            },
        ),
        (
            "a sock of 4",
            wire::Traits {
                socks: vec![0, 4, 0, 0],
                ..born.clone()
            },
        ),
        (
            "a sock past a byte",
            wire::Traits {
                socks: vec![0, 259, 0, 0],
                ..born.clone()
            },
        ),
    ];
    let mut life = Life::default();
    for (what, traits) in refused {
        let keep = wire::Keep {
            pet: Some(wire::Pet {
                number: wingless,
                spot: Some(spot(point, 3.0)),
                wings: Some(String::new()),
                traits: Some(traits),
                ..Default::default()
            }),
        };
        let answer = ask(&mut life, &mut stage, 3, wire::KEEP, &keep);
        assert_eq!(answer.as_deref(), Some("traits"), "{what}");
    }
}

/// Traits handed without a voice, by a front end from before it was
/// drawn, are kept with the herd's: the voice of its seed now.
#[test]
fn a_voice_kept_without_is_the_herds() {
    let (mut stage, point) = stage();
    let (_, _, wingless) = winged_and_not(&stage);
    let born = wire_of(&drawn(wingless, "", &stage));
    assert!(born.voice_cents.is_some());
    let keep = wire::Keep {
        pet: Some(wire::Pet {
            number: wingless,
            spot: Some(spot(point, 3.0)),
            wings: Some(String::new()),
            traits: Some(wire::Traits {
                voice_cents: None,
                ..born.clone()
            }),
            ..Default::default()
        }),
    };
    let mut life = Life::default();
    assert_eq!(ask(&mut life, &mut stage, 3, wire::KEEP, &keep), None);
    let told = told_to(&stage, 3).unwrap();
    assert_eq!(
        told.traits.and_then(|traits| traits.voice_cents),
        born.voice_cents
    );

    // With no ground to read the herd from, it is its kind's own voice:
    // never a refusal.
    stage.ground = None;
    let mut life = Life::default();
    assert_eq!(ask(&mut life, &mut stage, 3, wire::KEEP, &keep), None);
    let told = told_to(&stage, 3).unwrap();
    assert_eq!(told.traits.and_then(|traits| traits.voice_cents), Some(0.0));
}

/// Without the world's ground to read its herd from, taming keeps no half
/// record: it is refused.
#[test]
fn no_ground_tames_nothing() {
    let (mut stage, point) = stage();
    let (winged, _, _) = winged_and_not(&stage);
    stage.ground = None;
    stage.here[0] = body(1, "ana", point, winged);
    let mut life = Life::default();
    let refused = ask(&mut life, &mut stage, 1, wire::TAME, &tame(winged, point));
    assert_eq!(refused.as_deref(), Some("ground"));
    assert!(!stage.store.contains_key(&pet_key("ana")));
}
