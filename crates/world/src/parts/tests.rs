//! The parts' system over a room a test holds: a store in memory, and
//! whoever a test puts in it.

use glam::DQuat;
use topology::{BLOCK_M, Sector, SurfacePoint};
use worldgen::{Generator, Recipe};

use super::*;
use crate::stage::Stage;

fn body(session: u32, point: SurfacePoint, level: Level) -> Who {
    Who {
        session,
        user: String::new(),
        name: format!("body {session}"),
        level,
        stance: Some(Stance {
            sector: point.sector.index() as u32,
            u: point.u as f32,
            v: point.v as f32,
            height_m: 0.0,
            ..Default::default()
        }),
    }
}

/// A world with a builder in the middle of a sector and a visitor far
/// along it.
fn stage() -> (Stage, SurfacePoint) {
    let generator = Generator::new(Recipe::new(1)).unwrap();
    let side = f64::from(generator.sphere().blocks().side());
    let point = SurfacePoint::new(Sector::new(4).unwrap(), side * 0.41, side * 0.37);
    let far = SurfacePoint::new(point.sector, point.u + 8000.0, point.v);
    let here = vec![
        body(1, point, Level::Builder),
        body(2, far, Level::Anonymous),
    ];
    let stage = Stage {
        here,
        ..Stage::of(generator, false)
    };
    (stage, point)
}

fn cube(point: SurfacePoint, h: f64) -> wire::Part {
    let part = part::Part {
        shape: part::Shape::Cube,
        pose: part::Pose {
            at: [point.u, point.v, h],
            rotation: DQuat::from_rotation_z(0.5),
            scale: [2.0, 2.0, 2.0],
        },
        paint: 11,
        solid: true,
        shine: 0.0,
        reach_m: 0.0,
        name: None,
        muted: false,
        volume: 0.0,
    };
    seat::part_wire(Seat::Sector(point.sector), &part)
}

fn landmark(point: SurfacePoint, name: &str) -> wire::Part {
    let part = part::Part {
        shape: part::Shape::Landmark(part::Name::new(name).unwrap()),
        pose: part::Pose::upright([point.u, point.v, 0.0], [2.0, 2.0, 0.16]),
        paint: 11,
        solid: false,
        shine: 0.0,
        reach_m: 0.0,
        name: None,
        muted: false,
        volume: 0.0,
    };
    seat::part_wire(Seat::Sector(point.sector), &part)
}

/// Takes back or puts back a session's last change through the owner's op.
/// Whether it landed.
fn restore(parts: &mut Parts, stage: &mut Stage, who: &Who, back: bool) -> bool {
    stage.refused = None;
    match back {
        true => parts.op(
            wire::TAKE_BACK,
            &wire::TakeBack {}.encode_to_vec(),
            who,
            stage,
        ),
        false => parts.op(
            wire::PUT_BACK,
            &wire::PutBack {}.encode_to_vec(),
            who,
            stage,
        ),
    }
    stage.refused.is_none()
}

fn told<M: Message + Default>(stage: &mut Stage, kind: &str) -> Vec<(M, Vec<u32>)> {
    let told = core::mem::take(&mut stage.told);
    told.into_iter()
        .filter(|(said, _, _)| said == kind)
        .map(|(_, payload, to)| (M::decode(payload.as_slice()).unwrap(), to))
        .collect()
}

