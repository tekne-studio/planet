//! The other bodies in the world: who they are, where they were last seen,
//! and where to draw them now.
//!
//! A stance arrives a few times a second and a frame is drawn sixty times a
//! second, so a peer is drawn a little in the past, between the last two
//! stances heard, and holds at the newest one when the next is late. The
//! stance is an address; it becomes a world position here, once, when it
//! lands, so a walk across a sector seam interpolates in world space and
//! never through the seam.

use std::collections::HashMap;

use glam::DVec3;
use topology::{Sector, SurfacePoint};
use worldgen::{Generator, MOON_RADIUS_M};

use crate::figure::{self, Clips, Figure, Gait, Motion};
use crate::hand::{Thing, Things};
use crate::hover;
use crate::seam::PeerInfo;
use crate::way::{Profile, Way, Ways};

/// Gravity on the planet, metres per second squared: what a turn leans
/// against.
const GRAVITY_MPS2: f64 = crate::controller::GRAVITY_MPS2;

/// How far behind the newest stance a peer is drawn: a tick and a half, so
/// there is nearly always a next stance to move toward.
const DELAY_S: f64 = 1.5 / crate::session::STANCE_HZ;

/// Where a stance put a body, relative to the body it stands on.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Anchor {
    /// From the planet's centre.
    Planet(DVec3),
    /// From the moon's centre, wherever the moon is by then.
    Moon(DVec3),
}

impl Anchor {
    fn position(self, moon: DVec3) -> DVec3 {
        match self {
            Anchor::Planet(offset) => offset,
            Anchor::Moon(offset) => moon + offset,
        }
    }

    fn up(self) -> DVec3 {
        self.offset().normalize_or(DVec3::Y)
    }

    /// From the centre of the body it stands on.
    fn offset(self) -> DVec3 {
        match self {
            Anchor::Planet(offset) | Anchor::Moon(offset) => offset,
        }
    }

    fn same_body(self, other: Anchor) -> bool {
        matches!(
            (self, other),
            (Anchor::Planet(_), Anchor::Planet(_)) | (Anchor::Moon(_), Anchor::Moon(_))
        )
    }
}

/// One stance, as heard.
#[derive(Clone, Copy, Debug)]
struct Sample {
    at_s: f64,
    anchor: Anchor,
    facing: DVec3,
    gait: Gait,
    speed_mps: f64,
    sprint: bool,
}

impl Sample {
    /// A stance as heard, or none when it says no place: a sector that is
    /// none, or a number that is not finite, which would stay in an eased
    /// lean or a drawn body for good.
    fn read(stance: &protocol::Stance, generator: &Generator, at_s: f64) -> Option<Sample> {
        let numbers = [
            stance.u,
            stance.v,
            stance.height_m,
            stance.facing_x,
            stance.facing_y,
            stance.facing_z,
            stance.speed_mps,
        ];
        if !numbers.iter().all(|number| number.is_finite()) {
            return None;
        }
        let sector = Sector::new(u8::try_from(stance.sector).ok()?)?;
        let sphere = generator.sphere();
        let point = SurfacePoint::new(sector, f64::from(stance.u), f64::from(stance.v));
        let direction = DVec3::from(sphere.blocks().direction(point));
        let height_m = f64::from(stance.height_m);
        let anchor = match stance.body() {
            protocol::Body::Planet => Anchor::Planet(direction * (sphere.radius_m() + height_m)),
            protocol::Body::Moon => Anchor::Moon(direction * (MOON_RADIUS_M + height_m)),
        };
        let facing = DVec3::new(
            f64::from(stance.facing_x),
            f64::from(stance.facing_y),
            f64::from(stance.facing_z),
        );
        Some(Sample {
            at_s,
            anchor,
            facing: facing.normalize_or(anchor.up().any_orthonormal_vector()),
            gait: Gait::from_wire(stance.gait()),
            speed_mps: f64::from(stance.speed_mps),
            sprint: stance.sprint,
        })
    }
}

pub struct Peer {
    pub info: PeerInfo,
    /// The asset reference of what this peer wears.
    pub avatar: String,
    pub figure: Figure,
    /// The two newest stances: where the body is drawn between.
    previous: Option<Sample>,
    latest: Option<Sample>,
    stride_m: f64,
    /// The way of moving its newest stance says, by its full name: empty on
    /// foot and in flight.
    way: String,
    /// The animal its way carries, by its number: zero for none.
    carrier: u64,
    /// What its newest stance says it holds, by its full name: empty for
    /// an empty hand.
    hand: String,
    /// In a way: how far it leans into a turn, radians, and how much the
    /// ground under it rises along its heading, a metre a metre. Read from
    /// the turn and the climb between its two newest stances, and eased.
    lean: f64,
    pitch: f64,
}

impl Peer {
    fn new(info: PeerInfo, avatar: String) -> Peer {
        Peer {
            info,
            avatar,
            figure: Figure::default(),
            previous: None,
            latest: None,
            stride_m: 0.0,
            way: String::new(),
            carrier: 0,
            hand: String::new(),
            lean: 0.0,
            pitch: 0.0,
        }
    }

