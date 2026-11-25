//! The command/event seam between the core and any UI.
//!
//! Svelte panels and the native UI send [`Command`]s and render [`Event`]s.
//! Both travel as JSON tagged by `type`, so the same seam serves a WASM
//! boundary, a native panel and tests.
//!
//! A plugin's commands and events ride the same seam under its name: a
//! `type` with a dot in it, `chat.say`, is a plugin's, and the host hands it
//! over (`crate::plugin`).

use scene::Effects;
use serde::{Deserialize, Serialize};
use worldgen::Recipe;

use crate::figure::Play;
use crate::marker::{At as MarkerAt, Said as MarkerSaid};
use crate::offer::{Act, Prompt, Reached};
use crate::pets::Pet;

/// How the body moves: on foot, in flight, or in a way of moving a plugin
/// gave, by its full name. Said as one string: `"walk"`, `"fly"`,
/// `"<plugin>.<way>"`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Mode {
    Walk,
    Fly,
    /// A way of moving a plugin gave: the plugin's name, a dot, the way's.
    Way(String),
}

impl Mode {
    /// The string it is said as.
    pub fn name(&self) -> &str {
        match self {
            Mode::Walk => "walk",
            Mode::Fly => "fly",
            Mode::Way(name) => name,
        }
    }
}

impl Serialize for Mode {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.name())
    }
}

impl<'de> Deserialize<'de> for Mode {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Mode, D::Error> {
        let name = String::deserialize(deserializer)?;
        match name.as_str() {
            "walk" => Ok(Mode::Walk),
            "fly" => Ok(Mode::Fly),
            _ if name
                .split_once('.')
                .is_some_and(|(plugin, way)| !plugin.is_empty() && !way.is_empty()) =>
            {
                Ok(Mode::Way(name))
            }
            _ => Err(serde::de::Error::custom(format!(
                "`{name}` is no mode: walk, fly, or a way as `plugin.way`"
            ))),
        }
    }
}

/// What a session may do, as the world said it in its welcome. The order is
/// the order of trust: a builder builds, an admin builds and founds the world.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Anonymous,
    SignedIn,
    Builder,
    Admin,
}

