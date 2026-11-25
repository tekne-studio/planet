//! The pets of a world as this client knows them: one animal an account, and
//! a visitor's for its session, kept by the world's life and told to every
//! session. This client asks for all of them on every welcome, takes each
//! change as it is told, and asks the world for its own body's: tame, name,
//! leave, call, release.
//!
//! A visitor's pet is its front end's to keep, as its home and favourites
//! are: the page hands it back with `set_pet`, and it is handed to the
//! world on every welcome, held there while the session lasts.

use std::collections::BTreeMap;

use gaia::fauna::{Blot, LEGS, Traits};
use glam::Vec3;
use protocol::Message;
use protocol::life as wire;
use serde::{Deserialize, Serialize};

use crate::cells::Ask;

/// Where the numbers this client asks life's ops under start: an answer
/// names no owner, so each owner's numbers keep apart from the cells' and
/// the parts', which count up from one.
const ASKS_FROM: u32 = 1 << 31;

/// Where a pet stands and which way it faces, since when: a sector, a
/// place in it in blocks, the heading in radians from the sector's `+u`
/// toward its `+v`, and the world's clock in milliseconds.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Spot {
    pub sector: u32,
    pub u: f64,
    pub v: f64,
    pub heading: f64,
    pub at_ms: u64,
}

impl Spot {
    fn of(spot: &wire::Spot) -> Spot {
        Spot {
            sector: spot.sector,
            u: spot.u,
            v: spot.v,
            heading: spot.heading,
            at_ms: spot.at_ms,
        }
    }

    pub(crate) fn wire(self) -> wire::Spot {
        wire::Spot {
            sector: self.sector,
            u: self.u,
            v: self.v,
            heading: self.heading,
            at_ms: self.at_ms,
        }
    }
}

/// A pet as the world told it: its animal's number, its name (empty until
/// named), where it stands, and, called, where it set off from; who keeps
/// it, whether it is this client's own, and for a visitor's, the session
/// that keeps it.
#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct Pet {
    pub number: u64,
    pub name: String,
    pub spot: Spot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Spot>,
    /// How fast it was going as it set off from `from`, metres a second.
    #[serde(default, skip_serializing_if = "is_still")]
    pub from_mps: f64,
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub mine: bool,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub session: u32,
    /// Everything it was born with, as the world kept it when it was
    /// tamed: what it is drawn in, its wings among them, worn in its coat.
    /// None in a record from before the world kept them. Boxed, a pet
    /// stays small to carry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub traits: Option<Box<Traits>>,
}

fn is_zero(session: &u32) -> bool {
    *session == 0
}

fn is_still(mps: &f64) -> bool {
    *mps == 0.0
}

impl Pet {
    fn of(pet: wire::Pet) -> Option<Pet> {
        Some(Pet {
            number: pet.number,
            spot: Spot::of(pet.spot.as_ref()?),
            from: pet.from.as_ref().map(Spot::of),
            from_mps: pet.from_mps,
            name: pet.name,
            owner: pet.owner,
            mine: pet.mine,
            session: pet.session,
            traits: pet.traits.as_ref().map(|traits| {
                let wings = pet.wings.as_deref().is_some_and(|coat| !coat.is_empty());
                Box::new(traits_of(traits, wings))
            }),
        })
    }

    /// The coat it wears its wings in, empty for none, as the wire keeps
    /// them; none in a record from before.
    fn wings(&self) -> Option<String> {
        let traits = self.traits.as_deref()?;
        Some(match traits.wings {
            true => traits.coat.clone(),
            false => String::new(),
        })
    }
}