    fn heard(&mut self, sample: Sample, stance: &protocol::Stance) {
        self.previous = self.latest.take();
        self.latest = Some(sample);
        if self.way != stance.way {
            self.way.clone_from(&stance.way);
        }
        self.carrier = match stance.way.is_empty() {
            true => 0,
            false => stance.carrier,
        };
        if self.hand != stance.hand {
            self.hand.clone_from(&stance.hand);
        }
    }

    /// The thing it holds, when one given here.
    fn thing<'t>(&self, things: &'t Things) -> Option<&'t Thing> {
        things.get(&self.hand).map(|(_, thing)| thing)
    }

    /// The way of moving it is in, when one given here.
    fn way<'w>(&self, ways: &'w Ways) -> Option<(&'static str, &'w Way)> {
        ways.get(&self.way)
    }

    /// Eases its lean and pitch toward what its two newest stances say: the
    /// turn between them at its speed, and the climb over the ground
    /// between them.
    fn glide(&mut self, way: &Way, dt: f64) {
        // On feet a body stands upright, whatever it rides: only a hover
        // leans and pitches.
        let Profile::Hover(hover) = &way.profile else {
            self.lean = 0.0;
            self.pitch = 0.0;
            return;
        };
        let (lean, pitch) = match (self.previous, self.latest) {
            (Some(previous), Some(latest))
                if previous.anchor.same_body(latest.anchor) && latest.at_s > previous.at_s =>
            {
                let span_s = latest.at_s - previous.at_s;
                let up = latest.anchor.up();
                let (from, to) = (previous.facing, latest.facing);
                let yaw_per_s = from.cross(to).dot(up).atan2(from.dot(to)) / span_s;
                let lean = hover::lean(hover, latest.speed_mps, yaw_per_s, GRAVITY_MPS2);
                let moved = latest.anchor.offset() - previous.anchor.offset();
                let along_m = (moved - up * moved.dot(up)).length();
                let rise_m = latest.anchor.offset().length() - previous.anchor.offset().length();
                let pitch = if along_m > 0.05 {
                    rise_m / along_m
                } else {
                    0.0
                };
                (lean, pitch.clamp(-1.0, 1.0))
            }
            _ => (0.0, 0.0),
        };
        let ease = 1.0 - (-hover::LEAN_PER_S * dt).exp();
        self.lean += (lean - self.lean) * ease;
        self.pitch += (pitch - self.pitch) * ease;
    }

    /// Where to draw the body at `now`, or nowhere until a stance has come.
    /// In a way of moving it stands tilted with the ground, leaned into the
    /// turn and bobbing, as a body in it here does.
    fn motion_at(&self, now_s: f64, moon: DVec3, way: Option<&Way>) -> Option<Motion> {
        let latest = self.latest?;
        let render_s = now_s - DELAY_S;
        let (anchor, facing) = match self.previous {
            Some(previous)
                if previous.anchor.same_body(latest.anchor) && render_s < latest.at_s =>
            {
                let span = latest.at_s - previous.at_s;
                let t = if span > 1e-6 {
                    ((render_s - previous.at_s) / span).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let anchor = match (previous.anchor, latest.anchor) {
                    (Anchor::Planet(a), Anchor::Planet(b)) => Anchor::Planet(a.lerp(b, t)),
                    (Anchor::Moon(a), Anchor::Moon(b)) => Anchor::Moon(a.lerp(b, t)),
                    (_, b) => b,
                };
                let facing = previous
                    .facing
                    .lerp(latest.facing, t)
                    .normalize_or(latest.facing);
                (anchor, facing)
            }
            _ => (latest.anchor, latest.facing),
        };
        let up = anchor.up();
        let hover = way.and_then(|way| match &way.profile {
            Profile::Hover(hover) => Some(hover),
            Profile::Paces(_) => None,
        });
        let (position, stands) = match hover {
            Some(hover) => {
                let tilt = hover::normal(up, facing * self.pitch);
                (
                    anchor.position(moon) + up * hover::bob_m(hover, now_s),
                    hover::leaned(tilt, facing, self.lean),
                )
            }
            None => (anchor.position(moon), up),
        };
        Some(Motion {
            position,
            basis: figure::basis(stands, facing),
            gait: latest.gait,
            speed_mps: latest.speed_mps,
            sprint: latest.sprint,
            stride_m: self.stride_m,
        })
    }
}

#[derive(Default)]
pub struct Peers {
    by_session: HashMap<u32, Peer>,
    /// The client's clock, seconds: what samples are stamped with.
    now_s: f64,
}

impl Peers {
    pub fn add(&mut self, peer: &protocol::Peer, generator: &Generator) {
        let info = PeerInfo {
            session: peer.session,
            name: peer.name.clone(),
            visitor: peer.visitor,
        };
        let mut added = Peer::new(info, peer.avatar.clone());
        if let Some(stance) = &peer.stance
            && let Some(sample) = Sample::read(stance, generator, self.now_s)
        {
            added.heard(sample, stance);
        }
        self.by_session.insert(peer.session, added);
    }