#[test]
fn a_part_placed_is_named_told_to_who_is_near_and_kept() {
    let (mut stage, point) = stage();
    let mut parts = Parts::default();
    let who = stage.here[0].clone();
    let place = wire::Place {
        part: Some(cube(point, 4.0)),
    };
    parts.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    assert_eq!(stage.refused, None);
    let placed = told::<wire::Placed>(&mut stage, wire::PLACED);
    assert_eq!(placed.len(), 1);
    let (placed, to) = &placed[0];
    assert_eq!(
        to,
        &[1],
        "the builder near it hears it, the visitor far off does not"
    );
    let record = placed.record.as_ref().unwrap();
    assert_eq!((placed.session, record.id, record.version), (1, 1, 0));
    assert_eq!(record.part.as_ref().unwrap().paint, 11);
    let visitor = wire::By {
        name: who.name.clone(),
        user: String::new(),
        visitor: true,
    };
    assert_eq!(
        (record.placed_by.as_ref(), record.set_by.as_ref()),
        (Some(&visitor), None),
        "placed by its builder, a visitor, never set"
    );
    // Kept under its name, and the next name after it.
    assert!(stage.store.contains_key(&record_key(1)));
    assert_eq!(
        stage.store.get(NEXT_KEY).unwrap(),
        &2u64.to_be_bytes().to_vec()
    );

    // A new instance reads what stands and gives the next name, never one
    // given before, even to a part since removed.
    let mut again = Parts::default();
    let remove = wire::Remove { id: 1 };
    again.op(wire::REMOVE, &remove.encode_to_vec(), &who, &mut stage);
    assert_eq!(told::<wire::Removed>(&mut stage, wire::REMOVED)[0].0.id, 1);
    assert!(stage.scan(b"p").is_empty());
    again.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    let placed = told::<wire::Placed>(&mut stage, wire::PLACED);
    assert_eq!(placed[0].0.record.as_ref().unwrap().id, 2);
}

#[test]
fn a_part_set_counts_a_version_and_a_pose_outside_its_sector_is_refused() {
    let (mut stage, point) = stage();
    let mut parts = Parts::default();
    let who = stage.here[0].clone();
    let place = wire::Place {
        part: Some(cube(point, 4.0)),
    };
    parts.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    stage.told.clear();
    let mut moved = cube(point, 9.0);
    moved.paint = 3;
    let set = wire::Set {
        id: 1,
        part: Some(moved),
    };
    let bea = Who {
        session: 9,
        user: "account of bea".into(),
        name: "bea".into(),
        ..who.clone()
    };
    parts.op(wire::SET, &set.encode_to_vec(), &bea, &mut stage);
    assert_eq!(stage.refused, None);
    let changed = told::<wire::Changed>(&mut stage, wire::CHANGED);
    let record = changed[0].0.record.as_ref().unwrap();
    assert_eq!(
        (record.version, record.part.as_ref().unwrap().paint),
        (1, 3)
    );
    let kept = wire::Record::decode(stage.store.get(&record_key(1)).unwrap().as_slice()).unwrap();
    assert_eq!(kept.version, 1);
    // Who placed it and who set it last, kept with the account's id or as
    // a visitor's, and told whether it was a visitor's, never the id.
    let by = |name: &str, user: &str, visitor| wire::By {
        name: name.into(),
        user: user.into(),
        visitor,
    };
    assert_eq!(
        (record.placed_by.clone(), record.set_by.clone()),
        (Some(by(&who.name, "", true)), Some(by("bea", "", false)))
    );
    assert_eq!(
        (kept.placed_by, kept.set_by),
        (
            Some(by(&who.name, "", true)),
            Some(by("bea", "account of bea", false))
        )
    );

    let outside = SurfacePoint::new(point.sector, -3.0, point.v);
    let set = wire::Set {
        id: 1,
        part: Some(cube(outside, 4.0)),
    };
    parts.op(wire::SET, &set.encode_to_vec(), &who, &mut stage);
    assert_eq!(stage.refused.take(), Some("pose".into()));
    let set = wire::Set {
        id: 9,
        part: Some(cube(point, 4.0)),
    };
    parts.op(wire::SET, &set.encode_to_vec(), &who, &mut stage);
    assert_eq!(stage.refused.take(), Some("gone".into()));
    let mut flat = cube(point, 4.0);
    flat.pose.as_mut().unwrap().sx = 0.0;
    let place = wire::Place { part: Some(flat) };
    parts.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    assert_eq!(stage.refused.take(), Some("message".into()));
}

