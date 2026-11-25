//! The life of a world in the server: a system of the core a world's admin
//! switches, on until the admin turns it off. A free animal is the same in every client from
//! the world's seed and clock, so the server keeps nothing of it; its line
//! in the world's statement says the version of life the world runs.
//!
//! What it keeps is the pets: one animal an account, by its number, with
//! its name and where it stands, in one row under the account's id. A
//! visitor's pet is its front end's to keep: handed over with `keep`, held
//! here for the session's life and let go when it leaves. Every change is
//! told to every session, and `pets` answers all of them, so a client draws
//! each pet where the world says, whoever saw it last.

use std::collections::BTreeMap;

use prost::Message;
use protocol::Stance;
use protocol::life as wire;

use gaia::fauna::{Blot, LEGS, NAME_BYTES, Region, Traits, named};
use glam::Vec3;

use crate::{Level, Op, Plugin, Room, Who};

/// How near the asker a place it names for its pet is, metres: where it
/// tames it, leaves it, or calls it to.
pub const NEAR_M: f64 = 8.0;
/// How long before the world hears a call the asker may have seen its pet
/// where it says, milliseconds: a call's way over the link.
const SEEN_MS: u64 = 1000;

/// The key an account's pet is kept under: a tag and the account's id.
fn pet_key(user: &str) -> Vec<u8> {
    let mut key = vec![b'p'];
    key.extend(user.as_bytes());
    key
}

/// Who keeps a pet.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Keeper {
    /// An account, by its id in mundos.
    Account(String),
    /// A visitor, by its session.
    Visitor(u32),
}

impl Keeper {
    fn of(who: &Who) -> Keeper {
        match who.user.is_empty() {
            true => Keeper::Visitor(who.session),
            false => Keeper::Account(who.user.clone()),
        }
    }

    /// Whether a session is this keeper's.
    fn is(&self, who: &Who) -> bool {
        *self == Keeper::of(who)
    }
}

/// The life of a world, as the server's module holds it.
#[derive(Default)]
pub struct Life {
    /// Every account's pet, by the account's id: read from the store the
    /// first time one is asked of.
    pets: Option<BTreeMap<String, wire::Pet>>,
    /// Every visitor's pet, by its session.
    visitors: BTreeMap<u32, wire::Pet>,
}

/// The system as the server's module holds it.
pub fn system() -> Box<dyn Plugin> {
    Box::new(Life::default())
}

impl Life {
    /// Every account's pet, read once. A row kept before the world kept a
    /// pet's look takes it as [`filled`] says, kept back.
    fn accounts(&mut self, room: &mut dyn Room) -> &mut BTreeMap<String, wire::Pet> {
        if self.pets.is_none() {
            let mut pets: BTreeMap<String, wire::Pet> = room
                .scan(b"p")
                .into_iter()
                .filter_map(|(key, value)| {
                    let user = String::from_utf8(key.get(1..)?.to_vec()).ok()?;
                    Some((user, wire::Pet::decode(value.as_slice()).ok()?))
                })
                .collect();
            for (user, pet) in &mut pets {
                if filled(pet, room) {
                    room.keep(&pet_key(user), pet.encode_to_vec());
                }
            }
            self.pets = Some(pets);
        }
        self.pets.get_or_insert_default()
    }

    /// The pet a keeper has, as kept.
    fn pet_of(&mut self, keeper: &Keeper, room: &mut dyn Room) -> Option<wire::Pet> {
        match keeper {
            Keeper::Account(user) => self.accounts(room).get(user).cloned(),
            Keeper::Visitor(session) => self.visitors.get(session).cloned(),
        }
    }

    /// Who keeps the animal of a number, when someone does.
    fn keeper_of(&mut self, number: u64, room: &mut dyn Room) -> Option<Keeper> {
        let account = self
            .accounts(room)
            .iter()
            .find(|(_, pet)| pet.number == number);
        if let Some((user, _)) = account {
            return Some(Keeper::Account(user.clone()));
        }
        let visitor = self.visitors.iter().find(|(_, pet)| pet.number == number);
        visitor.map(|(session, _)| Keeper::Visitor(*session))
    }