/// Traits as the wire says them; `wings`, whether the pet wears its own.
/// The world's own, word for word (`world::life::traits_of`), with the
/// same tests: what either refuses, [`Traits::fits`] refuses.
fn traits_of(traits: &wire::Traits, wings: bool) -> Traits {
    let colour = |c: &Option<wire::Colour>| c.map_or(Vec3::NAN, |c| Vec3::new(c.r, c.g, c.b));
    // One a leg, each 0 to 3; anything else is no sock an animal wears.
    let socks = <[u32; LEGS]>::try_from(traits.socks.as_slice()).map_or([u8::MAX; LEGS], |socks| {
        socks.map(|sock| u8::try_from(sock).unwrap_or(u8::MAX))
    });
    let blot = |blot: &wire::Blot| Blot {
        y: blot.y,
        z: blot.z,
        radius_m: blot.radius_m,
    };
    Traits {
        coat: traits.coat.clone(),
        hair: colour(&traits.hair),
        mane: colour(&traits.mane),
        points: colour(&traits.points),
        muzzle: colour(&traits.muzzle),
        hoof: colour(&traits.hoof),
        face: traits.face.clone(),
        socks,
        blots: traits.blots.iter().map(blot).collect(),
        size: traits.size,
        build: traits.build,
        mane_m: traits.mane_m,
        mane_falls: traits.mane_falls,
        tail_m: traits.tail_m,
        feathers: traits.feathers,
        wings,
        voice_cents: traits.voice_cents,
    }
}

/// Traits as the wire says them.
fn wire_of(traits: &Traits) -> wire::Traits {
    let colour = |c: Vec3| {
        Some(wire::Colour {
            r: c.x,
            g: c.y,
            b: c.z,
        })
    };
    let blot = |blot: &Blot| wire::Blot {
        y: blot.y,
        z: blot.z,
        radius_m: blot.radius_m,
    };
    wire::Traits {
        coat: traits.coat.clone(),
        hair: colour(traits.hair),
        mane: colour(traits.mane),
        points: colour(traits.points),
        muzzle: colour(traits.muzzle),
        hoof: colour(traits.hoof),
        face: traits.face.clone(),
        socks: traits.socks.map(u32::from).to_vec(),
        blots: traits.blots.iter().map(blot).collect(),
        size: traits.size,
        build: traits.build,
        mane_m: traits.mane_m,
        mane_falls: traits.mane_falls,
        tail_m: traits.tail_m,
        feathers: traits.feathers,
        voice_cents: traits.voice_cents,
    }
}

/// The world's pets as this client was told them, and its asks of life.
#[derive(Default)]
pub(crate) struct Pets {
    /// The session, while a world has welcomed this client, and whether it
    /// is an account's.
    link: Option<(u32, bool)>,
    asked: u32,
    asks: Vec<Ask>,
    /// What each ask waiting on its answer asked, by its number.
    waiting: BTreeMap<u32, &'static str>,
    /// Every pet told, by its animal's number.
    pets: BTreeMap<u64, Pet>,
    /// A visitor's pet as its front end kept it.
    kept: Option<Pet>,
    /// Whether the world's life is on, which keeps the pets.
    on: bool,
    /// Whether the world answered `pets` since this session was welcomed:
    /// until then, what this client holds of them is nothing yet.
    told: bool,
    changed: bool,
    /// This client's own pet as last said to its front end.
    said: Option<Pet>,
}

impl Pets {
    /// A world welcomed this session; its pets are asked for once it says
    /// its life is on ([`Pets::life`]).
    pub(crate) fn link(&mut self, me: u32, account: bool) {
        self.unlink();
        self.link = Some((me, account));
    }

    /// Whether the world's life is on, as its statement says: turned on,
    /// every pet is asked for, and a visitor's own handed over as its front
    /// end kept it; off, no pet is.
    pub(crate) fn life(&mut self, on: bool) {
        let was = std::mem::replace(&mut self.on, on);
        if on && !was {
            self.ask(wire::PETS, &wire::Ask {});
            self.hand_kept();
        } else if !on {
            self.changed |= !self.pets.is_empty();
            self.pets.clear();
        }
    }

    /// The world is gone, and its pets with it.
    pub(crate) fn unlink(&mut self) {
        self.link = None;
        self.on = false;
        self.told = false;
        self.asks.clear();
        self.waiting.clear();
        self.changed |= !self.pets.is_empty();
        self.pets.clear();
    }