#[test]
fn a_change_is_taken_back_through_the_owner_and_put_back() {
    let (mut stage, point) = stage();
    let mut parts = Parts::default();
    let who = stage.here[0].clone();
    let place = wire::Place {
        part: Some(cube(point, 4.0)),
    };
    parts.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    let mut moved = cube(point, 9.0);
    moved.paint = 3;
    let set = wire::Set {
        id: 1,
        part: Some(moved),
    };
    parts.op(wire::SET, &set.encode_to_vec(), &who, &mut stage);
    stage.told.clear();
    // The set goes back: the part is as placed, one version on, and told.
    assert!(restore(&mut parts, &mut stage, &who, true));
    let changed = told::<wire::Changed>(&mut stage, wire::CHANGED);
    let record = changed[0].0.record.as_ref().unwrap();
    assert_eq!(
        (record.version, record.part.as_ref().unwrap().paint),
        (2, 11)
    );
    // The place goes back: the part is gone and told so.
    assert!(restore(&mut parts, &mut stage, &who, true));
    assert_eq!(told::<wire::Removed>(&mut stage, wire::REMOVED)[0].0.id, 1);
    assert!(stage.scan(b"p").is_empty());
    // Nothing more to take back.
    assert!(!restore(&mut parts, &mut stage, &who, true));
    // Put back: it stands again under its name, then the set is put back.
    assert!(restore(&mut parts, &mut stage, &who, false));
    let placed = told::<wire::Placed>(&mut stage, wire::PLACED);
    assert_eq!(placed[0].0.record.as_ref().unwrap().id, 1);
    assert!(restore(&mut parts, &mut stage, &who, false));
    let changed = told::<wire::Changed>(&mut stage, wire::CHANGED);
    assert_eq!(
        changed[0]
            .0
            .record
            .as_ref()
            .unwrap()
            .part
            .as_ref()
            .unwrap()
            .paint,
        3
    );
    assert!(!restore(&mut parts, &mut stage, &who, false));
    // Removed by someone else since: the change is gone and nothing is done.
    let other = stage.here[1].clone();
    let remove = wire::Remove { id: 1 };
    parts.op(wire::REMOVE, &remove.encode_to_vec(), &other, &mut stage);
    stage.told.clear();
    assert!(!restore(&mut parts, &mut stage, &who, true));
    assert!(stage.told.is_empty());
}

#[test]
fn a_look_is_answered_with_what_is_lacking_near_and_what_is_gone() {
    let (mut stage, point) = stage();
    let mut parts = Parts::default();
    let who = stage.here[0].clone();
    for h in [4.0, 6.0] {
        let place = wire::Place {
            part: Some(cube(point, h)),
        };
        parts.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    }
    // Along the sector, past holding distance.
    let far = SurfacePoint::new(point.sector, point.u + (HOLD_M + 512.0) / BLOCK_M, point.v);
    let place = wire::Place {
        part: Some(cube(far, 4.0)),
    };
    parts.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    stage.told.clear();
    // Holding the first as it stands, the second stale, and one that is gone.
    let look = wire::Look {
        held: vec![
            wire::Held { id: 1, version: 0 },
            wire::Held { id: 2, version: 7 },
            wire::Held { id: 40, version: 0 },
        ],
    };
    parts.op(wire::LOOK, &look.encode_to_vec(), &who, &mut stage);
    let seen = told::<wire::Seen>(&mut stage, wire::SEEN);
    let (seen, to) = &seen[0];
    assert_eq!(to, &[1]);
    let ids: Vec<u64> = seen.records.iter().map(|record| record.id).collect();
    assert_eq!(
        ids,
        vec![2],
        "the stale one near; not the held one, not the far one"
    );
    assert_eq!(seen.gone, vec![40]);
    // The visitor far off lacks everything near it: the far part alone.
    let visitor = body(2, far, Level::Anonymous);
    let look = wire::Look { held: vec![] };
    parts.op(wire::LOOK, &look.encode_to_vec(), &visitor, &mut stage);
    let seen = told::<wire::Seen>(&mut stage, wire::SEEN);
    let ids: Vec<u64> = seen[0].0.records.iter().map(|record| record.id).collect();
    assert_eq!(ids, vec![3]);
}

