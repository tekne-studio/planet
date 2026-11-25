//! What is heard: every source of sound in the world this frame, where it
//! stands and how far it carries, the voices struck at a place, and the
//! voices sustained while their owners name them. The
//! shell mixes them: one listener at the eye, a panner a source, with the
//! platform's own audio. A picture that moves and carries sound, a clip
//! placed in the world and a cabinet's game are sources; a peer's speech,
//! heard by proximity, is the next, over the same list.

use std::collections::HashMap;

use glam::DVec3;
use part::PartId;
use scene::PictureId;
use synth::Voice;

/// Who makes a sound.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SoundSource {
    /// A playing picture: the shell holds its video, and the sound is its.
    Picture(PictureId),
    /// A sound placed in the world: the shell plays its file.
    Clip(PartId),
    /// A cabinet's game, running: the shell runs it, and the sound is what
    /// it plays, as the shell hears it.
    Game(PictureId),
}

/// One source of sound this frame.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Sound {
    pub source: SoundSource,
    /// Where it stands, in world space.
    pub position: DVec3,
    /// How far it is heard, metres: silent from there on.
    pub reach_m: f64,
    /// How loud, 0 to 1, as its part says.
    pub volume: f32,
}

/// How many seeds the strikes of one voice turn through: two in a row are
/// never one sample, and a shell keeps a few buffers of each voice.
pub const VARIANTS: u32 = 6;

/// How many struck voices a shell plays at once, the nearest first: the
/// rest are not heard.
pub const STRUCK_AT_ONCE: usize = 16;

/// How many sustained voices are heard at once, the nearest first.
pub const SUSTAINS_AT_ONCE: usize = 4;

/// What a struck voice is, as a person would name it to turn it up or down:
/// the shell mixes each group on a bus of its own. The core names a
/// voice's group where it strikes or sustains it. The groups are closed: a
/// new kind of voice takes one of these.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Group {
    /// What a body's motion makes: a hoof coming down, a wing's beat.
    Steps,
    /// What a body calls out, its own or a peer's: the whistle.
    Calls,
}

/// A voice struck at a place, now: the shell renders it once, for its seed,
/// and plays it there, falling off to silence at its reach.
#[derive(Clone, PartialEq, Debug)]
pub struct Strike {
    pub voice: Voice,
    pub group: Group,
    pub seed: u32,
    /// Where it sounds, in world space.
    pub position: DVec3,
    pub reach_m: f64,
}

/// A voice sounding for as long as its owner names it each frame, of a
/// group, at a place: the shell loops it, eases to its level and its tone,
/// and fades it out the first frame it is not named.
#[derive(Clone, PartialEq, Debug)]
pub struct Sustain {
    /// Whose it is: one owner, one voice going on from frame to frame.
    pub key: u64,
    pub voice: Voice,
    pub group: Group,
    /// Where it sounds, in world space.
    pub position: DVec3,
    pub reach_m: f64,
    /// How loud, 0 to 1 of the voice.
    pub level: f32,
    /// How bright, 0 dark to 1 the voice as written.
    pub tone: f32,
}

/// The voices struck since the shell last took them. A strike is heard for
/// one frame: one the shell did not take, a shell with no sound or an agent,
/// is gone at the next.
#[derive(Default)]
pub(crate) struct Strikes {
    struck: Vec<Strike>,
    /// How many of `struck` the last frame heard.
    heard: usize,
    /// How many times each voice was struck, by its key: its next seed.
    turns: HashMap<Vec<u8>, u32>,
}

/// The voices sustained this frame, named again every frame they sound.
#[derive(Default)]
pub(crate) struct Sustains {
    sounding: Vec<Sustain>,
}

impl Sustains {
    /// Opens a frame: nothing sounds until it is named again.
    pub fn begin_frame(&mut self) {
        self.sounding.clear();
    }

    /// A voice sounds on this frame for `key`, its owner.
    pub fn sustain(&mut self, sustain: Sustain) {
        self.sounding.push(sustain);
    }

    /// Keeps what the eye is within reach of, the nearest first, at most
    /// [`SUSTAINS_AT_ONCE`].
    pub fn hear(&mut self, eye: DVec3) {
        self.sounding
            .retain(|sustain| sustain.position.distance(eye) <= sustain.reach_m);
        self.sounding.sort_by(|a, b| {
            a.position
                .distance_squared(eye)
                .total_cmp(&b.position.distance_squared(eye))
        });
        self.sounding.truncate(SUSTAINS_AT_ONCE);
    }

    pub fn heard(&self) -> &[Sustain] {
        &self.sounding
    }
}

impl Strikes {
    /// Opens a frame: what the last one heard and nobody took is let go;
    /// what was struck since, by a command, stays.
    pub fn begin_frame(&mut self) {
        self.struck.drain(..self.heard);
        self.heard = 0;
    }

    /// A voice sounds at a place, with the next of its seeds: the seed it
    /// took.
    pub fn strike(&mut self, voice: &Voice, group: Group, position: DVec3, reach_m: f64) -> u32 {
        let turn = self.turns.entry(voice.key()).or_default();
        let seed = *turn % VARIANTS;
        *turn = turn.wrapping_add(1);
        self.struck_with(voice, group, seed, position, reach_m);
        seed
    }

    /// A voice struck at a place with a seed it was given: a peer's, as its
    /// own client struck it.
    pub fn struck_with(
        &mut self,
        voice: &Voice,
        group: Group,
        seed: u32,
        position: DVec3,
        reach_m: f64,
    ) {
        self.struck.push(Strike {
            voice: voice.clone(),
            group,
            seed,
            position,
            reach_m,
        });
    }

    /// Keeps what the eye is within reach of, the nearest first.
    pub fn hear(&mut self, eye: DVec3) {
        self.struck
            .retain(|strike| strike.position.distance(eye) <= strike.reach_m);
        self.struck.sort_by(|a, b| {
            a.position
                .distance_squared(eye)
                .total_cmp(&b.position.distance_squared(eye))
        });
        self.heard = self.struck.len();
    }

    pub fn drain(&mut self) -> Vec<Strike> {
        self.heard = 0;
        core::mem::take(&mut self.struck)
    }
}

#[cfg(test)]
mod tests {
    use synth::Vary;

    use super::*;

    fn wind(key: u64, x: f64) -> Sustain {
        Sustain {
            key,
            voice: Voice::new("wind", Vary::NONE, &[]),
            group: Group::Steps,
            position: DVec3::new(x, 0.0, 0.0),
            reach_m: 40.0,
            level: 0.5,
            tone: 0.5,
        }
    }

    /// A sustained voice is heard within its reach, the nearest first, a
    /// few at once, and only while it is named.
    #[test]
    fn sustains_are_heard_near_first_and_only_while_named() {
        let mut sustains = Sustains::default();
        for (key, x) in [
            (1, 30.0),
            (2, 50.0),
            (3, 5.0),
            (4, 20.0),
            (5, 10.0),
            (6, 35.0),
        ] {
            sustains.sustain(wind(key, x));
        }
        sustains.hear(DVec3::ZERO);
        let keys: Vec<u64> = sustains.heard().iter().map(|sustain| sustain.key).collect();
        assert_eq!(keys, [3, 5, 4, 1], "beyond reach and past four, none");
        sustains.begin_frame();
        sustains.hear(DVec3::ZERO);
        assert!(sustains.heard().is_empty(), "not named, not heard");
    }
}
