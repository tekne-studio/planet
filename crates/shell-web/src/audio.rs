//! What is heard, mixed at the eye with Web Audio: one listener, a panner a
//! source. The engine says where each source stands this frame and how far
//! it carries (`client::Sound`); the shell owns what sounds: a playing
//! video's element, a placed clip, a cabinet's game, and tomorrow a peer's
//! speech, heard by proximity, over the same panner. A game sounds through
//! a node the page makes for it, the arcade's chip in an AudioWorklet, fed
//! every update the game runs, each to be played one update after the
//! last, so the chip plays each tone on the sample the game played it
//! (`Audio::open_game`). A voice the engine strikes
//! (`client::Strike`) is rendered once for its seed into a buffer, kept,
//! and played where it was struck. A voice the engine sustains
//! (`client::Sustain`) is rendered once as a loop and played round, eased
//! to the level and tone it is named with each frame, and faded out the
//! first frame it is not; while the page is hidden every sustained voice
//! eases to silence, and back as it shows. A game's stream holds with its
//! game, left by its player or its player away, eased out and kept where
//! it is, and eased in from there as it runs on. What else
//! sounds goes on hidden: a struck voice plays out, and a video or clip
//! plays on as loud as it was.
//! Positions are given relative to the
//! eye, which sits at the origin looking down `-z`, so the world's size
//! never meets the graph's f32. The browser lets sound out only after a gesture: the context
//! resumes on the first press or key, and a video stays muted until then.
//! Everything goes through one mix: a bus for what is played in the world,
//! one for the games, and one for each group of struck voices
//! (`client::Group`), under one for everything, each as loud as the person
//! set it (`Loudness`).

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use client::{Group, STRUCK_AT_ONCE, Sound, SoundSource, Strike, Sustain};
use glam::DVec3;
use js_sys::{Array, Function, Object, Promise, Reflect, Uint32Array};
use scene::{Camera, PictureId};
use serde::Deserialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState, AudioNode,
    AudioScheduledSourceNode, AudioWorkletNode, BiquadFilterNode, BiquadFilterType,
    DistanceModelType, GainNode, HtmlAudioElement, HtmlMediaElement, MediaElementAudioSourceNode,
    PannerNode, PanningModelType,
};

/// Within this far of a source the sound is at its full; from there to its
/// reach it falls off in a straight line.
const FULL_M: f64 = 1.0;

/// How many voices are kept rendered, by voice and seed: every seed of the
/// whistle, and room for many more. The oldest goes first.
const RENDERED: usize = 128;

/// How loud the struck voices and the games are at their bus's full level:
/// the room they leave the world's sounds.
const VOICES_GAIN: f32 = 0.8;

/// How long a game's stream stays connected once ended, seconds: its
/// channels' way out, which its processor renders before it stops.
const STREAM_TAIL_S: f64 = 0.1;

/// How many batches of a game's updates wait while the page makes its node:
/// a quarter of a second at a frame each.
const WAITING_BATCHES: usize = 15;

/// How long a level takes to reach a new setting, seconds: a slider dragged
/// is heard at once, without a click.
const LEVEL_EASE_S: f64 = 0.015;

/// How long a sustained voice's loop is, seconds.
const LOOP_S: f32 = 2.0;

/// How long a sustained voice takes to reach its level and tone, seconds:
/// what fades it in as it is first named and out as it is not.
const SUSTAIN_EASE_S: f64 = 0.3;

/// How long after its last frame a sustained voice is let go, seconds: its
/// fade out done.
const SUSTAIN_GONE_S: f64 = 5.0 * SUSTAIN_EASE_S;

/// The cutoff a sustained voice is darkened to at tone 0 and opened to at
/// 1, Hz: between them by octaves.
const TONE_HZ: (f32, f32) = (300.0, 8000.0);

/// How loud each group of what is heard is, 0 to 1, under one level for
/// everything and a mute, as the person set them. The page keeps them.
#[derive(Clone, Copy, PartialEq, Debug, Deserialize)]
#[serde(default)]
pub struct Loudness {
    pub master: f32,
    pub muted: bool,
    /// What is played in the world: videos and sound parts.
    pub world: f32,
    pub steps: f32,
    pub calls: f32,
    pub games: f32,
}