    /// A visitor's pet, as its front end kept it: handed to the world now
    /// when it is welcome, and on every welcome after.
    pub(crate) fn set_kept(&mut self, pet: Option<Pet>) {
        self.kept = pet;
        self.hand_kept();
    }

    fn hand_kept(&mut self) {
        let Some(pet) = self.kept.clone() else {
            return;
        };
        if self.visitor() {
            let pet = wire::Pet {
                wings: pet.wings(),
                number: pet.number,
                name: pet.name,
                spot: Some(pet.spot.wire()),
                from: pet.from.map(Spot::wire),
                from_mps: pet.from_mps,
                traits: pet.traits.as_deref().map(wire_of),
                ..wire::Pet::default()
            };
            self.ask(wire::KEEP, &wire::Keep { pet: Some(pet) });
        }
    }

    fn ask<M: Message>(&mut self, kind: &'static str, op: &M) -> bool {
        if self.link.is_none() || !self.on {
            return false;
        }
        self.asked = self.asked.wrapping_add(1) % ASKS_FROM;
        let id = ASKS_FROM + self.asked;
        self.waiting.insert(id, kind);
        self.asks.push(Ask {
            kind,
            payload: op.encode_to_vec(),
            id,
        });
        true
    }

    /// The ops asked since the last drain, in order.
    pub(crate) fn drain_asks(&mut self) -> Vec<Ask> {
        std::mem::take(&mut self.asks)
    }