    pub fn remove(&mut self, session: u32) -> Option<Peer> {
        self.by_session.remove(&session)
    }

    pub fn clear(&mut self) {
        self.by_session.clear();
    }

    pub fn moved(&mut self, session: u32, stance: &protocol::Stance, generator: &Generator) {
        if let (Some(peer), Some(sample)) = (
            self.by_session.get_mut(&session),
            Sample::read(stance, generator, self.now_s),
        ) {
            peer.heard(sample, stance);
        }
    }

    pub fn wearing(&mut self, session: u32, avatar: &str) {
        if let Some(peer) = self.by_session.get_mut(&session) {
            peer.avatar = avatar.to_owned();
        }
    }

    pub fn renamed(&mut self, session: u32, name: &str) {
        if let Some(peer) = self.by_session.get_mut(&session) {
            peer.info.name = name.to_owned();
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Peer> {
        self.by_session.values()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Peer> {
        self.by_session.values_mut()
    }

    /// Where each one that rides no animal stands now, world space.
    pub fn on_foot(&self, moon: DVec3, ways: &Ways) -> impl Iterator<Item = DVec3> {
        self.by_session
            .values()
            .filter(|peer| peer.carrier == 0)
            .filter_map(move |peer| {
                let way = peer.way(ways).map(|(_, way)| way);
                Some(peer.motion_at(self.now_s, moon, way)?.position)
            })
    }

    /// Who is here, for the seam. In session order, so a list stays put.
    pub fn infos(&self) -> Vec<PeerInfo> {
        let mut infos: Vec<PeerInfo> = self.by_session.values().map(|p| p.info.clone()).collect();
        infos.sort_by_key(|info| info.session);
        infos
    }

    /// Time passes: every peer's figure moves on.
    pub fn update(&mut self, dt: f64, moon: DVec3, clips: &Clips, ways: &Ways, things: &Things) {
        self.now_s += dt;
        for peer in self.by_session.values_mut() {
            let way = peer.way(ways).map(|(_, way)| way);
            if let Some(way) = way {
                peer.glide(way, dt);
            }
            let Some(motion) = peer.motion_at(self.now_s, moon, way) else {
                continue;
            };
            if motion.gait.grounded() {
                peer.stride_m += motion.speed_mps * dt;
            }
            peer.figure
                .update(dt, &motion, clips, way.map(|way| &way.held));
            let arm = peer.thing(things).map(|thing| thing.arm.as_slice());
            peer.figure.grasp(dt, arm);
        }
    }

    /// Every peer that has said where it is, and where to draw it now.
    /// Where each peer's head is now, world space, by session: what a label
    /// is hung over.
    pub fn heads(&self, moon: DVec3, ways: &Ways) -> impl Iterator<Item = (u32, DVec3)> {
        self.by_session.iter().filter_map(move |(session, peer)| {
            let way = peer.way(ways).map(|(_, way)| way);
            let motion = peer.motion_at(self.now_s, moon, way)?;
            Some((
                *session,
                motion.position + motion.basis.y_axis * peer.figure.label_m(),
            ))
        })
    }

    /// Where a peer is `over_m` above its feet now, world space, along its
    /// own up: where what it sounds comes from. None until it said where.
    pub fn over_feet(&self, session: u32, over_m: f64, moon: DVec3, ways: &Ways) -> Option<DVec3> {
        let peer = self.by_session.get(&session)?;
        let way = peer.way(ways).map(|(_, way)| way);
        let motion = peer.motion_at(self.now_s, moon, way)?;
        Some(motion.position + motion.basis.y_axis * over_m)
    }

    /// Every peer that has said where it is: its session, its figure, where
    /// to draw it now, the animal it rides, zero for none, and the thing it
    /// holds.
    pub fn bodies<'t>(
        &self,
        moon: DVec3,
        ways: &Ways,
        things: &'t Things,
    ) -> impl Iterator<Item = (u32, &Figure, Motion, u64, Option<&'t Thing>)> {
        self.by_session.iter().filter_map(move |(&session, peer)| {
            let way = peer.way(ways).map(|(_, way)| way);
            let motion = peer.motion_at(self.now_s, moon, way)?;
            Some((
                session,
                &peer.figure,
                motion,
                peer.carrier,
                peer.thing(things),
            ))
        })
    }

    /// Every peer in a way of moving given here: its session, the plugin
    /// that gave the way, the way, the animal it carries, and where to draw
    /// the body now.
    pub fn in_ways<'w>(
        &self,
        moon: DVec3,
        ways: &'w Ways,
    ) -> impl Iterator<Item = (u32, &'static str, &'w Way, u64, Motion)> {
        self.by_session.iter().filter_map(move |(session, peer)| {
            let (plugin, way) = peer.way(ways)?;
            let motion = peer.motion_at(self.now_s, moon, Some(way))?;
            Some((*session, plugin, way, peer.carrier, motion))
        })
    }
}