impl Level {
    /// A level this client has yet to learn reads as the least.
    pub(crate) fn from_wire(level: i32) -> Level {
        match protocol::Level::try_from(level) {
            Ok(protocol::Level::SignedIn) => Level::SignedIn,
            Ok(protocol::Level::Builder) => Level::Builder,
            Ok(protocol::Level::Admin) => Level::Admin,
            Ok(protocol::Level::Anonymous) | Err(_) => Level::Anonymous,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    /// Regenerate the world from this recipe and respawn.
    SetRecipe {
        recipe: Recipe,
    },
    SetMode {
        mode: Mode,
    },
    /// Wear the avatar at this asset reference: a path under the asset root
    /// (`avatars/Kyle.vrm`) or an absolute URL (a user's own avatar).
    SetAvatar {
        path: String,
    },
    /// Wear any avatar the manifest offers. Nothing while the account wears
    /// its own, as the world's welcome says it.
    RandomAvatar,
    /// Wear the next avatar on offer, after the current one. Nothing while
    /// the account wears its own.
    NextAvatar,
    /// How the picture is made. Fields left out take their default.
    SetEffects {
        effects: Effects,
    },
    /// Stand where a pose says, and look the way it says: `"4-K7M42Q"`, or
    /// `"m4-K7M42Q@40,180,-5"` for a body on the moon, forty blocks up,
    /// facing south and looking a little down. A code shorter than full
    /// precision names a box, and the middle of it is where you land.
    ///
    /// An arrival, at once: how a shared address opens where it says, how
    /// a body is born at home, how `--at` aims a headless render. A front
    /// end veils it until [`Event::Settled`]. A place chosen inside the
    /// world is reached by [`Command::Travel`].
    GoTo {
        place: String,
    },
    /// Travel to a place as [`Command::GoTo`] reads it, through the passage
    /// a portal takes a body by, with no portal: the picture drawn into the
    /// body, held while the ground at the place comes, and bursting out of
    /// it there. How a place chosen inside the world is reached, a line of
    /// the chart, a place shared in chat, by a person's click and by an
    /// agent alike; a front end never veils it. Walking and flying are the
    /// other ways about a world.
    Travel {
        place: String,
    },
    /// What to be called: said in Hello and, while online, changed at once.
    /// A signed in person's account name wins on the server; a front end
    /// keeps the account in step itself. Empty is a name too.
    SetName {
        name: String,
    },
    /// The plugin with the pointer puts it down, with what it had in hand:
    /// what a front end asks when a panel of its own opens, so one thing
    /// is open at a time.
    PutDown,
    /// A visitor's home and favourites in the chart, as its front end kept
    /// them from [`Event::Choices`]: each a landmark by its name in the
    /// world. An account's are the world's, and these are not.
    SetChoices {
        home: Option<u64>,
        #[serde(default)]
        favourites: Vec<u64>,
    },
    /// Plays one of the figure's own clips over what the body moves by, for
    /// a while: what a plugin asks with `Host::play`, the same request.
    Play(Play),
    /// Acts on what the prompt offers, as its key does: `{"type":"act"}`
    /// taps it. With `plugin` and `id`, on that offer, whichever way the
    /// body faces, while it is within reach:
    /// `{"type":"act","plugin":"ride","id":0}`. With `hold`, holds it as
    /// the key held does, the ring said closing by [`Event::Ring`] until
    /// it is done or the offer leaves.
    Act {
        #[serde(default)]
        plugin: Option<String>,
        #[serde(default)]
        id: Option<u64>,
        #[serde(default)]
        hold: bool,
    },
    /// Asks for every offer within reach: answered by [`Event::Offers`].
    Offers,
    /// Holds controls down while they are lent, as keys held do, until the
    /// next `press` or the controls go back:
    /// `{"type":"press","controls":["right","jump"]}`, by the names
    /// [`crate::Pad::named`] takes; `[]` lets every one go. Refused while
    /// nobody holds them.
    Press {
        controls: Vec<String>,
    },
    /// Takes the controls back from whoever holds them, as Escape does.
    /// Refused while nobody holds them.
    Leave,
    /// Asks for every animal drawn near: answered by [`Event::Animals`].
    Animals,
    /// Asks for every marker given and not hidden, wherever it is: answered by
    /// [`Event::Markers`]. A marker's offers are commands, sent as they are.
    Markers,
    /// The kinds of marker the person hides, in place of the last:
    /// `{"type":"hide_markers","kinds":["favourite"]}`. What a front end
    /// keeps for them and hands over on each start; none hides nothing.
    HideMarkers {
        #[serde(default)]
        kinds: Vec<String>,
    },
    /// The body sounds one of the core's voices, by its name: `whistle`.
    /// It sounds at the body's mouth, for this client and every peer
    /// within the voice's reach. The whistle calls the body's pet within
    /// [`gaia::fauna::ANSWERS_M`]: [`Event::Whistled`] says whether and how
    /// it comes.
    Sound {
        voice: String,
    },
    /// A visitor's pet as its front end kept it from [`Event::Pet`], handed
    /// to the world on every welcome and held there for the session; none
    /// lets it go. An account's pet is the world's, and this is not asked.
    SetPet {
        #[serde(default)]
        pet: Option<Pet>,
    },
    /// Names the body's pet: trimmed, no control character, at most
    /// [`gaia::fauna::NAME_BYTES`].
    NamePet {
        name: String,
    },
    /// Reads the screen of the game on the cabinet `part`, or, with none,
    /// of the game played: answered by [`Event::Screen`] once the runner
    /// reads it, with its pixels too when `indices`. Refused for a cabinet
    /// whose game does not play here.
    Screen {
        #[serde(default)]
        part: Option<u64>,
        #[serde(default)]
        indices: bool,
    },
}

#[derive(Clone, PartialEq, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// Sent once, first. `generator_version` is what new worlds should use.
    Ready {
        generator_version: u32,
    },
    RecipeChanged {
        recipe: Recipe,
    },
    ModeChanged {
        mode: Mode,
    },
    /// The avatar this body is in, or on its way into, whenever either
    /// changes. `path` is the choice: a UI persists it as the person's,
    /// unless it is the account's own.
    /// While `worn` is false the file is on its way and the body stays as
    /// it was: a UI shows the wait. `None` is the box figure, nothing wanted.
    AvatarChanged {
        path: Option<String>,
        worn: bool,
    },
    /// The effects now in force, clamped to what is sane. Sent once at the
    /// start too. A UI persists this as the person's choice.
    EffectsChanged {
        effects: Effects,
    },
    /// Sent about twice a second.
    Stats {
        fps: f32,
        altitude_m: f64,
        speed_mps: f64,
        /// Where the body is, as a person says it: `"4-K7M42Q"`, or
        /// `"m4-K7M42Q@40"` on the moon, off the ground. The sector is a
        /// character of it, so it is not sent twice.
        place: String,
        /// The same, plus the way of looking: what a link carries, so someone
        /// who opens it stands where you stood seeing what you saw. A front
        /// end puts this in the address bar and hands it back as `GoTo`
        /// without ever taking it apart.
        pose: String,
        /// Degrees clockwise from north, `0..360`. `None` at a pole, where a
        /// bearing is not a thing that exists. The letters are the front
        /// end's: N and S are English, and a user visible string belongs to
        /// whoever holds the locale.
        bearing_deg: Option<f64>,
        /// The pace the body goes at in a way that goes in paces, by the
        /// name its plugin gave it: a word of that plugin's.
        pace: Option<&'static str>,
        /// Whether a whistle calls someone now: the body has a pet, is not
        /// on it, and stands on the planet with life on.
        calls: bool,
        /// Whether the body flies, on its own or aloft on what it rides,
        /// a pegasus: what rises and sinks it.
        flying: bool,
    },
    /// The link to the world server, whenever it changes. Sent once at the
    /// start too, offline.
    Session {
        status: SessionStatus,
        /// This client's session in the world, while online.
        session: Option<u32>,
        /// What the world said this session may do. `None` until a world
        /// has spoken, and then the last word stands through a dropped link.
        level: Option<Level>,
    },
    /// Who else is here, whenever that changes. Empty when offline.
    Peers {
        peers: Vec<PeerInfo>,
    },
    /// Where every head in view is on the screen, this client's own
    /// included: sent every frame while there is one, and once empty after
    /// the last. A front end hangs a nametag or a balloon there.
    Anchors {
        anchors: Vec<Anchor>,
    },
    /// The body's own pet, whenever the world says it changed: tamed (with
    /// an empty name until named), named, left, called; none once it has
    /// none. A visitor's front end keeps it and hands it back with
    /// [`Command::SetPet`].
    Pet {
        pet: Option<Pet>,
    },
    /// The body whistled with a pet: how far it was along the ground,
    /// metres, whether it answers, and how long it is out of sight before
    /// it comes into it, seconds. Within earshot, [`gaia::fauna::HEARS_M`],
    /// it runs in at once, 0; past it, under two seconds; past
    /// [`gaia::fauna::ANSWERS_M`] it does not answer, and does not come.
    Whistled {
        away_m: f64,
        coming_s: f64,
        answered: bool,
    },
    /// Where every named part in view, a landmark, is on the screen: sent
    /// every frame while there is one, and once empty after the last. A
    /// front end hangs the name there.
    Labels {
        labels: Vec<Label>,
    },
    /// Every cabinet in view whose game waits for the player who left it,
    /// on the screen, and how long it still waits: sent every frame while
    /// there is one, and once empty after the last. A front end marks the
    /// game there as waiting.
    Waiting {
        waiting: Vec<Waiting>,
    },
    /// The plugins that are on in the world this client is in, whenever
    /// that changes: what a front end mounts. Empty when offline, where no
    /// world has spoken.
    Statement {
        plugins: Vec<PluginOn>,
    },
    /// The streamer has nothing left to build for this view: what is on
    /// screen is the world at the detail it is meant to have. Sent each time
    /// that becomes true again, after a new recipe or a walk, and after
    /// every leap, even to ground already drawn.
    Settled,
    /// The paints a cell can take, in order, as `#rrggbb`. Sent once, first
    /// after [`Event::Ready`].
    Palette {
        colors: Vec<String>,
    },
    /// Which plugin has the pointer now, a tool in hand, or none. One has it
    /// at a time: taking it rests the one that had it.
    PointerChanged {
        plugin: Option<String>,
    },
    /// Which plugin the controls are lent to now, or none: while one holds
    /// them, the keys press its controls and the eye is its, and a front
    /// end shows the keys that press them and the one that leaves.
    ControlsChanged {
        plugin: Option<String>,
    },
    /// Where the body is born: the place of this person's home, as the
    /// first chart after the welcome says it, `None` with no home. Said
    /// once a link; a front end that asked no place goes there with
    /// [`Command::GoTo`].
    Home {
        place: Option<String>,
    },
    /// A visitor's home and favourites, whenever they change: what its
    /// front end keeps and hands back with [`Command::SetChoices`]. An
    /// account's are kept by the world, and never said here.
    Choices {
        home: Option<u64>,
        favourites: Vec<u64>,
    },
    /// What the prompt offers, whenever that changes: the thing in front of
    /// the body or the body's own, and what a tap and a hold on it do, in
    /// the words of the plugin that offers it. `None` when nothing is
    /// offered.
    Prompt {
        prompt: Option<Prompt>,
    },
    /// Where the prompt's point is on the screen, every frame while the
    /// prompt is a point in view: fractions of the viewport from the top
    /// left, and how far it is, as [`Anchor`]; and where the body stands on
    /// the screen while it is seen, `[left, top, right, bottom]`, which the
    /// prompt keeps clear of.
    PromptAt {
        x: f32,
        y: f32,
        distance_m: f32,
        body: Option<[f32; 4]>,
    },
    /// How far the ring around the prompt's key has closed, 0 to 1, every
    /// frame a hold is held past a tap; 1 as it closes, then `None` once it
    /// is done, let go or broken.
    Ring {
        closed: Option<f32>,
    },
    /// An offer was acted on, by the key, a finger or an agent: a front
    /// end answers it on the card that shows the offer, a tap's sweep of
    /// the ring or a hold's pulse.
    Acted {
        plugin: &'static str,
        id: u64,
        act: Act,
    },
    /// Every offer within reach, whichever way the body faces: the answer
    /// to [`Command::Offers`].
    Offers {
        offers: Vec<Reached>,
    },
    /// Every animal drawn near, nearest first: the answer to
    /// [`Command::Animals`].
    Animals {
        animals: Vec<Animal>,
    },
    /// What the givers point out and the person does not hide, whenever
    /// that changes: whose, what it marks, its kind, label, glyph and note,
    /// and the choices a press offers. A front end draws each where
    /// [`Event::MarkersAt`] says. While a plugin holds the controls, its
    /// markers alone.
    MarkersChanged {
        markers: Vec<MarkerSaid>,
    },
    /// Where the markers shown are for the eye, the nearest few: sent each
    /// frame one moved past a pixel, and once empty after the last.
    MarkersAt {
        at: Vec<MarkerAt>,
    },
    /// Every marker given and not hidden, nearest first, with where its
    /// thing is: the answer to [`Command::Markers`].
    Markers {
        markers: Vec<Pointed>,
    },
    /// A game's screen as it was read, the answer to [`Command::Screen`]:
    /// `hash`, FNV-1a of its palette and its pixels, sixteen hex digits,
    /// changes when what it shows changes; `palette` is its colours,
    /// `#rrggbb`; with `indices` asked, its pixels row by row, one digit
    /// of base 32 each (`0` to `9`, `a` to `v`), an index into the palette.
    Screen {
        part: u64,
        width: u32,
        height: u32,
        hash: String,
        palette: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        indices: Option<String>,
    },
    /// A command was refused. `message` is for logs, not for end users.
    Rejected {
        message: String,
    },
    /// An event of a plugin, as the plugin wrote it: an object whose `type`
    /// is the plugin's name and the event's, `chat.said`.
    #[serde(untagged)]
    Plugin(serde_json::Value),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Offline,
    /// The socket is open and Welcome is awaited.
    Connecting,
    Online,
}

/// Someone else in the world, as a front end lists them. The name is the
/// person's own or empty; a visitor is signed out.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct PeerInfo {
    pub session: u32,
    pub name: String,
    pub visitor: bool,
}