    /// Every pet with who keeps it.
    fn every(&mut self, room: &mut dyn Room) -> Vec<(Keeper, wire::Pet)> {
        let accounts = self.accounts(room).clone().into_iter();
        let accounts = accounts.map(|(user, pet)| (Keeper::Account(user), pet));
        let visitors = self.visitors.clone().into_iter();
        accounts
            .chain(visitors.map(|(session, pet)| (Keeper::Visitor(session), pet)))
            .collect()
    }

    /// Keeps a keeper's pet, and tells every session.
    fn keep(&mut self, keeper: Keeper, pet: wire::Pet, room: &mut dyn Room) {
        match &keeper {
            Keeper::Account(user) => {
                room.keep(&pet_key(user), pet.encode_to_vec());
                self.accounts(room).insert(user.clone(), pet.clone());
            }
            Keeper::Visitor(session) => {
                self.visitors.insert(*session, pet.clone());
            }
        }
        tell_pet(&keeper, pet, room);
    }

    /// A keeper's pet goes back to its herd, told to every session.
    fn free(&mut self, keeper: &Keeper, room: &mut dyn Room) {
        let freed = match keeper {
            Keeper::Account(user) => {
                room.forget(&pet_key(user));
                self.accounts(room).remove(user)
            }
            Keeper::Visitor(session) => self.visitors.remove(session),
        };
        if let Some(pet) = freed {
            let freed = wire::Freed { number: pet.number };
            room.tell(wire::FREED, freed.encode_to_vec(), &everyone(room));
        }
    }

    /// The animal the asker rides becomes its pet, in the place of the one
    /// it had.
    fn tame(&mut self, payload: &[u8], who: &Who, room: &mut dyn Room) {
        let Ok(wire::Tame {
            number,
            spot: Some(spot),
        }) = wire::Tame::decode(payload)
        else {
            return room.refuse("message");
        };
        if !placed(&spot, room) {
            return room.refuse("place");
        }
        if number == 0 || who.stance.as_ref().is_none_or(|s| s.carrier != number) {
            return room.refuse("riding");
        }
        if ridden_by_another(number, who, room) {
            return room.refuse("ridden");
        }
        let keeper = Keeper::of(who);
        let kept_by = self.keeper_of(number, room);
        if kept_by.as_ref().is_some_and(|k| *k != keeper) {
            return room.refuse("anothers");
        }
        if !near(who, &spot, room) {
            return room.refuse("far");
        }
        // What its herd gives it is read from the world's ground: without
        // it, no record is kept half.
        if room.ground().is_none() {
            return room.refuse("ground");
        }
        let had = self.pet_of(&keeper, room);
        if had.as_ref().is_some_and(|pet| pet.number != number) {
            self.free(&keeper, room);
        }
        // Tamed, it keeps what its herd gives it now; tamed again, its name
        // and all it kept.
        let again = had.filter(|pet| pet.number == number);
        let wings = match again.as_ref().and_then(|pet| pet.wings.clone()) {
            Some(kept) => Some(kept),
            None => wings_given(number, room),
        };
        let traits = match again.as_ref().and_then(|pet| pet.traits.clone()) {
            Some(kept) => Some(kept),
            None => wings.as_deref().and_then(|wings| born(number, wings, room)),
        };
        let pet = wire::Pet {
            number,
            name: again.map(|pet| pet.name).unwrap_or_default(),
            spot: Some(now(spot, room)),
            from: None,
            from_mps: 0.0,
            owner: who.name.clone(),
            mine: false,
            session: 0,
            wings,
            traits,
        };
        self.keep(keeper, pet, room);
    }

    /// Names the asker's pet.
    fn name(&mut self, payload: &[u8], who: &Who, room: &mut dyn Room) {
        let Ok(asked) = wire::Name::decode(payload) else {
            return room.refuse("message");
        };
        let Some(name) = named(&asked.name) else {
            return room.refuse("name");
        };
        let keeper = Keeper::of(who);
        let Some(pet) = self.pet_of(&keeper, room) else {
            return room.refuse("none");
        };
        self.keep(keeper, wire::Pet { name, ..pet }, room);
    }