#[test]
fn a_chart_says_every_landmark_nearest_first_to_whoever_asks() {
    let (mut stage, point) = stage();
    let mut parts = Parts::default();
    let who = stage.here[0].clone();
    // A cube, a landmark where the visitor stands, 8000 blocks along the
    // sector, and one here: the chart says the landmarks alone, wherever
    // they stand, the nearer the asker first.
    let far = SurfacePoint::new(point.sector, point.u + 8000.0, point.v);
    for part in [
        cube(point, 4.0),
        landmark(far, "Far"),
        landmark(point, "Near"),
    ] {
        let place = wire::Place { part: Some(part) };
        parts.op(wire::PLACE, &place.encode_to_vec(), &who, &mut stage);
    }
    stage.told.clear();
    // The visitor asks: anyone may.
    let visitor = stage.here[1].clone();
    parts.op(
        wire::CHART,
        &wire::Chart {}.encode_to_vec(),
        &visitor,
        &mut stage,
    );
    assert_eq!(stage.refused, None);
    let charted = told::<wire::Charted>(&mut stage, wire::CHARTED);
    let (charted, to) = &charted[0];
    assert_eq!(to, &[2]);
    let names: Vec<String> = charted
        .records
        .iter()
        .filter_map(|record| seat::part_from_wire(record.part.as_ref()?))
        .map(|(_, part)| match part.shape {
            part::Shape::Landmark(name) => name.to_string(),
            _ => panic!("a chart is landmarks alone"),
        })
        .collect();
    assert_eq!(names, ["Far", "Near"]);
    // With none, the chart is answered empty.
    let mut empty = Parts::default();
    let mut bare = stage;
    bare.store.clear();
    bare.told.clear();
    empty.op(
        wire::CHART,
        &wire::Chart {}.encode_to_vec(),
        &who,
        &mut bare,
    );
    let charted = told::<wire::Charted>(&mut bare, wire::CHARTED);
    assert!(charted[0].0.records.is_empty());
}

#[test]
fn an_account_charts_its_home_and_favourites_first_and_a_visitor_keeps_none() {
    let (mut stage, point) = stage();
    let mut parts = Parts::default();
    let builder = stage.here[0].clone();
    // Three landmarks along the sector, the nearest first.
    for (along, name) in [(10.0, "Near"), (500.0, "Mid"), (4000.0, "Far")] {
        let at = SurfacePoint::new(point.sector, point.u + along, point.v);
        let place = wire::Place {
            part: Some(landmark(at, name)),
        };
        parts.op(wire::PLACE, &place.encode_to_vec(), &builder, &mut stage);
    }
    let id = |name: &str, stage: &Stage| {
        stage
            .store
            .values()
            .filter_map(|value| wire::Record::decode(value.as_slice()).ok())
            .find(|record| {
                record.part.as_ref().and_then(|part| part.shape.as_ref()).is_some_and(|shape| {
                    matches!(&shape.shape, Some(wire::shape::Shape::Landmark(l)) if l.name == name)
                })
            })
            .map(|record| record.id)
            .unwrap()
    };
    let (near, mid, far) = (id("Near", &stage), id("Mid", &stage), id("Far", &stage));
    let ada = Who {
        user: "ada".into(),
        level: Level::SignedIn,
        ..builder.clone()
    };
    let chart = |parts: &mut Parts, stage: &mut Stage, who: &Who| {
        stage.told.clear();
        parts.op(wire::CHART, &wire::Chart {}.encode_to_vec(), who, stage);
        let charted = told::<wire::Charted>(stage, wire::CHARTED).remove(0).0;
        let ids: Vec<u64> = charted.records.iter().map(|record| record.id).collect();
        (ids, charted.home, charted.favourites)
    };
    let op = |parts: &mut Parts, stage: &mut Stage, who: &Who, kind: &str, payload: Vec<u8>| {
        stage.refused = None;
        parts.op(kind, &payload, who, stage);
        stage.refused.take()
    };
    assert_eq!(
        chart(&mut parts, &mut stage, &ada),
        (vec![near, mid, far], 0, vec![])
    );

    // Far a favourite, Mid the home: the home first, then the favourites.
    let favour = |id, on| wire::Favour { id, on }.encode_to_vec();
    assert_eq!(
        op(
            &mut parts,
            &mut stage,
            &ada,
            wire::FAVOUR,
            favour(far, true)
        ),
        None
    );
    let home = |id| wire::Home { id }.encode_to_vec();
    assert_eq!(
        op(&mut parts, &mut stage, &ada, wire::HOME, home(mid)),
        None
    );
    assert_eq!(
        chart(&mut parts, &mut stage, &ada),
        (vec![mid, far, near], mid, vec![far])
    );

    // Another account, and a visitor, chart as before: the choices are
    // Ada's alone, and a visitor may make none.
    let bea = Who {
        user: "bea".into(),
        ..ada.clone()
    };
    assert_eq!(
        chart(&mut parts, &mut stage, &bea),
        (vec![near, mid, far], 0, vec![])
    );
    assert_eq!(
        chart(&mut parts, &mut stage, &builder),
        (vec![near, mid, far], 0, vec![])
    );
    let refused = op(&mut parts, &mut stage, &builder, wire::HOME, home(near));
    assert_eq!(refused.as_deref(), Some("visitor"));

    // A new home takes the place of the last; what is no landmark is none.
    assert_eq!(
        op(&mut parts, &mut stage, &ada, wire::HOME, home(near)),
        None
    );
    assert_eq!(
        chart(&mut parts, &mut stage, &ada),
        (vec![near, far, mid], near, vec![far])
    );
    let refused = op(
        &mut parts,
        &mut stage,
        &ada,
        wire::FAVOUR,
        favour(999, true),
    );
    assert_eq!(refused.as_deref(), Some("gone"));

    // A new instance reads them again from the store.
    let mut again = Parts::default();
    assert_eq!(
        chart(&mut again, &mut stage, &ada),
        (vec![near, far, mid], near, vec![far])
    );

    // Unfavoured and the home cleared, nothing is kept for the account.
    assert_eq!(
        op(
            &mut parts,
            &mut stage,
            &ada,
            wire::FAVOUR,
            favour(far, false)
        ),
        None
    );
    assert_eq!(op(&mut parts, &mut stage, &ada, wire::HOME, home(0)), None);
    assert_eq!(
        chart(&mut parts, &mut stage, &ada),
        (vec![near, mid, far], 0, vec![])
    );
    assert!(stage.store.keys().all(|key| key[0] != b'a'));

    // Signing in is the least level that chooses.
    let ops = parts.ops();
    let level = |kind| ops.iter().find(|op| op.kind == kind).unwrap().level;
    assert_eq!(level(wire::FAVOUR), Level::SignedIn);
    assert_eq!(level(wire::HOME), Level::SignedIn);
}