    /// The world answered an ask: which op it was, when it was this
    /// client's of life, with the code it was refused by, empty when it
    /// landed. A call or a leave refused was drawn ahead of the answer:
    /// the pets are asked for again, to stand where the world keeps them.
    pub(crate) fn answered(&mut self, id: u32, code: &str) -> Option<&'static str> {
        let kind = self.waiting.remove(&id)?;
        if !code.is_empty() && (kind == wire::CALL || kind == wire::LEAVE) {
            self.ask(wire::PETS, &wire::Ask {});
        }
        Some(kind)
    }

    /// Whether this session is a visitor's, whose pet its front end keeps.
    fn visitor(&self) -> bool {
        self.link.is_some_and(|(_, account)| !account)
    }

    /// What life said.
    pub(crate) fn receive(&mut self, kind: &str, payload: &[u8]) {
        match kind {
            wire::PET => {
                let told = wire::Told::decode(payload).ok().and_then(|told| told.pet);
                if let Some(pet) = told.and_then(Pet::of) {
                    if pet.mine {
                        self.pets.retain(|_, kept| !kept.mine);
                        // A visitor's is handed back as it now stands on
                        // the next welcome.
                        if self.visitor() {
                            self.kept = Some(pet.clone());
                        }
                    } else if self
                        .kept
                        .as_ref()
                        .is_some_and(|kept| kept.number == pet.number)
                    {
                        // Another took the animal this front end kept.
                        self.kept = None;
                    }
                    self.pets.insert(pet.number, pet);
                }
            }
            wire::FREED => {
                if let Ok(freed) = wire::Freed::decode(payload) {
                    let was = self.pets.remove(&freed.number);
                    let kept = self.kept.as_ref();
                    if kept.is_some_and(|pet| pet.number == freed.number) {
                        if was.is_some_and(|pet| pet.mine) {
                            self.kept = None;
                        } else {
                            // Held for another session, this front end's
                            // own before a reconnect: it is free to keep.
                            self.hand_kept();
                        }
                    }
                }
            }
            wire::PETS => {
                if let Ok(pets) = wire::Pets::decode(payload) {
                    self.told = true;
                    self.pets = pets
                        .pets
                        .into_iter()
                        .filter_map(Pet::of)
                        .map(|pet| (pet.number, pet))
                        .collect();
                }
            }
            _ => return,
        }
        self.changed = true;
    }

    /// A session left: a visitor's pet goes with it.
    pub(crate) fn left(&mut self, session: u32) {
        let before = self.pets.len();
        self.pets.retain(|_, pet| pet.session != session);
        self.changed |= self.pets.len() != before;
    }

    /// Whether the pets changed since this was last asked.
    pub(crate) fn take_changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    /// This client's own pet, when it changed since it was last said: only
    /// once a world with life on has told its pets, so a dropped link, a
    /// new one not yet answered, or life switched off never reads as the
    /// pet gone. A visitor's stays the one its front end kept while the
    /// world holds it for another session, its own from before a reconnect,
    /// until that one lets it go.
    pub(crate) fn resaid(&mut self) -> Option<Option<Pet>> {
        if self.link.is_none() || !self.on || !self.told {
            return None;
        }
        let kept = self.kept.clone().filter(|_| self.visitor());
        let mine = self.mine().cloned().or(kept);
        (mine != self.said).then(|| {
            self.said.clone_from(&mine);
            mine
        })
    }

    /// Every pet told, by its animal's number.
    pub(crate) fn all(&self) -> impl Iterator<Item = &Pet> {
        self.pets.values()
    }

    /// This client's own pet.
    pub(crate) fn mine(&self) -> Option<&Pet> {
        self.pets.values().find(|pet| pet.mine)
    }

    /// The pet of an animal's number.
    pub(crate) fn of(&self, number: u64) -> Option<&Pet> {
        self.pets.get(&number)
    }

    /// The animal the body rides becomes its pet, standing at `spot`.
    pub(crate) fn tame(&mut self, number: u64, spot: Spot) -> bool {
        let spot = Some(spot.wire());
        self.ask(wire::TAME, &wire::Tame { number, spot })
    }

    /// Names this client's pet.
    pub(crate) fn name(&mut self, name: String) -> bool {
        self.ask(wire::NAME, &wire::Name { name })
    }

    /// The body stepped down from its pet, which stands at `spot`: drawn
    /// there at once, as the world will say.
    pub(crate) fn leave(&mut self, spot: Spot) -> bool {
        self.predict(spot, None, 0.0);
        let spot = Some(spot.wire());
        self.ask(wire::LEAVE, &wire::Leave { spot })
    }

    /// Calls this client's pet from where it is seen, going `from_mps`, to
    /// beside the body: it sets off at once, as the world will say.
    pub(crate) fn call(&mut self, from: Spot, to: Spot, from_mps: f64) -> bool {
        self.predict(to, Some(from), from_mps);
        let (from, to) = (Some(from.wire()), Some(to.wire()));
        self.ask(
            wire::CALL,
            &wire::Call {
                from,
                to,
                enter: None,
                from_mps,
            },
        )
    }

    /// Calls this client's pet from past the whistle's reach to beside the
    /// body: it is out of sight until `to.at_ms`, then comes into it at
    /// `enter` and runs the rest, as the world will say, its wait counted
    /// there.
    pub(crate) fn call_from_afar(&mut self, enter: Spot, to: Spot, from_mps: f64) -> bool {
        self.predict(to, Some(enter), from_mps);
        let (to, enter) = (Some(to.wire()), Some(enter.wire()));
        self.ask(
            wire::CALL,
            &wire::Call {
                from: None,
                to,
                enter,
                from_mps,
            },
        )
    }

    /// This client's own pet where it is about to be told it is.
    fn predict(&mut self, spot: Spot, from: Option<Spot>, from_mps: f64) {
        if self.link.is_none() {
            return;
        }
        if let Some(pet) = self.pets.values_mut().find(|pet| pet.mine) {
            pet.spot = spot;
            pet.from = from;
            pet.from_mps = from_mps;
            self.changed = true;
        }
    }

    /// This client's pet goes back to its herd.
    pub(crate) fn release(&mut self) -> bool {
        self.ask(wire::RELEASE, &wire::Release {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn told(number: u64, mine: bool, session: u32) -> Vec<u8> {
        let pet = wire::Pet {
            number,
            name: format!("pet {number}"),
            spot: Some(wire::Spot::default()),
            mine,
            session,
            ..wire::Pet::default()
        };
        wire::Told { pet: Some(pet) }.encode_to_vec()
    }

    fn answer(told: &[(u64, bool, u32)]) -> Vec<u8> {
        let pets = told
            .iter()
            .map(|&(number, mine, session)| wire::Pet {
                number,
                spot: Some(wire::Spot::default()),
                mine,
                session,
                ..wire::Pet::default()
            })
            .collect();
        wire::Pets { pets }.encode_to_vec()
    }

    /// A pet's traits come through the wire as told and through the page
    /// as kept: what its front end keeps of a visitor's draws the same,
    /// and is handed back with its wings.
    #[test]
    fn a_pets_traits_are_kept_whole() {
        let born = Traits::drawn(gaia::fauna::KINDS[0], 7, Some("white"));
        let told = wire::Pet {
            number: 7,
            spot: Some(wire::Spot::default()),
            wings: Some("white".into()),
            traits: Some(wire_of(&born)),
            ..wire::Pet::default()
        };
        let pet = Pet::of(told.clone()).unwrap();
        assert_eq!(pet.traits.as_deref(), Some(&born));
        assert_eq!(pet.traits.as_deref().map(wire_of), told.traits);
        assert_eq!(pet.wings().as_deref(), Some("white"));
        let page = serde_json::to_string(&pet).unwrap();
        assert_eq!(serde_json::from_str::<Pet>(&page).unwrap(), pet);
    }

    /// Traits told wrong are traits no animal is drawn in, as the world
    /// reads them: socks not one a leg or past 3, a colour missing, a voice
    /// missing or past its kind's.
    #[test]
    fn traits_told_wrong_do_not_fit() {
        let horse = gaia::fauna::KINDS[0];
        let born = wire_of(&Traits::drawn(horse, 7, None));
        assert!(traits_of(&born, false).fits(horse));
        for (what, wrong) in [
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
            (
                "a missing colour",
                wire::Traits {
                    hair: None,
                    ..born.clone()
                },
            ),
            (
                "no voice",
                wire::Traits {
                    voice_cents: None,
                    ..born.clone()
                },
            ),
            (
                "a voice an octave off",
                wire::Traits {
                    voice_cents: Some(1200.0),
                    ..born.clone()
                },
            ),
        ] {
            assert!(!traits_of(&wrong, false).fits(horse), "{what}");
        }
    }

    /// A new link is no pet gone: nothing is said of it until the world
    /// answers what it keeps.
    #[test]
    fn a_reconnect_says_the_pet_only_once_the_world_answers() {
        let mut pets = Pets::default();
        pets.link(1, true);
        pets.life(true);
        pets.receive(wire::PETS, &answer(&[(7, true, 0)]));
        assert_eq!(pets.resaid().flatten().map(|pet| pet.number), Some(7));
        pets.link(2, true);
        pets.life(true);
        assert_eq!(pets.resaid(), None, "asked, not yet answered");
        pets.receive(wire::PETS, &answer(&[(7, true, 0)]));
        assert_eq!(pets.resaid(), None, "the same pet, unchanged");
    }

    /// A visitor back before the world let its last session go: the world
    /// holds the pet for that one and refuses it; once it is freed, it is
    /// handed again, and the front end never hears it gone.
    #[test]
    fn a_visitor_back_soon_keeps_its_pet_once_its_last_session_goes() {
        let mut pets = Pets::default();
        let mut kept = Pet {
            number: 9,
            mine: true,
            ..Pet::default()
        };
        pets.set_kept(Some(kept.clone()));
        pets.link(2, false);
        pets.life(true);
        pets.drain_asks();
        pets.receive(wire::PETS, &answer(&[(9, false, 1)]));
        let said = pets.resaid();
        assert_eq!(said.flatten().map(|pet| pet.number), Some(9));
        pets.receive(wire::FREED, &wire::Freed { number: 9 }.encode_to_vec());
        let asks: Vec<&str> = pets.drain_asks().iter().map(|ask| ask.kind).collect();
        assert_eq!(asks, [wire::KEEP], "handed again once free");
        pets.receive(wire::PET, &told(9, true, 2));
        kept.name = "pet 9".into();
        kept.session = 2;
        assert_eq!(pets.resaid().flatten().map(|pet| pet.name), Some(kept.name));

        // Freed while its own, it is gone; another taking it, it is theirs.
        pets.receive(wire::FREED, &wire::Freed { number: 9 }.encode_to_vec());
        assert_eq!(pets.resaid(), Some(None));
        assert!(pets.drain_asks().is_empty());
        pets.set_kept(Some(Pet {
            number: 4,
            mine: true,
            ..Pet::default()
        }));
        pets.drain_asks();
        pets.receive(wire::PET, &told(4, false, 3));
        assert_eq!(pets.resaid(), None, "nothing kept, nothing said");
    }

    /// A call the world refused was drawn ahead: the pets are asked again.
    #[test]
    fn a_refused_call_asks_where_the_pets_are() {
        let mut pets = Pets::default();
        pets.link(1, true);
        pets.life(true);
        pets.drain_asks();
        assert!(pets.call(Spot::default(), Spot::default(), 0.0));
        let call = pets.drain_asks().remove(0);
        assert_eq!(pets.answered(call.id, "unheard"), Some(wire::CALL));
        let asks: Vec<&str> = pets.drain_asks().iter().map(|ask| ask.kind).collect();
        assert_eq!(asks, [wire::PETS]);
    }

    #[test]
    fn pets_are_asked_on_welcome_and_follow_what_is_told() {
        let mut pets = Pets::default();
        assert!(!pets.tame(7, Spot::default()), "no world, no ask");
        pets.set_kept(Some(Pet {
            number: 9,
            ..Pet::default()
        }));
        pets.link(1, false);
        assert!(pets.drain_asks().is_empty(), "life is not on yet");
        pets.life(true);
        let asks: Vec<&str> = pets.drain_asks().iter().map(|ask| ask.kind).collect();
        assert_eq!(asks, [wire::PETS, wire::KEEP], "a visitor hands its own");
        pets.link(1, true);
        pets.life(true);
        let asks: Vec<&str> = pets.drain_asks().iter().map(|ask| ask.kind).collect();
        assert_eq!(asks, [wire::PETS], "an account's is the world's");
        pets.receive(wire::PETS, &answer(&[]));

        pets.receive(wire::PET, &told(7, true, 0));
        pets.receive(wire::PET, &told(8, false, 4));
        assert_eq!(pets.mine().map(|pet| pet.number), Some(7));
        pets.receive(wire::PET, &told(5, true, 0));
        assert_eq!(pets.mine().map(|pet| pet.number), Some(5), "one an account");
        assert!(pets.of(7).is_none());
        pets.left(4);
        assert!(pets.of(8).is_none(), "a visitor's goes with its session");
        let freed = wire::Freed { number: 5 }.encode_to_vec();
        pets.receive(wire::FREED, &freed);
        assert!(pets.mine().is_none());
        assert!(pets.take_changed() && !pets.take_changed());
        assert_eq!(pets.resaid(), None, "it had none, and has none");
        pets.receive(wire::PET, &told(6, true, 0));
        assert_eq!(pets.resaid().flatten().map(|pet| pet.number), Some(6));
        assert_eq!(pets.resaid(), None);

        assert!(pets.release());
        let ask = pets.drain_asks().remove(0);
        assert!(ask.id >= ASKS_FROM, "apart from the cells' and the parts'");
        assert_eq!(pets.answered(ask.id, ""), Some(wire::RELEASE));
        assert_eq!(pets.answered(ask.id, ""), None);
        pets.unlink();
        assert_eq!(pets.resaid(), None, "a dropped link is no pet gone");
    }
}