    /// The asker's pet stands where it stepped down from it.
    fn leave(&mut self, payload: &[u8], who: &Who, room: &mut dyn Room) {
        let Ok(wire::Leave { spot: Some(spot) }) = wire::Leave::decode(payload) else {
            return room.refuse("message");
        };
        if !placed(&spot, room) {
            return room.refuse("place");
        }
        let keeper = Keeper::of(who);
        let Some(pet) = self.pet_of(&keeper, room) else {
            return room.refuse("none");
        };
        if !near(who, &spot, room) {
            return room.refuse("far");
        }
        let left = wire::Pet {
            spot: Some(now(spot, room)),
            from: None,
            from_mps: 0.0,
            ..pet
        };
        self.keep(keeper, left, room);
    }

    /// The asker's pet runs to beside the body, setting off at the speed
    /// the asker saw, at most [`gaia::fauna::FASTEST_MPS`]. Within earshot
    /// it sets off from where it was seen, at the instant the asker saw it
    /// there, within [`SEEN_MS`] before now. From past it, within
    /// [`gaia::fauna::ANSWERS_M`], it leaves where it was now, is out of
    /// sight a while, by how far that was, and comes into sight where the
    /// asker says, near the body: the record says so by a `from` earlier
    /// than its `spot`. Farther, it does not answer: `too_far`.
    fn call(&mut self, payload: &[u8], who: &Who, room: &mut dyn Room) {
        let Ok(wire::Call {
            from,
            to: Some(to),
            enter,
            from_mps,
        }) = wire::Call::decode(payload)
        else {
            return room.refuse("message");
        };
        if !(0.0..=gaia::fauna::FASTEST_MPS).contains(&from_mps) {
            return room.refuse("message");
        }
        let (seen, afar) = match (from, enter) {
            (Some(seen), None) => (seen, false),
            (None, Some(enter)) => (enter, true),
            _ => return room.refuse("message"),
        };
        if !placed(&seen, room) || !placed(&to, room) {
            return room.refuse("place");
        }
        let keeper = Keeper::of(who);
        let Some(pet) = self.pet_of(&keeper, room) else {
            return room.refuse("none");
        };
        if !near(who, &to, room) {
            return room.refuse("far");
        }
        let now_ms = room.now().0;
        let called = match afar {
            false => {
                let heard = within(who, &seen, gaia::fauna::HEARS_M, room)
                    && on_its_way(&pet, &seen, who, room);
                if !heard {
                    return room.refuse("unheard");
                }
                // Set off where the asker saw it, when it saw it there: a
                // peer draws the run on from there, never a step back.
                let now_ms = room.now().0;
                let at_ms = seen.at_ms.clamp(now_ms.saturating_sub(SEEN_MS), now_ms);
                wire::Pet {
                    spot: Some(wire::Spot { at_ms, ..to }),
                    from: Some(wire::Spot { at_ms, ..seen }),
                    from_mps,
                    ..pet
                }
            }
            true => {
                if !within(who, &seen, gaia::fauna::COMES_M + NEAR_M, room) {
                    return room.refuse("far");
                }
                let was_m = pet
                    .spot
                    .as_ref()
                    .and_then(|spot| apart_m(who, spot, &to, room));
                // Measured from beside the body, where the client measured
                // from the body: a step of slack.
                let Some(was_m) = was_m.filter(|&m| m <= gaia::fauna::ANSWERS_M + NEAR_M) else {
                    return room.refuse("too_far");
                };
                let coming_s = gaia::fauna::coming_s(was_m);
                let comes_ms = now_ms + (coming_s * 1000.0).round() as u64;
                wire::Pet {
                    spot: Some(wire::Spot {
                        at_ms: comes_ms,
                        ..to
                    }),
                    from: Some(wire::Spot {
                        at_ms: now_ms,
                        ..seen
                    }),
                    from_mps,
                    ..pet
                }
            }
        };
        self.keep(keeper, called, room);
    }

