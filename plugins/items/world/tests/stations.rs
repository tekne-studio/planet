//! The items world half, handed a room of four: who holds a cabinet's
//! stations, as everyone hears it, and who is linked to relay about it.

use std::collections::BTreeMap;

use items_world::{
    Items, LOOK, REFUSED_PAYLOAD, REFUSED_STATION, REFUSED_TAKEN, RISE, SIT, STATIONS,
    STATIONS_MOST, wire,
};
use prost::Message;
use topology::QuadSphere;
use world::{Level, Measure, Moment, Plugin, Room, Who};

const CABINET: u64 = 0xcab;
const OTHER: u64 = 0xbee;

#[derive(Default)]
struct Hall {
    here: Vec<Who>,
    /// Who heard which cabinet's stations, in order.
    heard: Vec<(u32, wire::Stations)>,
    linked: BTreeMap<u64, Vec<u32>>,
    refused: Option<String>,
}

impl Room for Hall {
    fn now(&self) -> Moment {
        Moment(0)
    }

    fn measure(&self) -> Measure {
        Measure {
            sphere: QuadSphere::new(16).unwrap(),
            moon: QuadSphere::new(14).unwrap(),
        }
    }

    fn sessions(&self) -> &[Who] {
        &self.here
    }

    fn tell(&mut self, kind: &str, payload: Vec<u8>, to: &[u32]) {
        assert_eq!(kind, STATIONS, "items says one event");
        let said = wire::Stations::decode(payload.as_slice()).unwrap();
        self.heard.extend(to.iter().map(|&who| (who, said.clone())));
    }

    fn ground(&self) -> Option<&worldgen::Generator> {
        None
    }

    fn get(&mut self, _key: &[u8]) -> Option<Vec<u8>> {
        None
    }

    fn scan(&mut self, _prefix: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        Vec::new()
    }

    fn keep(&mut self, key: &[u8], _value: Vec<u8>) {
        panic!("who sits where is kept by nobody: {key:?}");
    }

    fn forget(&mut self, key: &[u8]) {
        panic!("who sits where is kept by nobody: {key:?}");
    }

    fn refuse(&mut self, code: &str) {
        self.refused = Some(code.to_owned());
    }

    fn link(&mut self, thing: u64, sessions: &[u32]) {
        if sessions.is_empty() {
            self.linked.remove(&thing);
        } else {
            self.linked.insert(thing, sessions.to_vec());
        }
    }
}

fn who(session: u32) -> Who {
    Who {
        session,
        user: String::new(),
        name: String::new(),
        level: Level::Anonymous,
        stance: None,
    }
}

/// Four in the world, nobody seated.
fn hall() -> (Items, Hall) {
    let hall = Hall {
        here: (1..=4).map(who).collect(),
        ..Hall::default()
    };
    (Items::default(), hall)
}

fn sit(items: &mut Items, hall: &mut Hall, session: u32, cabinet: u64, station: u32) {
    hall.refused = None;
    let payload = wire::Sit { cabinet, station }.encode_to_vec();
    items.op(SIT, &payload, &who(session), hall);
}

fn rise(items: &mut Items, hall: &mut Hall, session: u32, cabinet: u64) {
    hall.refused = None;
    let payload = wire::Rise { cabinet }.encode_to_vec();
    items.op(RISE, &payload, &who(session), hall);
}

/// What `session` last heard of `cabinet`'s stations.
fn last(hall: &Hall, session: u32, cabinet: u64) -> Option<Vec<u32>> {
    hall.heard
        .iter()
        .rev()
        .find(|(to, said)| *to == session && said.cabinet == cabinet)
        .map(|(_, said)| said.holders.clone())
}