#[test]
fn a_record_that_names_nobody_tells_nobody() {
    let (mut stage, point) = stage();
    let who = stage.here[0].clone();
    let record = wire::Record {
        id: 1,
        part: Some(cube(point, 4.0)),
        version: 0,
        placed_by: None,
        set_by: None,
        ..Default::default()
    };
    stage.keep(&record_key(1), record.encode_to_vec());
    let mut parts = Parts::default();
    let set = wire::Set {
        id: 1,
        part: Some(cube(point, 5.0)),
    };
    parts.op(wire::SET, &set.encode_to_vec(), &who, &mut stage);
    let changed = told::<wire::Changed>(&mut stage, wire::CHANGED);
    let record = changed[0].0.record.as_ref().unwrap();
    assert_eq!(record.placed_by, None, "nobody known placed it");
    assert_eq!(record.set_by.as_ref().map(|by| by.visitor), Some(true));
}

/// A record a world kept by the name alone, before it kept the account,
/// still reads: its part stands, and its name is told with no word on its
/// account, never as a visitor's.
#[test]
fn a_record_kept_by_the_name_alone_reads_and_says_no_account() {
    let (mut stage, point) = stage();
    let who = stage.here[0].clone();
    let record = wire::Record {
        id: 1,
        part: Some(cube(point, 4.0)),
        version: 0,
        placed_name: "Ana".into(),
        ..Default::default()
    };
    stage.keep(&record_key(1), record.encode_to_vec());
    let mut parts = Parts::default();
    let set = wire::Set {
        id: 1,
        part: Some(cube(point, 5.0)),
    };
    parts.op(wire::SET, &set.encode_to_vec(), &who, &mut stage);
    let changed = told::<wire::Changed>(&mut stage, wire::CHANGED);
    let record = changed[0].0.record.as_ref().expect("the part stands");
    let ana = wire::By {
        name: "Ana".into(),
        user: String::new(),
        visitor: false,
    };
    assert_eq!(record.placed_by.as_ref(), Some(&ana));
    let kept = wire::Record::decode(stage.store.get(&record_key(1)).unwrap().as_slice()).unwrap();
    assert_eq!(kept.placed_by.as_ref(), Some(&ana), "kept as it was known");
    assert_eq!(kept.placed_name, "", "and no longer by the name alone");
}