impl Default for Loudness {
    fn default() -> Self {
        Self {
            master: 1.0,
            muted: false,
            world: 1.0,
            steps: 1.0,
            calls: 1.0,
            games: 1.0,
        }
    }
}

impl Loudness {
    fn everything(&self) -> f32 {
        if self.muted { 0.0 } else { level(self.master) }
    }

    fn group(&self, group: Group) -> f32 {
        let value = match group {
            Group::Steps => self.steps,
            Group::Calls => self.calls,
        };
        level(value) * VOICES_GAIN
    }

    fn games(&self) -> f32 {
        level(self.games) * VOICES_GAIN
    }
}

/// A level as the mix takes it: 0 to 1, and nothing that is not a number.
fn level(value: f32) -> f32 {
    if value.is_nan() {
        1.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

/// The buses everything heard goes through, made with the context.
struct Mix {
    everything: GainNode,
    world: GainNode,
    steps: GainNode,
    calls: GainNode,
    games: GainNode,
}

impl Mix {
    fn new(context: &AudioContext, loudness: &Loudness) -> Result<Self, JsValue> {
        let everything = context.create_gain()?;
        everything.connect_with_audio_node(&context.destination())?;
        let bus = || -> Result<GainNode, JsValue> {
            let bus = context.create_gain()?;
            bus.connect_with_audio_node(&everything)?;
            Ok(bus)
        };
        let mix = Self {
            world: bus()?,
            steps: bus()?,
            calls: bus()?,
            games: bus()?,
            everything,
        };
        mix.set(loudness, None);
        Ok(mix)
    }

    /// Each bus as loud as `loudness` says: at once, or eased from `now_s`.
    fn set(&self, loudness: &Loudness, now_s: Option<f64>) {
        let set = |bus: &GainNode, value: f32| match now_s {
            Some(now_s) => {
                let _ = bus.gain().set_target_at_time(value, now_s, LEVEL_EASE_S);
            }
            None => bus.gain().set_value(value),
        };
        set(&self.everything, loudness.everything());
        set(&self.world, level(loudness.world));
        set(&self.steps, loudness.group(Group::Steps));
        set(&self.calls, loudness.group(Group::Calls));
        set(&self.games, loudness.games());
    }

    fn voices(&self, group: Group) -> &GainNode {
        match group {
            Group::Steps => &self.steps,
            Group::Calls => &self.calls,
        }
    }

    /// The bus a source is heard on.
    fn of(&self, source: SoundSource) -> &GainNode {
        match source {
            SoundSource::Picture(_) | SoundSource::Clip(_) => &self.world,
            SoundSource::Game(_) => &self.games,
        }
    }
}

/// What a source's sound comes from.
enum Origin {
    /// A video's element, or a clip's.
    Element(HtmlMediaElement, MediaElementAudioSourceNode),
    /// A game's stream: the node the page made for it, fed what it plays.
    Stream(AudioWorkletNode),
}

/// One source in the graph: what its sound comes from, its panner.
struct Panned {
    origin: Origin,
    /// How loud: the part's volume while the engine names it, silence while
    /// it does not.
    gain: GainNode,
    panner: PannerNode,
    /// Where it stood when last heard, relative to the eye.
    at: DVec3,
    volume: f32,
}

impl Panned {
    /// Enters the graph, silent, through a panner of its own, on `bus`.
    fn new(context: &AudioContext, origin: Origin, bus: &GainNode) -> Result<Panned, JsValue> {
        let gain = context.create_gain()?;
        gain.gain().set_value(0.0);
        let panner = placed(context, DVec3::ZERO, FULL_M)?;
        let node: &AudioNode = match &origin {
            Origin::Element(_, node) => node,
            Origin::Stream(node) => node,
        };
        node.connect_with_audio_node(&gain)?;
        gain.connect_with_audio_node(&panner)?;
        panner.connect_with_audio_node(bus)?;
        Ok(Panned {
            origin,
            gain,
            panner,
            at: DVec3::ZERO,
            volume: 0.0,
        })
    }

    /// Leaves the graph.
    fn disconnect(&self) {
        match &self.origin {
            Origin::Element(_, node) => node.disconnect().ok(),
            Origin::Stream(node) => node.disconnect().ok(),
        };
        self.gain.disconnect().ok();
        self.panner.disconnect().ok();
    }

    /// Says `message` to its stream, when it is one.
    fn tell(&self, message: &JsValue) {
        if let Origin::Stream(node) = &self.origin
            && let Ok(port) = node.port()
        {
            port.post_message(message).ok();
        }
    }

    /// Gives its stream a game's updates, the array's memory with them:
    /// nothing is copied on the way to the audio thread.
    fn send(&self, played: &Uint32Array) {
        if let Origin::Stream(node) = &self.origin
            && let Ok(port) = node.port()
        {
            port.post_message_with_transferable(played, &Array::of1(&played.buffer()))
                .ok();
        }
    }
}

/// A message to a game's stream, `{ <key>: true }`.
fn flag(key: &str) -> JsValue {
    let message = Object::new();
    Reflect::set(&message, &key.into(), &JsValue::TRUE).ok();
    message.into()
}

/// A node the page made for a game: none when it could not.
struct Made {
    picture: PictureId,
    /// The open it answers: a game closed and opened again before its node
    /// came asks for another.
    open: u32,
    node: Option<AudioWorkletNode>,
}

/// The games' streams: made by the page as each game opens, fed while it
/// runs, ended as it closes.
#[derive(Default)]
struct Games {
    /// What makes a game's node: the page's, `(context) => Promise<node>`.
    make: Option<Function>,
    /// The games whose node is on its way, by picture: the open it answers,
    /// and what they played meanwhile, the last few batches.
    opening: HashMap<PictureId, (u32, VecDeque<Uint32Array>)>,
    opens: u32,
    /// The nodes made since the last frame.
    made: Rc<RefCell<Vec<Made>>>,
    /// The games held, their stream held with them, its node made or not.
    held: HashSet<PictureId>,
    /// Streams ended, each connected until its tail is out, on the
    /// context's clock.
    ending: Vec<(Panned, f64)>,
}

/// A struck voice playing, its nodes let go once it has ended.
struct Struck {
    source: AudioBufferSourceNode,
    panner: PannerNode,
    /// When it has ended, on the context's clock.
    ends_s: f64,
}

/// The voices struck: rendered once each, then played as often as struck.
#[derive(Default)]
struct Voices {
    rendered: HashMap<(Vec<u8>, u32), AudioBuffer>,
    /// The order they were rendered in: the first leaves first.
    order: VecDeque<(Vec<u8>, u32)>,
    playing: Vec<Struck>,
}

impl Voices {
    /// The voice rendered for its seed at the context's rate, kept for the
    /// next strike.
    fn rendered(
        &mut self,
        context: &AudioContext,
        strike: &Strike,
    ) -> Result<AudioBuffer, JsValue> {
        let key = (strike.voice.key(), strike.seed);
        if let Some(buffer) = self.rendered.get(&key) {
            return Ok(buffer.clone());
        }
        let rate = context.sample_rate();
        let mut samples = vec![0.0; strike.voice.length(strike.seed, rate as u32).max(1)];
        strike.voice.render(strike.seed, rate as u32, &mut samples);
        let buffer = context.create_buffer(1, samples.len() as u32, rate)?;
        buffer.copy_to_channel(&samples, 0)?;
        if self.order.len() == RENDERED
            && let Some(oldest) = self.order.pop_front()
        {
            self.rendered.remove(&oldest);
        }
        self.order.push_back(key.clone());
        self.rendered.insert(key, buffer.clone());
        Ok(buffer)
    }

    /// Plays a strike once, `at` where it sounds relative to the eye, on
    /// its group's `bus`.
    fn play(
        &mut self,
        context: &AudioContext,
        bus: &GainNode,
        strike: &Strike,
        at: DVec3,
    ) -> Result<(), JsValue> {
        let buffer = self.rendered(context, strike)?;
        let source = context.create_buffer_source()?;
        source.set_buffer(Some(&buffer));
        let panner = placed(context, at, strike.reach_m)?;
        source.connect_with_audio_node(&panner)?;
        panner.connect_with_audio_node(bus)?;
        source.start()?;
        self.playing.push(Struck {
            source,
            panner,
            ends_s: context.current_time() + buffer.duration(),
        });
        Ok(())
    }

    /// Lets go of what has ended by `now`.
    fn ended(&mut self, now_s: f64) {
        self.playing.retain(|struck| {
            let playing = struck.ends_s > now_s;
            if !playing {
                struck.source.disconnect().ok();
                struck.panner.disconnect().ok();
            }
            playing
        });
    }
}

/// A sustained voice playing round, by its key.
struct Sustained {
    source: AudioBufferSourceNode,
    filter: BiquadFilterNode,
    gain: GainNode,
    panner: PannerNode,
    level: f32,
    tone: f32,
    at: DVec3,
    /// When it is let go, on the context's clock: set the first frame it
    /// is not named, as it fades out.
    gone_s: Option<f64>,
    /// The last frame that named it.
    named: u64,
}

impl Sustained {
    fn stop(&self) {
        AsRef::<AudioScheduledSourceNode>::as_ref(&self.source)
            .stop()
            .ok();
        self.source.disconnect().ok();
        self.filter.disconnect().ok();
        self.gain.disconnect().ok();
        self.panner.disconnect().ok();
    }
}

/// The voices sustained: each voice's loop rendered once, and what plays
/// by key.
#[derive(Default)]
struct Sustains {
    loops: HashMap<Vec<u8>, AudioBuffer>,
    playing: HashMap<u64, Sustained>,
    /// Those that could not start, by key, at the last frame that named
    /// them: tried again only once a frame has left them out.
    failed: HashMap<u64, u64>,
    /// The frames sustained so far.
    frame: u64,
    /// Whether the page is hidden: every voice is silent until it shows.
    hidden: bool,
}

impl Sustains {
    /// The voice's loop at the context's rate, kept for every key that
    /// sustains it.
    fn looped(
        &mut self,
        context: &AudioContext,
        sustain: &Sustain,
    ) -> Result<AudioBuffer, JsValue> {
        let key = sustain.voice.key();
        if let Some(buffer) = self.loops.get(&key) {
            return Ok(buffer.clone());
        }
        let rate = context.sample_rate();
        let samples = sustain.voice.looped(0, rate as u32, LOOP_S);
        let buffer = context.create_buffer(1, samples.len().max(1) as u32, rate)?;
        buffer.copy_to_channel(&samples, 0)?;
        self.loops.insert(key, buffer.clone());
        Ok(buffer)
    }

    /// Starts a sustained voice, silent, fading in to its level.
    fn start(
        &mut self,
        context: &AudioContext,
        bus: &GainNode,
        sustain: &Sustain,
        at: DVec3,
    ) -> Result<Sustained, JsValue> {
        let buffer = self.looped(context, sustain)?;
        let source = context.create_buffer_source()?;
        source.set_buffer(Some(&buffer));
        source.set_loop(true);
        let filter = context.create_biquad_filter()?;
        filter.set_type(BiquadFilterType::Lowpass);
        filter.frequency().set_value(tone_hz(sustain.tone));
        let gain = context.create_gain()?;
        gain.gain().set_value(0.0);
        let panner = placed(context, at, sustain.reach_m)?;
        source.connect_with_audio_node(&filter)?;
        filter.connect_with_audio_node(&gain)?;
        gain.connect_with_audio_node(&panner)?;
        panner.connect_with_audio_node(bus)?;
        source.start()?;
        Ok(Sustained {
            source,
            filter,
            gain,
            panner,
            level: f32::NAN,
            tone: sustain.tone,
            at,
            gone_s: None,
            named: self.frame,
        })
    }
}

/// The cutoff for a tone, 0 to 1: between [`TONE_HZ`] by octaves.
fn tone_hz(tone: f32) -> f32 {
    let (dark, bright) = TONE_HZ;
    dark * (bright / dark).powf(tone.clamp(0.0, 1.0))
}

/// A panner that hears a source `at` this place relative to the eye: full
/// within [`FULL_M`], falling off in a straight line to silence at its reach.
fn placed(context: &AudioContext, at: DVec3, reach_m: f64) -> Result<PannerNode, JsValue> {
    let panner = context.create_panner()?;
    panner.set_panning_model(PanningModelType::Hrtf);
    panner.set_distance_model(DistanceModelType::Linear);
    panner.set_ref_distance(FULL_M);
    panner.set_rolloff_factor(1.0);
    panner.set_max_distance(reach_m.max(FULL_M + 0.01));
    let [x, y, z] = at.as_vec3().to_array();
    panner.position_x().set_value(x);
    panner.position_y().set_value(y);
    panner.position_z().set_value(z);
    Ok(panner)
}

#[derive(Default)]
pub struct Audio {
    /// Made at the first source or gesture; suspended until a gesture.
    context: Option<AudioContext>,
    /// The buses, made with the context.
    mix: Option<Mix>,
    /// As loud as the person set it: the mix takes it when it is made.
    loudness: Loudness,
    /// Whether a gesture has happened: the context runs, or is resuming.
    unlocked: bool,
    /// Whether the context ran at the last frame: a game's stream is fed
    /// only then, so nothing waits on a device that renders nothing.
    runs: bool,
    sources: HashMap<SoundSource, Panned>,
    voices: Voices,
    sustains: Sustains,
    games: Games,
}

impl Drop for Audio {
    /// The engine let go: every game's stream ended, every element quiet,
    /// and the context closed, so no processor renders on and the device
    /// is given back.
    fn drop(&mut self) {
        let end = flag("end");
        for panned in self.sources.values() {
            if let Origin::Element(element, _) = &panned.origin {
                element.set_muted(true);
                let _ = element.pause();
            }
            panned.tell(&end);
            panned.disconnect();
        }
        for (panned, _) in &self.games.ending {
            panned.disconnect();
        }
        if let Some(context) = &self.context {
            let _ = context.close();
        }
    }
}

impl Audio {
    /// The context and its mix, made the first time either is wanted.
    fn context(&mut self) -> Option<(&AudioContext, &Mix)> {
        if self.context.is_none() {
            // A context whose mix cannot be made is closed, not left open.
            let made =
                AudioContext::new().and_then(|context| match Mix::new(&context, &self.loudness) {
                    Ok(mix) => Ok((mix, context)),
                    Err(error) => {
                        let _ = context.close();
                        Err(error)
                    }
                });
            match made {
                Ok((mix, context)) => {
                    self.mix = Some(mix);
                    self.context = Some(context);
                }
                Err(error) => log::warn!("no audio: {error:?}"),
            }
        }
        self.context.as_ref().zip(self.mix.as_ref())
    }

    /// How loud each group is, heard at once: eased into a mix that plays,
    /// taken by the one made later.
    pub fn set_loudness(&mut self, loudness: Loudness) {
        self.loudness = loudness;
        if let Some((context, mix)) = self.context.as_ref().zip(self.mix.as_ref()) {
            mix.set(&loudness, Some(context.current_time()));
        }
    }

    /// A gesture happened: the browser now lets sound out.
    pub fn unlock(&mut self) {
        if let Some((context, _)) = self.context() {
            if context.state() == AudioContextState::Suspended {
                let _ = context.resume();
            }
            self.unlocked = true;
        }
    }

    /// Whether sound goes out: the context runs, a gesture having happened.
    fn running(&self) -> bool {
        self.context
            .as_ref()
            .is_some_and(|context| context.state() == AudioContextState::Running)
    }

    /// A source's element enters the graph, through a panner of its own.
    /// It stays muted until sound may go out.
    pub fn attach(&mut self, source: SoundSource, element: &HtmlMediaElement) {
        let Some((context, mix)) = self.context() else {
            return;
        };
        let made = context
            .create_media_element_source(element)
            .and_then(|node| {
                let origin = Origin::Element(element.clone(), node);
                Panned::new(context, origin, mix.of(source))
            });
        match made {
            Ok(panned) => {
                self.sources.insert(source, panned);
            }
            Err(error) => log::warn!("{source:?}: not heard: {error:?}"),
        }
    }

    /// A source leaves the graph, and its element goes quiet.
    pub fn detach(&mut self, source: SoundSource) {
        if let Some(panned) = self.sources.remove(&source) {
            if let Origin::Element(element, _) = &panned.origin {
                element.set_muted(true);
                let _ = element.pause();
            }
            panned.disconnect();
        }
    }

    /// A clip placed in the world starts playing, looping, off an audio
    /// element of its own in the graph: muted until sound may go out.
    pub fn open_clip(&mut self, source: SoundSource, url: &str) {
        let element = match HtmlAudioElement::new() {
            Ok(element) => element,
            Err(error) => {
                log::warn!("{source:?}: no audio element: {error:?}");
                return;
            }
        };
        element.set_loop(true);
        element.set_muted(true);
        element.set_autoplay(true);
        element.set_preload("auto");
        element.set_src(url);
        if let Ok(playing) = element.play() {
            let quiet = wasm_bindgen::closure::Closure::<dyn FnMut(JsValue)>::new(|_| ());
            let _ = playing.catch(&quiet);
            quiet.forget();
        }
        self.attach(source, &element);
    }

    /// A clip stops: its element leaves the graph and lets its file go.
    pub fn close_clip(&mut self, source: SoundSource) {
        if let Some(Panned {
            origin: Origin::Element(element, _),
            ..
        }) = self.sources.get(&source)
        {
            element.remove_attribute("src").ok();
            element.load();
        }
        self.detach(source);
    }

    /// What makes the node a game sounds through, the page's: called with
    /// the context, it gives a promise of an AudioWorkletNode that takes a
    /// game's batches, `{ hold: true }`, `{ resume: true }` and
    /// `{ end: true }`.
    pub fn set_games(&mut self, make: Function) {
        self.games.make = Some(make);
    }

    /// A game starts running in `picture`: the page makes its node, and
    /// what it plays meanwhile waits for it. A page with no maker hears no
    /// game.
    pub fn open_game(&mut self, picture: PictureId) {
        self.close_game(picture);
        let Some(make) = self.games.make.clone() else {
            return;
        };
        let Some((context, _)) = self.context() else {
            return;
        };
        let promise = match make.call1(&JsValue::NULL, context) {
            Ok(promise) => Promise::resolve(&promise),
            Err(error) => {
                log::warn!("a game's sound was not made: {error:?}");
                return;
            }
        };
        self.games.opens = self.games.opens.wrapping_add(1);
        let open = self.games.opens;
        self.games.opening.insert(picture, (open, VecDeque::new()));
        let made = Rc::clone(&self.games.made);
        wasm_bindgen_futures::spawn_local(async move {
            let node = match JsFuture::from(promise).await {
                Ok(node) => node.dyn_into::<AudioWorkletNode>().ok(),
                Err(error) => {
                    log::warn!("a game's sound was not made: {error:?}");
                    None
                }
            };
            made.borrow_mut().push(Made {
                picture,
                open,
                node,
            });
        });
    }

    /// What the game in `picture` played since the last frame, the
    /// emulator's batches in order, for its stream to play one update
    /// apart (the processor the pinned emulator carries). What a game plays
    /// while the context does not run is let go: before the first gesture,
    /// while it resumes, or once the device was taken from it.
    pub fn play_game(&mut self, picture: PictureId, played: &VecDeque<Vec<u32>>) {
        if !self.runs {
            return;
        }
        for batch in played {
            let batch = Uint32Array::from(&batch[..]);
            if let Some(panned) = self.sources.get(&SoundSource::Game(picture)) {
                panned.send(&batch);
            } else if let Some((_, waiting)) = self.games.opening.get_mut(&picture) {
                if waiting.len() == WAITING_BATCHES {
                    waiting.pop_front();
                }
                waiting.push_back(batch);
            }
        }
    }

    /// The game in `picture` holds, its player gone: its stream eases out
    /// and holds where it is, what it is still to play kept.
    pub fn hold_game(&mut self, picture: PictureId) {
        self.games.held.insert(picture);
        if let Some(panned) = self.sources.get(&SoundSource::Game(picture)) {
            panned.tell(&flag("hold"));
        }
    }

    /// The game held in `picture` runs on: its stream eases in from where
    /// it held, and plays on as if it never held.
    pub fn resume_game(&mut self, picture: PictureId) {
        if self.games.held.remove(&picture)
            && let Some(panned) = self.sources.get(&SoundSource::Game(picture))
        {
            panned.tell(&flag("resume"));
        }
    }

    /// The game in `picture` stops: its stream is hushed and ended, and
    /// let go once its channels are out.
    pub fn close_game(&mut self, picture: PictureId) {
        self.games.opening.remove(&picture);
        self.games.held.remove(&picture);
        if let Some(panned) = self.sources.remove(&SoundSource::Game(picture)) {
            self.end(panned);
        }
    }

    /// Ends a game's stream: its processor stops once the tail is out, and
    /// its nodes are let go after.
    fn end(&mut self, panned: Panned) {
        panned.tell(&flag("end"));
        match &self.context {
            Some(context) => {
                let gone_s = context.current_time() + STREAM_TAIL_S;
                self.games.ending.push((panned, gone_s));
            }
            None => panned.disconnect(),
        }
    }

    /// The nodes the page made since the last frame: each into the graph
    /// on the games' bus, with what its game played meanwhile, or ended at
    /// once when its game stopped since.
    fn take_made(&mut self) {
        // No game made or ending: a frame pays nothing here.
        if self.games.made.borrow().is_empty() && self.games.ending.is_empty() {
            return;
        }
        let made = core::mem::take(&mut *self.games.made.borrow_mut());
        for Made {
            picture,
            open,
            node,
        } in made
        {
            let wanted = self.games.opening.get(&picture).map(|(at, _)| *at) == Some(open);
            let waiting = if wanted {
                self.games
                    .opening
                    .remove(&picture)
                    .map(|(_, waiting)| waiting)
            } else {
                None
            };
            let source = SoundSource::Game(picture);
            let Some(((context, mix), node)) =
                self.context.as_ref().zip(self.mix.as_ref()).zip(node)
            else {
                continue;
            };
            let panned = match Panned::new(context, Origin::Stream(node), mix.of(source)) {
                Ok(panned) => panned,
                Err(error) => {
                    log::warn!("{source:?}: not heard: {error:?}");
                    continue;
                }
            };
            // Its game stopped while the node was on its way.
            let Some(waiting) = waiting else {
                self.end(panned);
                continue;
            };
            for played in &waiting {
                panned.send(played);
            }
            // Its game held while the node was on its way.
            if self.games.held.contains(&picture) {
                panned.tell(&flag("hold"));
            }
            self.sources.insert(source, panned);
        }
        if let Some(context) = self
            .context
            .as_ref()
            .filter(|_| !self.games.ending.is_empty())
        {
            let now_s = context.current_time();
            self.games.ending.retain(|(panned, gone_s)| {
                let ending = *gone_s > now_s;
                if !ending {
                    panned.disconnect();
                }
                ending
            });
        }
    }

    /// Every source put where it stands this frame, relative to the eye, as
    /// loud as its part says, and let out once the browser allows. A source
    /// the engine does not name this frame is silent: a video silenced, or
    /// out of its reach, still plays for the eye.
    pub fn tick(&mut self, camera: &Camera, sounds: &[Sound]) {
        self.take_made();
        let running = self.running();
        self.runs = running;
        let eye_from_world = camera.rotation.inverse();
        for panned in self.sources.values_mut() {
            if panned.volume != 0.0 {
                panned.volume = 0.0;
                panned.gain.gain().set_value(0.0);
            }
        }
        for sound in sounds {
            let Some(panned) = self.sources.get_mut(&sound.source) else {
                continue;
            };
            if panned.volume != sound.volume {
                panned.volume = sound.volume;
                panned.gain.gain().set_value(sound.volume);
            }
            let at = eye_from_world * (sound.position - camera.position);
            if at != panned.at {
                panned.at = at;
                let [x, y, z] = at.as_vec3().to_array();
                panned.panner.position_x().set_value(x);
                panned.panner.position_y().set_value(y);
                panned.panner.position_z().set_value(z);
            }
            panned
                .panner
                .set_max_distance(sound.reach_m.max(FULL_M + 0.01));
        }
        if running {
            for panned in self.sources.values() {
                if let Origin::Element(element, _) = &panned.origin
                    && element.muted()
                {
                    element.set_muted(false);
                }
            }
        }
    }

    /// The page hides or shows. Hidden, every sustained voice eases to
    /// silence where it is; shown, the next frame eases each voice back to
    /// what it is named with. A game's stream holds with its game.
    pub fn hidden(&mut self, hidden: bool) {
        self.sustains.hidden = hidden;
        let Some(context) = &self.context else {
            return;
        };
        let now_s = context.current_time();
        for playing in self.sustains.playing.values_mut() {
            // Unknown, so the next frame shown sets it again.
            playing.level = f32::NAN;
            if hidden {
                let _ = playing
                    .gain
                    .gain()
                    .set_target_at_time(0.0, now_s, SUSTAIN_EASE_S);
            }
        }
    }

    /// Keeps each voice sustained this frame playing where it is, eased to
    /// its level and tone; one not named fades out and is let go.
    pub fn sustain(&mut self, camera: &Camera, sustains: &[Sustain]) {
        let held = &self.sustains;
        let idle = sustains.is_empty() && held.playing.is_empty() && held.failed.is_empty();
        if idle || !self.unlocked || held.hidden {
            return;
        }
        let (Some(context), Some(mix)) = (&self.context, &self.mix) else {
            return;
        };
        let now_s = context.current_time();
        let eye_from_world = camera.rotation.inverse();
        let frame = self.sustains.frame + 1;
        self.sustains.frame = frame;
        for sustain in sustains {
            let at = eye_from_world * (sustain.position - camera.position);
            if let Some(failed) = self.sustains.failed.get_mut(&sustain.key) {
                *failed = frame;
                continue;
            }
            if !self.sustains.playing.contains_key(&sustain.key) {
                let bus = mix.voices(sustain.group);
                match self.sustains.start(context, bus, sustain, at) {
                    Ok(started) => {
                        self.sustains.playing.insert(sustain.key, started);
                    }
                    Err(error) => {
                        log::warn!("{}: not heard: {error:?}", sustain.voice.name);
                        self.sustains.failed.insert(sustain.key, frame);
                        continue;
                    }
                }
            }
            let Some(playing) = self.sustains.playing.get_mut(&sustain.key) else {
                continue;
            };
            (playing.named, playing.gone_s) = (frame, None);
            if playing.level != sustain.level {
                playing.level = sustain.level;
                let level = sustain.level.clamp(0.0, 1.0);
                let _ = playing
                    .gain
                    .gain()
                    .set_target_at_time(level, now_s, SUSTAIN_EASE_S);
            }
            if playing.tone != sustain.tone {
                playing.tone = sustain.tone;
                let hz = tone_hz(sustain.tone);
                let _ = playing
                    .filter
                    .frequency()
                    .set_target_at_time(hz, now_s, SUSTAIN_EASE_S);
            }
            if playing.at != at {
                playing.at = at;
                let [x, y, z] = at.as_vec3().to_array();
                playing.panner.position_x().set_value(x);
                playing.panner.position_y().set_value(y);
                playing.panner.position_z().set_value(z);
            }
        }
        self.sustains.failed.retain(|_, named| *named == frame);
        self.sustains.playing.retain(|_, playing| {
            if playing.named == frame {
                return true;
            }
            match playing.gone_s {
                None => {
                    playing.gone_s = Some(now_s + SUSTAIN_GONE_S);
                    playing.level = 0.0;
                    let _ = playing
                        .gain
                        .gain()
                        .set_target_at_time(0.0, now_s, SUSTAIN_EASE_S);
                    true
                }
                Some(gone_s) if gone_s > now_s => true,
                Some(_) => {
                    playing.stop();
                    false
                }
            }
        });
    }

    /// Plays the voices struck this frame where each was struck, the
    /// nearest first, while fewer than [`STRUCK_AT_ONCE`] play. What is
    /// struck before the first gesture is let go: a strike is now or never.
    /// One struck by that gesture, the first whistle, starts as the context
    /// resumes.
    pub fn strike(&mut self, camera: &Camera, strikes: Vec<Strike>) {
        let idle = strikes.is_empty() && self.voices.playing.is_empty();
        if idle || !self.unlocked {
            return;
        }
        let (Some(context), Some(mix)) = (&self.context, &self.mix) else {
            return;
        };
        self.voices.ended(context.current_time());
        let room = STRUCK_AT_ONCE.saturating_sub(self.voices.playing.len());
        let eye_from_world = camera.rotation.inverse();
        for strike in strikes.iter().take(room) {
            let at = eye_from_world * (strike.position - camera.position);
            let bus = mix.voices(strike.group);
            if let Err(error) = self.voices.play(context, bus, strike, at) {
                log::warn!("{}: not heard: {error:?}", strike.voice.name);
            }
        }
    }
}