#[test]
fn two_sit_a_third_is_refused_and_everyone_hears_it() {
    let (mut items, mut hall) = hall();
    sit(&mut items, &mut hall, 1, CABINET, 0);
    sit(&mut items, &mut hall, 2, CABINET, 1);
    assert_eq!(hall.refused, None);
    // Everyone hears both stations held, the one who did not sit too.
    for session in 1..=4 {
        assert_eq!(last(&hall, session, CABINET), Some(vec![1, 2]));
    }
    assert_eq!(hall.linked[&CABINET], [1, 2], "the two relay to each other");
    // A third finds every station taken, and one past them is none.
    sit(&mut items, &mut hall, 3, CABINET, 0);
    assert_eq!(hall.refused.as_deref(), Some(REFUSED_TAKEN));
    sit(&mut items, &mut hall, 3, CABINET, STATIONS_MOST as u32);
    assert_eq!(hall.refused.as_deref(), Some(REFUSED_STATION));
    sit(&mut items, &mut hall, 3, CABINET, u32::MAX);
    assert_eq!(hall.refused.as_deref(), Some(REFUSED_STATION));
    assert_eq!(last(&hall, 3, CABINET), Some(vec![1, 2]), "nothing changed");
    // Sitting again where one sits already changes nothing and says it again,
    // links included: the server lets a link go while its owner is off.
    hall.linked.clear();
    let said = hall.heard.len();
    sit(&mut items, &mut hall, 1, CABINET, 0);
    assert_eq!(hall.refused, None);
    assert_eq!(hall.heard.len(), said + 4, "everyone hears it again");
    assert_eq!(last(&hall, 4, CABINET), Some(vec![1, 2]));
    assert_eq!(hall.linked[&CABINET], [1, 2]);
}

#[test]
fn rising_and_leaving_let_a_station_go() {
    let (mut items, mut hall) = hall();
    sit(&mut items, &mut hall, 1, CABINET, 0);
    sit(&mut items, &mut hall, 2, CABINET, 1);
    // Rising from a cabinet one does not sit at is nothing.
    rise(&mut items, &mut hall, 3, CABINET);
    rise(&mut items, &mut hall, 1, OTHER);
    assert_eq!(last(&hall, 4, CABINET), Some(vec![1, 2]));
    rise(&mut items, &mut hall, 1, CABINET);
    assert_eq!(last(&hall, 4, CABINET), Some(vec![0, 2]));
    assert_eq!(hall.linked[&CABINET], [2]);
    // The free station is taken again; one who left lets its own go, unsaid:
    // everyone hears the session left.
    sit(&mut items, &mut hall, 3, CABINET, 0);
    assert_eq!(last(&hall, 4, CABINET), Some(vec![3, 2]));
    items.gone(2);
    sit(&mut items, &mut hall, 4, CABINET, 1);
    assert_eq!(hall.refused, None, "a station of one who left is free");
    assert_eq!(last(&hall, 1, CABINET), Some(vec![3, 4]));
    // The last to rise lets the cabinet go, and its link with it.
    rise(&mut items, &mut hall, 3, CABINET);
    rise(&mut items, &mut hall, 4, CABINET);
    assert_eq!(last(&hall, 1, CABINET), Some(vec![0, 0]));
    assert!(hall.linked.is_empty());
}

#[test]
fn a_session_holds_one_station_in_the_world() {
    let (mut items, mut hall) = hall();
    sit(&mut items, &mut hall, 1, CABINET, 0);
    sit(&mut items, &mut hall, 1, CABINET, 1);
    assert_eq!(last(&hall, 2, CABINET), Some(vec![0, 1]), "moved along");
    sit(&mut items, &mut hall, 1, OTHER, 0);
    assert_eq!(last(&hall, 2, CABINET), Some(vec![0, 0]));
    assert_eq!(last(&hall, 2, OTHER), Some(vec![1, 0]));
    assert_eq!(hall.linked.keys().collect::<Vec<_>>(), [&OTHER]);
}

#[test]
fn a_look_tells_the_asker_every_station_held() {
    let (mut items, mut hall) = hall();
    sit(&mut items, &mut hall, 1, CABINET, 1);
    sit(&mut items, &mut hall, 2, OTHER, 0);
    hall.heard.clear();
    items.op(LOOK, &[], &who(3), &mut hall);
    let heard: Vec<_> = hall
        .heard
        .iter()
        .map(|(to, said)| (*to, said.cabinet, said.holders.clone()))
        .collect();
    assert_eq!(
        heard,
        [(3, OTHER, vec![2, 0]), (3, CABINET, vec![0, 1])],
        "to the asker alone, by cabinet"
    );
}

#[test]
fn a_payload_that_is_no_op_is_refused() {
    let (mut items, mut hall) = hall();
    sit(&mut items, &mut hall, 1, CABINET, 0);
    items.op(SIT, &[0xff, 0xff, 0xff], &who(2), &mut hall);
    assert_eq!(hall.refused.as_deref(), Some(REFUSED_PAYLOAD));
    hall.refused = None;
    items.op(RISE, &[0x0a], &who(1), &mut hall);
    assert_eq!(hall.refused.as_deref(), Some(REFUSED_PAYLOAD));
    assert_eq!(last(&hall, 2, CABINET), Some(vec![1, 0]));
}