    /// Every pet, to the asker alone.
    fn pets(&mut self, who: &Who, room: &mut dyn Room) {
        let pets = self
            .every(room)
            .into_iter()
            .map(|(keeper, pet)| told(&keeper, pet, keeper.is(who)));
        let pets = wire::Pets {
            pets: pets.collect(),
        };
        room.tell(wire::PETS, pets.encode_to_vec(), &[who.session]);
    }

    /// A visitor's pet, as its front end kept it.
    fn hold(&mut self, payload: &[u8], who: &Who, room: &mut dyn Room) {
        let Ok(wire::Keep { pet: Some(pet) }) = wire::Keep::decode(payload) else {
            return room.refuse("message");
        };
        if !who.user.is_empty() {
            return room.refuse("account");
        }
        let name = match pet.name.is_empty() {
            true => Some(String::new()),
            false => named(&pet.name),
        };
        let (Some(name), Some(spot)) = (name, pet.spot) else {
            return room.refuse("message");
        };
        if !placed(&spot, room) || pet.from.is_some_and(|from| !placed(&from, room)) {
            return room.refuse("place");
        }
        let keeper = Keeper::of(who);
        let kept_by = self.keeper_of(pet.number, room);
        if pet.number == 0 || kept_by.is_some_and(|k| k != keeper) {
            return room.refuse("anothers");
        }
        if ridden_by_another(pet.number, who, room) {
            return room.refuse("ridden");
        }
        // The look its front end kept, as it is, never against today's
        // table: wings none or a short coat's name on a kind born with
        // some, traits an animal of its kind can be drawn in. Kept from
        // before the world kept them, as an account's are filled.
        let kind = Region::of(pet.number).and_then(|(region, _)| region.kind());
        if pet
            .wings
            .as_deref()
            .is_some_and(|coat| !wings_fit(coat, kind.and_then(|kind| kind.wings).is_some()))
        {
            return room.refuse("wings");
        }
        // Filled first: a voice a front end from before does not hand is
        // the herd's, or its kind's own where no ground says the herd's;
        // never a refusal. Held for the session, it is never written back.
        let mut pet = pet;
        filled(&mut pet, room);
        if let Some(traits) = pet.traits.as_mut() {
            traits.voice_cents.get_or_insert(0.0);
        }
        let fits =
            |traits: &wire::Traits| kind.is_some_and(|kind| traits_of(traits, false).fits(kind));
        if pet.traits.as_ref().is_some_and(|traits| !fits(traits)) {
            return room.refuse("traits");
        }
        let had = self.pet_of(&keeper, room);
        if had.is_some_and(|had| had.number != pet.number) {
            self.free(&keeper, room);
        }
        let pet = wire::Pet {
            number: pet.number,
            name,
            spot: Some(spot),
            from: pet.from,
            owner: who.name.clone(),
            mine: false,
            session: 0,
            wings: pet.wings,
            traits: pet.traits,
            from_mps: Some(pet.from_mps)
                .filter(|mps| mps.is_finite())
                .map_or(0.0, |mps| mps.clamp(0.0, gaia::fauna::FASTEST_MPS)),
        };
        self.keep(keeper, pet, room);
    }
}

/// Fills what a record kept before the world kept a pet's look lacks, by
/// the rules of now, once: its wings, then the rest of its traits, then a
/// voice its traits were kept without. Whether it changed.
fn filled(pet: &mut wire::Pet, room: &dyn Room) -> bool {
    let voiced = |pet: &wire::Pet| {
        pet.traits
            .as_ref()
            .map(|traits| traits.voice_cents.is_some())
    };
    let had = (pet.wings.is_some(), pet.traits.is_some(), voiced(pet));
    if pet.wings.is_none() {
        pet.wings = wings_given(pet.number, room);
    }
    let wings = pet.wings.clone();
    let born_now = || {
        wings
            .as_deref()
            .and_then(|wings| born(pet.number, wings, room))
    };
    match &mut pet.traits {
        None => pet.traits = born_now(),
        Some(traits) if traits.voice_cents.is_none() => {
            traits.voice_cents = born_now().and_then(|born| born.voice_cents);
        }
        Some(_) => {}
    }
    (pet.wings.is_some(), pet.traits.is_some(), voiced(pet)) != had
}