/// An animal drawn near, as an agent reads it: `number` is what a pet is
/// told by, `place` what `go_to` takes, `away_m` along the ground from the
/// body's feet, and `bearing_deg` the way it faces, clockwise from north
/// (none at a pole).
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Animal {
    pub number: u64,
    pub kind: &'static str,
    pub place: String,
    pub away_m: f64,
    pub bearing_deg: Option<f64>,
    pub ridden: bool,
    /// Whose pet it is, none while it is its herd's.
    pub pet: Option<PetOf>,
}

/// A marker as an agent reads it: the marker, `place` what `go_to` takes,
/// `away_m` along the ground from the body's feet, and `bearing_deg` the
/// way to it from the body, clockwise from north (none at a pole, or
/// where the body stands). One whose thing is nowhere to be found is
/// left out.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Pointed {
    #[serde(flatten)]
    pub marker: MarkerSaid,
    pub place: String,
    pub away_m: f64,
    pub bearing_deg: Option<f64>,
}

/// An animal's keeping, as [`Animal`] says it: its name, empty until named,
/// and whether it is this body's own.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct PetOf {
    pub name: String,
    pub mine: bool,
}

/// A plugin that is on in this world, as the world's statement says it.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
pub struct PluginOn {
    pub name: String,
    pub version: u32,
}