/// The traits the herd gives the animal of `number` now, with wings in the
/// coat `wings` names, empty for none. None without the world's ground to
/// say, or for a number of no herd.
fn born(number: u64, wings: &str, room: &dyn Room) -> Option<wire::Traits> {
    let (region, place) = Region::of(number)?;
    let kind = region.kind()?;
    let (herd, _) = region.herd(room.ground()?.recipe().seed)?;
    let wings = Some(wings).filter(|wings| !wings.is_empty());
    Some(wire_of(&Traits::drawn(kind, herd.seed_of(place), wings)))
}

/// Traits as the wire says them; `wings`, whether the pet wears its own.
pub(crate) fn traits_of(traits: &wire::Traits, wings: bool) -> Traits {
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
pub(crate) fn wire_of(traits: &Traits) -> wire::Traits {
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

/// The wings the herd gives the animal of `number`, as a pet keeps them:
/// a coat, or empty for none. None without the world's ground to say.
fn wings_given(number: u64, room: &dyn Room) -> Option<String> {
    let seed = room.ground()?.recipe().seed;
    Some(Region::wings(number, seed).unwrap_or_default().to_owned())
}

/// Whether `coat` is wings a pet may keep: none, or a coat's name on a
/// kind born with wings. Never today's winged coats, which a pet outlives.
fn wings_fit(coat: &str, winged: bool) -> bool {
    coat.is_empty() || (winged && coat.len() <= NAME_BYTES)
}

/// Every session of the room.
fn everyone(room: &dyn Room) -> Vec<u32> {
    room.sessions().iter().map(|who| who.session).collect()
}

/// A pet as a session is told it: whether it is the session's own, and
/// for a visitor's, the session that keeps it.
fn told(keeper: &Keeper, pet: wire::Pet, mine: bool) -> wire::Pet {
    wire::Pet {
        mine,
        session: match keeper {
            Keeper::Visitor(session) => *session,
            Keeper::Account(_) => 0,
        },
        ..pet
    }
}

/// Tells every session of a keeper's pet, each whether it is its own.
fn tell_pet(keeper: &Keeper, pet: wire::Pet, room: &mut dyn Room) {
    let (mine, others): (Vec<&Who>, Vec<&Who>) =
        room.sessions().iter().partition(|who| keeper.is(who));
    let sessions = |whos: Vec<&Who>| whos.iter().map(|who| who.session).collect::<Vec<u32>>();
    for (to, mine) in [(sessions(mine), true), (sessions(others), false)] {
        if !to.is_empty() {
            let told = wire::Told {
                pet: Some(told(keeper, pet.clone(), mine)),
            };
            room.tell(wire::PET, told.encode_to_vec(), &to);
        }
    }
}

/// A spot said now, on the server's clock.
fn now(spot: wire::Spot, room: &dyn Room) -> wire::Spot {
    wire::Spot {
        at_ms: room.now().0,
        ..spot
    }
}

/// Whether a spot is a place on the planet: a sector, a place in it within
/// its side, and a heading, every number finite. What the world keeps of a
/// pet it tells every session, whose drawing and generator read it: a
/// spot from one client that is none is refused before it reaches them.
/// A spot says no height: a pet stands on the ground.
fn placed(spot: &wire::Spot, room: &dyn Room) -> bool {
    let side = f64::from(room.measure().sphere.blocks().side());
    let within = |at: f64| at.is_finite() && (0.0..=side).contains(&at);
    topology::Sector::new(u8::try_from(spot.sector).unwrap_or(u8::MAX)).is_some()
        && within(spot.u)
        && within(spot.v)
        && spot.heading.is_finite()
}

/// Whether a body other than the asker's rides the animal of `number`.
fn ridden_by_another(number: u64, who: &Who, room: &dyn Room) -> bool {
    let others = room.sessions().iter().filter(|s| s.session != who.session);
    others
        .filter_map(|s| s.stance.as_ref())
        .any(|stance| stance.carrier == number)
}

/// Whether a pet can be at `seen`: on its way from where it set off to
/// where it was kept, or standing there, within [`NEAR_M`] of that line
/// at either end. A client says where it sees its pet; the world holds it
/// to where the pet can be.
fn on_its_way(pet: &wire::Pet, seen: &wire::Spot, who: &Who, room: &dyn Room) -> bool {
    let Some(kept) = &pet.spot else {
        return false;
    };
    let way_m = pet
        .from
        .as_ref()
        .and_then(|from| apart_m(who, from, kept, room))
        .unwrap_or(0.0);
    apart_m(who, seen, kept, room).is_some_and(|seen_m| seen_m <= way_m + NEAR_M)
}

/// Where the asker stands, when it stands on the planet, where pets are:
/// from the moon no spot is measured, and nothing a pet does is asked.
fn on_planet(who: &Who) -> Option<&Stance> {
    let stance = who.stance.as_ref()?;
    (stance.body() == protocol::Body::Planet).then_some(stance)
}

/// How far apart two spots are along the planet, metres, at the asker's
/// height; none for a spot on no sector, or an asker off the planet.
fn apart_m(who: &Who, a: &wire::Spot, b: &wire::Spot, room: &dyn Room) -> Option<f64> {
    let stance = on_planet(who)?;
    let at = |spot: &wire::Spot| Stance {
        sector: spot.sector,
        u: spot.u as f32,
        v: spot.v as f32,
        ..stance.clone()
    };
    let apart_b = room.measure().apart(&at(a), &at(b))?;
    Some(apart_b * topology::BLOCK_M)
}

/// Whether a spot is within [`NEAR_M`] of where the asker stands.
fn near(who: &Who, spot: &wire::Spot, room: &dyn Room) -> bool {
    within(who, spot, NEAR_M, room)
}

/// Whether a spot is within `reach_m` of where the asker stands, along the
/// planet: its height is the asker's own, since a spot says none. Never
/// for an asker off the planet.
fn within(who: &Who, spot: &wire::Spot, reach_m: f64, room: &dyn Room) -> bool {
    let Some(stance) = on_planet(who) else {
        return false;
    };
    let at = Stance {
        sector: spot.sector,
        u: spot.u as f32,
        v: spot.v as f32,
        ..stance.clone()
    };
    let apart_b = room.measure().apart(stance, &at);
    apart_b.is_some_and(|blocks| blocks * topology::BLOCK_M <= reach_m)
}

impl Plugin for Life {
    fn name(&self) -> &'static str {
        gaia::NAME
    }

    fn version(&self) -> u32 {
        gaia::VERSION
    }

    /// A visitor keeps a pet for its session, so every op is anyone's.
    fn ops(&self) -> Vec<Op> {
        let kinds = [
            wire::TAME,
            wire::NAME,
            wire::LEAVE,
            wire::CALL,
            wire::RELEASE,
            wire::PETS,
            wire::KEEP,
        ];
        kinds
            .into_iter()
            .map(|kind| Op {
                kind,
                level: Level::Anonymous,
            })
            .collect()
    }

    fn op(&mut self, kind: &str, payload: &[u8], who: &Who, room: &mut dyn Room) {
        match kind {
            wire::TAME => self.tame(payload, who, room),
            wire::NAME => self.name(payload, who, room),
            wire::LEAVE => self.leave(payload, who, room),
            wire::CALL => self.call(payload, who, room),
            wire::RELEASE => {
                let keeper = Keeper::of(who);
                match self.pet_of(&keeper, room) {
                    Some(_) => self.free(&keeper, room),
                    None => room.refuse("none"),
                }
            }
            wire::PETS => self.pets(who, room),
            wire::KEEP => self.hold(payload, who, room),
            _ => {}
        }
    }

    /// A visitor's pet goes with its session: every client lets it go when
    /// the session leaves, by the session its pet was told with.
    fn gone(&mut self, session: u32) {
        self.visitors.remove(&session);
    }
}

#[cfg(test)]
mod tests;