/// A head on the screen: fractions of the viewport from the top left, so
/// no front end needs to know the canvas size, and how far the head is.
#[derive(Clone, Copy, PartialEq, Debug, Serialize)]
pub struct Anchor {
    pub session: u32,
    pub x: f32,
    pub y: f32,
    pub distance_m: f32,
}

/// A named part on the screen: its name here, the point over it as
/// fractions of the viewport from the top left, and how far it is.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Label {
    pub id: u64,
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub distance_m: f32,
}

/// A cabinet whose game waits for its player, on the screen: the cabinet,
/// the middle of its screen as fractions of the viewport from the top
/// left, how far it is, and how much of its window remains, a share from 1
/// as it was left to 0 as it ends, and in seconds.
#[derive(Clone, PartialEq, Debug, Serialize)]
pub struct Waiting {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub distance_m: f32,
    pub remains: f32,
    pub remains_s: f32,
}

impl Command {
    pub fn from_json(json: &str) -> Result<Command, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Event {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("events are plain data")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_parse_from_the_documented_json() {
        let json = r#"{"type":"set_recipe","recipe":{"seed":"00000000deadbeef","generator_version":3,"params":{}}}"#;
        assert_eq!(
            Command::from_json(json).unwrap(),
            Command::SetRecipe {
                recipe: Recipe::new(0xdead_beef)
            }
        );
        assert_eq!(
            Command::from_json(r#"{"type":"set_mode","mode":"fly"}"#).unwrap(),
            Command::SetMode { mode: Mode::Fly }
        );
        assert_eq!(
            Command::from_json(r#"{"type":"set_mode","mode":"test.way"}"#).unwrap(),
            Command::SetMode {
                mode: Mode::Way("test.way".into())
            }
        );
        assert!(Command::from_json(r#"{"type":"set_mode","mode":"swim"}"#).is_err());
        assert!(Command::from_json(r#"{"type":"set_mode","mode":"test."}"#).is_err());
    }

    #[test]
    fn effects_left_out_take_their_default() {
        let command =
            Command::from_json(r#"{"type":"set_effects","effects":{"clouds":false}}"#).unwrap();
        assert_eq!(
            command,
            Command::SetEffects {
                effects: Effects {
                    clouds: false,
                    ..Effects::default()
                }
            }
        );
    }

    #[test]
    fn events_serialize_tagged() {
        assert_eq!(
            Event::ModeChanged { mode: Mode::Walk }.to_json(),
            r#"{"type":"mode_changed","mode":"walk"}"#
        );
        assert_eq!(
            Event::ModeChanged {
                mode: Mode::Way("test.way".into())
            }
            .to_json(),
            r#"{"type":"mode_changed","mode":"test.way"}"#
        );
        assert_eq!(
            Event::Statement {
                plugins: vec![PluginOn {
                    name: "chat".into(),
                    version: 1
                }]
            }
            .to_json(),
            r#"{"type":"statement","plugins":[{"name":"chat","version":1}]}"#
        );
    }

    #[test]
    fn a_plugins_event_goes_out_as_the_plugin_wrote_it() {
        let event = Event::Plugin(serde_json::json!({"type": "chat.said", "text": "hi"}));
        assert_eq!(event.to_json(), r#"{"text":"hi","type":"chat.said"}"#);
    }
}
