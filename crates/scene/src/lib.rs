//! The scene: what a client hands to a renderer, as plain data.
//!
//! The client decides *what* is visible; a renderer decides *how* it looks.
//! An agent is a client without a renderer, so nothing here touches the GPU,
//! and the renderer never learns about recipes or addresses.
//!
//! World space is `f64`. Anything that reaches the GPU as `f32` is relative to
//! a nearby `f64` origin (camera-relative rendering).

use bytemuck::{Pod, Zeroable};
use std::ops::Range;
use std::sync::Arc;

use glam::{DAffine3, DQuat, DVec3, Mat4, Vec3};

/// Quads per side of a terrain patch. Every patch shares one index buffer.
pub const PATCH_GRID: u32 = 32;
/// Vertices of the patch surface: a `(PATCH_GRID + 1)^2` grid, row major,
/// `u` along the row.
pub const PATCH_SURFACE_VERTICES: u32 = (PATCH_GRID + 1) * (PATCH_GRID + 1);
/// Surface plus the skirt: one lowered copy of every edge vertex, four runs of
/// `PATCH_GRID + 1` in the order `v = 0`, `v = max`, `u = 0`, `u = max`.
pub const PATCH_VERTICES: u32 = PATCH_SURFACE_VERTICES + 4 * (PATCH_GRID + 1);

/// Stable identity of a terrain patch while it is in the scene.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct PatchId(pub u64);

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct TerrainVertex {
    /// Metres from [`TerrainMesh::origin`].
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// `rgb`: sRGB albedo of the ground cover, before the renderer exposes
    /// rock on slopes. `a`: gloss, 0 for dry ground up to 255 for a mirror.
    pub color: [u8; 4],
    /// How much of the ground the crowns of foliage cover here, 0 to 1: the
    /// share of it that wears the wood's colour past the foliage drawn
    /// ([`Effects::foliage_reach_m`]).
    pub canopy: f32,
}

/// The sea over a patch: the same grid at sea level.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct WaterVertex {
    /// Metres from [`TerrainMesh::origin`].
    pub position: [f32; 3],
    /// Water over the ground here, metres. Negative over dry land.
    pub depth_m: f32,
}

/// One terrain patch around an `f64` origin, relative to the centre of its
/// body (see [`PatchDraw`]).
///
/// A patch of the heightfield is [`PATCH_VERTICES`] vertices in the grid
/// [`patch_indices`] describes, and brings no indices of its own. A patch of
/// the volume layer has as many vertices as its surface needs and brings the
/// indices that join them.
#[derive(Clone, Debug)]
pub struct TerrainMesh {
    pub origin: DVec3,
    pub vertices: Vec<TerrainVertex>,
    /// Empty for a heightfield patch, which uses the shared grid.
    pub indices: Vec<u32>,
    /// Present when any of the patch is under the sea. Same layout and index
    /// buffer as `vertices`.
    pub water: Option<Vec<WaterVertex>>,
    /// Cosmetic tufts rooted in this patch, sorted by falling `reach_m`: the
    /// tufts seen from a distance are a prefix. Empty on far patches.
    pub grass: Vec<GrassInstance>,
    /// The foliage rooted in this patch.
    pub foliage: Vec<FoliageInstance>,
    /// Too coarse to carry the foliage rooted in it: none stands on it, so
    /// its ground wears the wood's colour by its canopy at any distance.
    pub bare: bool,
}

/// Foliage standing on a terrain patch: one of a world's shapes
/// ([`TerrainChange::Paint`]) placed in the patch's local space.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct FoliageInstance {
    /// Where each axis of the mesh's space goes, metres a unit: a block of
    /// the cell grid where it stands, sheared as the grid is there.
    pub axes: [[f32; 3]; 3],
    /// Where the mesh's origin stands, metres from the patch's origin.
    pub origin: [f32; 3],
    /// Which of the foliage shapes it is.
    pub shape: u32,
    /// Its own share of its kind's look, in `0..1`: how its tone leans and
    /// when the wind takes it, so no two of a shape are copies.
    pub seed: f32,
}

impl FoliageInstance {
    /// How far from its origin it is painted, metres, grown to the share
    /// `grown` of its size, for a shape painted to `reach` blocks
    /// ([`FoliageShape::reach`]): its mesh as its axes stretch it, and the
    /// wind's sway.
    pub fn reach_m(&self, reach: f32, grown: f32) -> f32 {
        reach * self.stretch() * grown + FOLIAGE_SWAY_M
    }

    /// The most its axes stretch a length of its mesh: a bound on the norm
    /// of the map they make, which a lean shears, by the largest sum of a
    /// row of its Gram matrix.
    fn stretch(&self) -> f32 {
        let axes = self.axes.map(Vec3::from);
        let row = |i: usize| {
            axes.iter()
                .map(|axis| axes[i].dot(*axis).abs())
                .sum::<f32>()
        };
        row(0).max(row(1)).max(row(2)).sqrt()
    }
}

/// How a renderer may draw foliage off where its mesh puts it, and no
/// further: a card's corner flutters along up by this share of its half
/// side, and the wind sways any point by up to [`FOLIAGE_SWAY_M`].
pub const FOLIAGE_FLUTTER: f32 = 0.05;
/// The farthest the wind sways a point of foliage, metres.
pub const FOLIAGE_SWAY_M: f32 = 0.44;

/// How many forms a foliage shape is painted in.
pub const FOLIAGE_FORMS: usize = 3;

/// The last share of the foliage's reach an instance grows over, from
/// nothing at the reach to its size: the band the ground wears the wood's
/// colour over. Nearer than it, a wood is its foliage.
pub const FOLIAGE_GROWING: f32 = 0.15;

/// One shape of a world's foliage as it is painted, in each of its forms,
/// nearest first: every leaf a card; each mass a few cards; a card a mass. A
/// renderer draws each instance in the form its distance calls for, and
/// crosses from one form to the next over a band, a share of the pixels at
/// a time, so none ever changes at once.
#[derive(Clone, Debug, Default)]
pub struct FoliageShape {
    pub forms: [FoliageMesh; FOLIAGE_FORMS],
    /// Whether it is undergrowth, which stops no body: a viewer may thin it
    /// out ([`Effects::undergrowth`]).
    pub undergrowth: bool,
}

impl FoliageShape {
    /// How far from its root any form of it is painted, blocks.
    pub fn reach(&self) -> f32 {
        self.forms
            .iter()
            .map(FoliageMesh::reach)
            .fold(0.0, f32::max)
    }
}

impl FoliageMesh {
    /// How far from its root it is painted, blocks: its wood, and each card
    /// to its corners turned any way, fluttering ([`FOLIAGE_FLUTTER`]).
    pub fn reach(&self) -> f32 {
        let corner = core::f32::consts::SQRT_2 + FOLIAGE_FLUTTER;
        self.vertices
            .iter()
            .map(|vertex| Vec3::from(vertex.position).length() + vertex.card[2] * corner)
            .fold(0.0, f32::max)
    }
}

/// One form of a foliage shape: its wood as tubes and its leaves as cards
/// about the round masses of its crown, in blocks about its root (as
/// [`FoliageInstance`] places it). Its wood and one card a mass cast the sun's
/// shadow.
#[derive(Clone, Debug, Default)]
pub struct FoliageMesh {
    pub vertices: Vec<FoliageVertex>,
    pub indices: Vec<u32>,
    /// The run of `indices` drawn in view, and the run that casts.
    pub drawn: Range<u32>,
    pub casts: Range<u32>,
}

/// A corner of a foliage mesh. A card is four corners at its middle, turned
/// to the eye where it is drawn.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct FoliageVertex {
    /// Where it stands, blocks about the shape's root; a card's middle.
    pub position: [f32; 3],
    /// Wood's own normal; a card's is its crown's, out from the crown's
    /// middle, as if the whole crown were one round body.
    pub normal: [f32; 3],
    /// rgb: its colour, as painted (sRGB); a leaf's middle tone. a: how open
    /// it is to the sky.
    pub color: [u8; 4],
    /// A card's corner: `xy` in `-1..1`, `z` its half side in blocks, `0`
    /// for wood. `w` what it is: wood `0`, a leaf card `1`, a card standing
    /// for a whole mass `2`.
    pub card: [f32; 4],
    /// The middle of the mass a card belongs to, blocks about the root: a
    /// point of a card is lit as that point of the round mass, so light
    /// falls on the mass and never on its cards. Wood's own position.
    pub mass: [f32; 3],
}

/// The shore as the ground is painted, metres over the sea: sand up to
/// [`SAND_TOP_M`], its line wandering by up to [`SAND_WANDER_M`] either way
/// up close and softened over [`SAND_EDGE_M`], so no sand is painted over
/// [`SAND_HIGHEST_M`]. Whatever grows on the ground starts over it.
pub const SAND_TOP_M: f32 = 2.0;
pub const SAND_WANDER_M: f32 = 1.05;
pub const SAND_EDGE_M: f32 = 0.35;
pub const SAND_HIGHEST_M: f32 = SAND_TOP_M + SAND_WANDER_M + SAND_EDGE_M;

/// The shell of weather clouds fill, metres over the sea, from its base to
/// its top: where a renderer draws them, and where what flies on wings
/// meets them. The atmosphere ends over it.
pub const CLOUD_BASE_M: f32 = 1100.0;
pub const CLOUD_TOP_M: f32 = 3000.0;

/// How far the densest grass tier is seen, metres. Each sparser tier doubles
/// it. Bounded by the terrain LOD: a patch must split before the tiers it
/// lacks come into reach (`client::grass`).
pub const GRASS_TIER_0_REACH_M: f32 = 20.0;

/// One cosmetic tuft, in the same local space as its terrain patch.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct GrassInstance {
    pub root: [f32; 3],
    pub height: f32,
    /// Of the ground at the root. A tuft is lit as the slope it grows on;
    /// it grows along the planet's up, which a renderer knows.
    pub normal: [f32; 3],
    /// The tuft has faded into the ground this far from the eye.
    pub reach_m: f32,
    /// Linear albedo, and rotation about radial up.
    pub color: [f32; 3],
    pub angle: f32,
    /// A tuft that blooms is a flower: the linear albedo of its petals, and
    /// their form, `1` to `4`; `0` for a tuft that is only grass.
    pub bloom: [u8; 4],
}

/// The curve that turns the scene's light into what a screen can show.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneMap {
    /// Filmic, contrasty, saturated: Narkowicz's fit of the ACES curve.
    #[default]
    Aces,
    /// Softer shoulder; bright colours fade to white instead of skewing hue.
    Agx,
    /// Khronos PBR neutral: colours as authored, only highlights compressed.
    Neutral,
    /// Reinhard on luminance: plain and flat.
    Reinhard,
    /// No curve: light above white clips.
    Linear,
}

impl ToneMap {
    /// The order the output shader knows them in.
    pub const ALL: [ToneMap; 5] = [
        ToneMap::Aces,
        ToneMap::Agx,
        ToneMap::Neutral,
        ToneMap::Reinhard,
        ToneMap::Linear,
    ];

    pub fn index(self) -> u32 {
        ToneMap::ALL
            .iter()
            .position(|&each| each == self)
            .expect("listed") as u32
    }
}

/// How the picture is made, not what the world is: what a person may turn
/// down on a slow machine or tune to taste. Travels in every [`Frame`]; a UI
/// sets it through the client's seam. Absent fields take their default.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Effects {
    pub shadows: bool,
    /// Whether the sun's shadows fade from one step of the sun to the next,
    /// the two near cascades drawn from both ([`SunSteps`]); off, every
    /// cascade is drawn once and the shadows move a step at a time.
    pub shadows_fade: bool,
    pub grass: bool,
    pub clouds: bool,
    /// What share of the view's pixels the world is drawn at, each way: 1
    /// is every pixel, a retina's included; 0.5 a quarter of the work, the
    /// picture sampled up to the view by the last stage.
    pub resolution: f32,
    /// How much of the sky the weather may fill, 0 to 1.
    pub cloud_cover: f32,
    /// How thick a cloud is to light: 1 is a storm deck, the default a soft
    /// fair weather cloud.
    pub cloud_density: f32,
    /// How fast the weather turns, metres a second at the equator.
    pub wind_m_s: f32,
    /// How fast clouds reshape where they stand, as a factor of the usual.
    pub cloud_change: f32,
    /// What the scene's light is multiplied by before the tone map.
    pub exposure: f32,
    /// How strongly bright light spills over its neighbours. 0 turns it off.
    pub bloom: f32,
    /// How bright light must be to spill, once exposed: 1 is a sunlit wall.
    pub bloom_threshold: f32,
    /// How thick the air is, as a factor of the usual: distance fades sooner.
    pub haze: f32,
    /// How far a swimmer sees under the sea, as a factor of plain sea water.
    pub water_clarity: f32,
    /// How far from the body a picture that moves plays, metres: past it,
    /// it is a still, its poster, and makes no sound.
    pub video_play_m: f32,
    /// How far from the body a picture is drawn, metres.
    pub picture_reach_m: f32,
    /// How far from the body a model, a cube, a landmark or a portal is
    /// drawn, metres for each block of its longest side: a giant sculpture
    /// is seen from far, a cube from near.
    pub model_reach_m: f32,
    /// How far from the eye foliage is drawn whole, every leaf a card,
    /// metres; past it, each mass a few cards.
    pub foliage_near_m: f32,
    /// How far foliage's masses are a few cards each, metres; past it, a
    /// card a mass.
    pub foliage_mid_m: f32,
    /// How far foliage is drawn at all, metres; past it a wood is the
    /// colour its ground wears. Never short of the wood that stops a body.
    pub foliage_reach_m: f32,
    /// The share of the undergrowth drawn, 0 to 1: the foliage that stops
    /// no body ([`FoliageShape::undergrowth`]).
    pub undergrowth: f32,
    pub tone_map: ToneMap,
}

/// The high picture: every effect on, at the distances a desktop draws.
impl Default for Effects {
    fn default() -> Self {
        Self {
            shadows: true,
            shadows_fade: true,
            grass: true,
            clouds: true,
            resolution: 1.0,
            cloud_cover: 0.5,
            cloud_density: 0.1,
            wind_m_s: 14.0,
            cloud_change: 1.0,
            exposure: 1.0,
            bloom: 0.7,
            bloom_threshold: 0.3,
            haze: 0.8,
            water_clarity: 3.0,
            video_play_m: 128.0,
            picture_reach_m: 256.0,
            model_reach_m: 128.0,
            foliage_near_m: 90.0,
            foliage_mid_m: 260.0,
            foliage_reach_m: 600.0,
            undergrowth: 1.0,
            tone_map: ToneMap::Aces,
        }
    }
}

impl Effects {
    /// The light picture, for a screen that pays several times more for a
    /// frame: a phone, a Pi. Shadows, grass, clouds and bloom off, the world
    /// drawn at half the pixels each way, pictures and models drawn half as
    /// far, videos playing half as far, all foliage a card a mass and no
    /// undergrowth. A UI offers it beside [`Effects::medium`] and
    /// [`Effects::default`] as the presets, and starts a touch screen on it.
    pub fn light() -> Self {
        let high = Self::default();
        Self {
            shadows: false,
            grass: false,
            clouds: false,
            resolution: 0.5,
            bloom: 0.0,
            video_play_m: high.video_play_m / 2.0,
            picture_reach_m: high.picture_reach_m / 2.0,
            model_reach_m: high.model_reach_m / 2.0,
            foliage_near_m: 0.0,
            foliage_mid_m: 0.0,
            foliage_reach_m: 250.0,
            undergrowth: 0.0,
            ..high
        }
    }

    /// Between the two: every effect on, foliage drawn whole half as far,
    /// half the undergrowth, and the shadows moving a step of the sun at a
    /// time: three passes of the casters a frame for five.
    pub fn medium() -> Self {
        let high = Self::default();
        Self {
            shadows_fade: false,
            foliage_near_m: 50.0,
            foliage_mid_m: 160.0,
            foliage_reach_m: 400.0,
            undergrowth: 0.5,
            ..high
        }
    }
}

/// Visual interaction only; does not participate in collision.
#[derive(Clone, Copy, Debug)]
pub struct InteractionCapsule {
    pub start: DVec3,
    pub end: DVec3,
    pub radius_m: f32,
}

/// The index buffer shared by every patch. Counter clockwise seen from
/// outside the planet; skirt walls face away from the patch.
pub fn patch_indices() -> Vec<u16> {
    let g = PATCH_GRID;
    let at = |i: u32, j: u32| (j * (g + 1) + i) as u16;
    let mut out = Vec::with_capacity((6 * g * g + 4 * 6 * g) as usize);
    for j in 0..g {
        for i in 0..g {
            out.extend([at(i, j), at(i + 1, j), at(i, j + 1)]);
            out.extend([at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    // (edge vertex k, skirt run, flipped winding) for the four edges.
    let skirt = |run: u32, k: u32| (PATCH_SURFACE_VERTICES + run * (g + 1) + k) as u16;
    for k in 0..g {
        let edges = [
            (at(k, 0), at(k + 1, 0), 0, false),
            (at(k, g), at(k + 1, g), 1, true),
            (at(0, k), at(0, k + 1), 2, true),
            (at(g, k), at(g, k + 1), 3, false),
        ];
        for (a, b, run, flipped) in edges {
            let (a_low, b_low) = (skirt(run, k), skirt(run, k + 1));
            if flipped {
                out.extend([b, b_low, a_low, b, a_low, a]);
            } else {
                out.extend([a, a_low, b_low, a, b_low, b]);
            }
        }
    }
    out
}

/// A patch to draw, and where its body is this frame. Bodies move (the moon
/// orbits), patches do not: a mesh is built once around its body's centre.
#[derive(Clone, Copy, Debug)]
pub struct PatchDraw {
    pub id: PatchId,
    pub body_center: DVec3,
}

/// The coarsest ground that may cast a shadow, metres across one cell.
///
/// The furthest shadow cascade has a texel of `2 * 4000 / 1024`, about 7.8 m
/// (`render::shadow`). A triangle whose cells are wider than that spans many
/// texels at once, and its depth varies across one of them by far more than
/// any bias can lift, so the whole triangle shadows itself and goes black.
/// Ground coarser than this is lit by the sun's angle alone, which is what a
/// body seen from a distance is lit by anyway.
pub const SHADOW_CASTER_CELL_M: f64 = 4.0;

/// A change to the set of terrain patches a renderer holds.
#[derive(Clone, Debug)]
pub enum TerrainChange {
    Add(PatchId, TerrainMesh),
    Remove(PatchId),
    /// One shape of a world's foliage, by the index the foliage of the
    /// patches added after it names it by, in place of any held there: each
    /// drawn many times, and sent with the first patch that draws it.
    Paint(u32, FoliageShape),
    /// Every foliage shape held is let go: none is drawn until painted
    /// again.
    Unpaint,
}

/// A unit cube (`-0.5..=0.5`) placed in world space. Avatars are made of these
/// until real models arrive.
#[derive(Clone, Copy, Debug)]
pub struct BoxPart {
    pub transform: DAffine3,
    /// Linear RGB.
    pub color: Vec3,
    /// How much it shines white with its own light in place of its colour,
    /// 0 to 1: a body charged by its passage through a portal.
    pub shine: f32,
    /// Casts its shadow and is not drawn: the body the camera is inside of.
    pub shadow_only: bool,
    /// Lit as one layer of a thin, soft sheet whose plane lies across its
    /// own `y`, from the side the eye sees: its shade is its colour a cool
    /// step down, and the sun on its far side shows through. Feathers,
    /// cloth, a leaf.
    pub sheet: bool,
}

/// Stable identity of a volume mesh while it is in the scene: one chunk of
/// one volume, the ghost of a stroke, or a guide.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct VolumeMeshId(pub u64);

/// A volume mesh to draw, and where the centre of its body is this frame:
/// its origin is counted from there, as a patch's is (see [`PatchDraw`]).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeDraw {
    pub id: VolumeMeshId,
    pub body_center: DVec3,
}

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct VolumeVertex {
    /// Metres from [`VolumeMesh::origin`].
    pub position: [f32; 3],
    /// Of the side this vertex is a corner of: a cube is lit flat.
    pub normal: [f32; 3],
    /// `rgb`: sRGB albedo. `a`: how much the side shines with its own
    /// colour, 0 lit by what falls on it up to 255 for a light.
    pub color: [u8; 4],
    /// The corner on its side. First how much of the sky it sees, 0 to 255:
    /// ambient light only. Then where it is across the side and up it, 0 or
    /// 255 each way. Last the edge drawn around the side: 0 none, 1 black,
    /// 2 white.
    pub side: [u8; 4],
}

/// Cubes of the build layer around an `f64` origin, counted from the centre
/// of the body they stand on. Every
/// vertex is a corner the client already bent onto the body, so a renderer
/// draws it as it is.
#[derive(Clone, Debug)]
pub struct VolumeMesh {
    pub origin: DVec3,
    pub vertices: Vec<VolumeVertex>,
    pub indices: Vec<u32>,
}

/// A corner of a guide.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct GuideVertex {
    /// Metres from [`GuideMesh::origin`].
    pub position: [f32; 3],
    /// Where it is on its side, in cells: a line is drawn wherever either is
    /// whole.
    pub lattice: [f32; 2],
    /// What shows between the lines. `rgb`: sRGB. `a`: how much of it.
    pub color: [u8; 4],
    /// The lines. `rgb`: sRGB. `a`: how much of one shows where cells meet
    /// in eights, and half as much between any two cells.
    pub ink: [u8; 4],
    /// What runs along each side of a line, so it shows over a ground of
    /// the line's own colour. `rgb`: sRGB. `a`: how much of it, none at 0.
    pub rim: [u8; 4],
}

/// Sides of cells drawn see-through with a line between each cell and the
/// next: where cells are before any is laid, and what a stroke would change.
/// Bent onto the body by the client, as a [`VolumeMesh`] is.
#[derive(Clone, Debug)]
pub struct GuideMesh {
    pub origin: DVec3,
    pub vertices: Vec<GuideVertex>,
    pub indices: Vec<u32>,
}

/// A light: the sides that shine in one chunk, taken as one where their
/// middle is, a part that shines, or a light thrown in a cone. It lights
/// whatever is near, the ground and bodies as well as cells, and casts no
/// shadow.
#[derive(Clone, Copy, Debug)]
pub struct Lamp {
    pub position: DVec3,
    /// Linear RGB: what it gives a side that faces it from a metre away.
    pub color: Vec3,
    /// How far its light goes, metres.
    pub reach_m: f32,
    /// How wide it is, metres: nearer than that its light fades gently, as
    /// light from a patch does, not a point's.
    pub size_m: f32,
    /// The cone it is thrown in; `None` lights all round.
    pub beam: Option<Beam>,
}

/// A cone a lamp throws its light in: along `direction`, whole within
/// `spread` of it and fading to nothing at `spread + edge`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Beam {
    /// Unit vector, in the frame of its lamp's `position`: world space in
    /// a frame.
    pub direction: Vec3,
    /// The half angle lit whole, radians.
    pub spread: f32,
    /// How far past `spread` the light fades out, radians.
    pub edge: f32,
}

/// A change to the set of volume meshes a renderer holds.
#[derive(Clone, Debug)]
pub enum VolumeChange {
    Add(VolumeMeshId, VolumeMesh),
    Guide(VolumeMeshId, GuideMesh),
    Remove(VolumeMeshId),
}

/// Most joints a skinned mesh may have: what fits the smallest uniform buffer
/// every target offers (WebGL2, 16 KiB).
pub const MAX_JOINTS: usize = 128;

/// Stable identity of a model mesh while it is in the scene: the mesh of a
/// shape of part in one paint, or of one model.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct ModelId(pub u64);

/// A mesh of cubes in its own space, drawn where a part puts it: a cube is
/// a model of one voxel, a model of many. Its space is the part's unit box,
/// the anchor the middle of its bottom. Its vertices are a volume's, so it
/// is lit, edged and glazed as the cells are.
#[derive(Clone, Debug)]
pub struct ModelMesh {
    pub vertices: Vec<VolumeVertex>,
    pub indices: Vec<u32>,
}

/// A corner of a mesh drawn with pictures, in its own space.
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct TexturedVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    /// Where on its run's picture, `(0, 0)` its top left corner.
    pub uv: [f32; 2],
    /// `rgb`: sRGB albedo, times the picture's. `a`: alpha, times the
    /// picture's.
    pub color: [u8; 4],
}

/// What a run of a textured mesh is drawn with, times its corners' colour.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunImage {
    /// Nothing: the corners' colour alone.
    Plain,
    /// One of the mesh's own pictures, by its index in
    /// [`TexturedMesh::images`].
    Own(usize),
    /// The picture its draw names ([`ModelDraw::picture`]): one mesh
    /// dressed in each draw's own, a picture the pictures keep.
    Drawn,
}

/// A run of a textured mesh's indices drawn with one picture.
#[derive(Clone, Debug)]
pub struct TexturedRun {
    pub indices: Range<u32>,
    pub image: RunImage,
    /// The alpha under which a pixel is not drawn: 0 draws every one.
    pub cutoff: f32,
    /// How much it shines with its own colour, as a part does: 0 lit by
    /// what falls on it, 1 its own colour as a screen shows it, more a
    /// lamp.
    pub shine: f32,
}

/// A mesh drawn with pictures in its own space, as a model of cubes is
/// placed: a 3D model, its anchor the middle of the bottom of its box. It
/// is lit and casts as the cubes do, with no edge and no glass.
#[derive(Clone, Debug)]
pub struct TexturedMesh {
    pub vertices: Vec<TexturedVertex>,
    /// Three a triangle, wound counter clockwise seen from its front.
    pub indices: Vec<u32>,
    pub runs: Vec<TexturedRun>,
    pub images: Vec<Image>,
}

/// A change to the set of model meshes a renderer holds.
#[derive(Clone, Debug)]
pub enum ModelChange {
    Add(ModelId, ModelMesh),
    /// A mesh drawn with pictures, under the same ids as the cubes.
    Textured(ModelId, TexturedMesh),
    Remove(ModelId),
}

/// Which pass a model is drawn in.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ModelPass {
    /// Opaque, with everything solid: it casts a shadow and hides what is
    /// behind it.
    Solid,
    /// Glass: over what is solid and blended, tested against depth.
    Glass,
    /// Over everything, whatever stands before it, cast by nothing: a mark
    /// for a hand to see, the handle of a gizmo.
    Over,
}

/// One model placed in the world this frame.
#[derive(Clone, Copy, Debug)]
pub struct ModelDraw {
    pub mesh: ModelId,
    /// World from model space.
    pub transform: DAffine3,
    pub pass: ModelPass,
    /// The picture its [`RunImage::Drawn`] runs show, one the pictures
    /// hold ([`PictureChange`]); with none, or one not held, they show
    /// their corners' colour.
    pub picture: Option<PictureId>,
}

/// Stable identity of a skinned mesh while it is in the scene.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct SkinnedMeshId(pub u64);

#[derive(Clone, Copy, Debug, Pod, Zeroable)]
#[repr(C)]
pub struct SkinnedVertex {
    /// Metres, in the space of the mesh at rest.
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub joints: [u16; 4],
    /// Sum to 1.
    pub weights: [f32; 4],
}

/// An RGBA8 sRGB image, top row first.
#[derive(Clone, Debug)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// A run of indices drawn with one texture.
#[derive(Clone, Debug)]
pub struct SkinnedPrimitive {
    pub indices: Range<u32>,
    /// Index into [`SkinnedMesh::images`]. `None` draws white.
    pub image: Option<usize>,
}

/// A mesh deformed by joints: an avatar.
#[derive(Clone, Debug)]
pub struct SkinnedMesh {
    pub vertices: Vec<SkinnedVertex>,
    pub indices: Vec<u32>,
    pub primitives: Vec<SkinnedPrimitive>,
    pub images: Vec<Image>,
}

/// A change to the set of skinned meshes a renderer holds.
#[derive(Clone, Debug)]
pub enum SkinnedChange {
    Add(SkinnedMeshId, Arc<SkinnedMesh>),
    Remove(SkinnedMeshId),
}

/// One posed skinned mesh in the world.
#[derive(Clone, Debug)]
pub struct SkinnedInstance {
    pub mesh: SkinnedMeshId,
    /// World from mesh space.
    pub transform: DAffine3,
    /// Mesh space from rest space, per joint: at most [`MAX_JOINTS`].
    pub joints: Vec<Mat4>,
    /// How much it shines white with its own light in place of its colour,
    /// 0 to 1: a body charged by its passage through a portal.
    pub shine: f32,
    /// Casts its shadow and is not drawn: the body the camera is inside of.
    pub shadow_only: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: DVec3,
    /// World from camera. The camera looks down its `-Z`, `+Y` is up.
    pub rotation: DQuat,
    /// Vertical field of view, radians.
    pub fov_y: f32,
    /// Near plane, metres. There is no far plane (reversed infinite depth).
    pub near: f32,
}

/// The moon as the light sees it: a sphere that casts a shadow and lends a
/// little light. Its ground is ordinary terrain, in [`Frame::patches`].
#[derive(Clone, Copy, Debug)]
pub struct Moon {
    pub position: DVec3,
    pub radius_m: f64,
}

/// The sun the shadow maps are drawn from: never the true one, which moves
/// every frame and slides a map's grid under the ground with it, but the
/// sun in steps of its own path. A map drawn from one step holds still on
/// the ground for the step's life; the picture fades from the step begun
/// to the next as the sun goes from one to the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunSteps {
    /// Unit vector toward the sun at the step begun.
    pub from: Vec3,
    /// The same at the next step.
    pub to: Vec3,
    /// How far the sun has gone from `from` to `to`, 0 to 1.
    pub blend: f32,
}

impl SunSteps {
    /// A sun that stands still toward `sun`: both steps on it.
    pub fn held(sun: Vec3) -> Self {
        Self {
            from: sun,
            to: sun,
            blend: 0.0,
        }
    }
}

/// Everything a renderer needs for one frame, besides the patch meshes it
/// already holds.
#[derive(Clone, Debug)]
pub struct Frame {
    pub camera: Camera,
    /// Unit vector toward the sun: what lights the world and draws the sky.
    pub sun_direction: Vec3,
    /// The sun its shadows are drawn from.
    pub sun_steps: SunSteps,
    pub moon: Moon,
    /// Radius of the sea level sphere, metres. The atmosphere sits on it.
    pub planet_radius_m: f64,
    /// World clock, seconds: moves water and wind.
    pub clock_s: f64,
    /// Patches to draw this frame. All were announced by a [`TerrainChange::Add`].
    pub patches: Vec<PatchDraw>,
    /// Loaded terrain selected at shadow LOD, including offscreen casters.
    pub shadow_patches: Vec<PatchDraw>,
    pub interaction: InteractionCapsule,
    pub effects: Effects,
    pub boxes: Vec<BoxPart>,
    pub skinned: Vec<SkinnedInstance>,
    /// The cubes of every volume in reach. All were announced by a
    /// [`VolumeChange::Add`].
    pub volumes: Vec<VolumeDraw>,
    /// The cubes of glass in reach, drawn over what is solid and blended.
    /// All were announced by a [`VolumeChange::Add`].
    pub glass: Vec<VolumeDraw>,
    /// The lights among the cells in reach. A view is lit by the nearest.
    pub lamps: Vec<Lamp>,
    /// The models placed in reach: every part. All their meshes were
    /// announced by a [`ModelChange::Add`].
    pub models: Vec<ModelDraw>,
    /// The stroke a build tool would make: drawn see-through over the world,
    /// cast by nothing. Announced by a [`VolumeChange::Guide`].
    pub ghost: Option<VolumeDraw>,
    /// The guides to draw over the world. All were announced by a
    /// [`VolumeChange::Guide`].
    pub guides: Vec<VolumeDraw>,
    /// The pictures in reach, each at the level the client picked. Every
    /// level drawn was announced by a [`PictureChange::Add`].
    pub pictures: Vec<PictureDraw>,
    /// The panes of the portals in reach: a window each on the world seen
    /// through it, or dark.
    pub panes: Vec<PaneDraw>,
    /// The world seen through one portal this frame, the one its pane says
    /// is seen through: drawn before the views, from the eye past it.
    pub through: Option<Through>,
    /// The picture drawn into a portal and out of another: a body's passage
    /// through one, while it lasts.
    pub warp: Option<Warp>,
    /// A picture laid over the first view once its picture is finished,
    /// in the target's own pixels.
    pub overlay: Option<Overlay>,
}

/// A picture laid over a view after the compositor, untouched by exposure,
/// bloom or the tone map: drawn into a rectangle of the target's pixels,
/// each texel the same whole number of them wide and tall, sampled at its
/// own level, over what the view shows as far as `opacity` says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Overlay {
    pub picture: PictureId,
    /// Left, top, width and height, in the target's pixels, wholly inside it.
    pub rect: [u32; 4],
    /// 0 to 1.
    pub opacity: f32,
}

/// The picture of a view drawn into a point of the screen as a body falls
/// into a portal, then out of a point as it lands past it: the picture
/// pulled toward the point and streaked along the pull, a black hole
/// opening there with a hot rim, then the same let go.
#[derive(Clone, Copy, Debug)]
pub struct Warp {
    /// The point, fractions of the viewport from the top left.
    pub centre: [f32; 2],
    /// How far along: from -1, the fall just begun, to 0, the body gone
    /// through, to 1, landed past it and the picture whole again.
    pub phase: f32,
}

/// A portal's pane in the world: the disc inside a unit quad, `-0.5..=0.5`
/// across `x` and `y`, facing `+z`, placed by its transform, which carries
/// the pane's size in metres as a scale; seen from both sides.
#[derive(Clone, Copy, Debug)]
pub struct PaneDraw {
    /// World from the quad.
    pub transform: DAffine3,
    /// How much the world seen through the portal, [`Frame::through`],
    /// shows in it, 0 to 1: at 0 a lens, the world behind the pane bent and
    /// pulsing as through glass; at 1 a window; between, the one becoming
    /// the other.
    pub seen: f32,
}

/// The world past a portal, as a wide lens in the portal sees it: a camera
/// carried from the pane's middle to where the portal leads, high over it,
/// looking the way the view looks through the pane. The pane shows it as a
/// fisheye: its middle along the camera's line, its rim [`Through::lens`]
/// off that line.
#[derive(Clone, Debug)]
pub struct Through {
    pub camera: Camera,
    /// What carries a direction on this side of the portal to the far side.
    pub turn: DQuat,
    /// How far off its line the lens sees at the pane's rim, radians.
    pub lens: f32,
    /// The patches in its view. All were announced by a [`TerrainChange::Add`].
    pub patches: Vec<PatchDraw>,
    /// How big it is drawn, of the view's size: as big as the pane is seen,
    /// a quarter at least, whole at most.
    pub size: f32,
}

/// Stable identity of a picture while a renderer holds it: one image of one
/// level of detail, resident as a texture. The client picks which level a
/// quad shows; a renderer draws whichever it is handed.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct PictureId(pub u64);

/// A change to the set of pictures a renderer holds. An added picture gets
/// its mip chain from the renderer: an image arrives as its one full level.
#[derive(Clone, Debug)]
pub enum PictureChange {
    Add(PictureId, Arc<Image>),
    /// A picture that moves: a texture of this size the shell writes frame
    /// by frame, a card until the first frame lands.
    Live {
        id: PictureId,
        width: u32,
        height: u32,
        /// Pixels meant to be seen as squares: drawn large, each texel is
        /// a hard edged square of its colour; drawn small, its mips blend.
        sharp: bool,
    },
    Remove(PictureId),
}

/// One picture in the world: a unit quad, `-0.5..=0.5` across `x` and `y`,
/// facing `+z`, placed by its transform, which carries the quad's size in
/// metres as a scale. The texture's top row is the quad's `+y` edge.
#[derive(Clone, Copy, Debug)]
pub struct PictureDraw {
    /// The level of detail to show. `None` draws a blank card of the quad's
    /// size: what a picture is before its first byte lands.
    pub picture: Option<PictureId>,
    /// World from the quad.
    pub transform: DAffine3,
    /// How much it shines with its own colour: 0 lit by what falls on it, 1
    /// its own colour as a screen shows it, more a lamp, brighter than any
    /// sunlit white.
    pub shine: f32,
    /// Whether its back is drawn too: a free standing picture is seen from
    /// both sides, a poster on a wall from one.
    pub both_sides: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An instance is painted within its reach: every point of its mesh,
    /// its axes sheared as a lean shears them, stands no farther from its
    /// origin than its shape's reach stretched by them.
    #[test]
    fn an_instance_is_painted_within_its_reach() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut unit = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 40) as f32 / (1u64 << 24) as f32 * 2.0 - 1.0
        };
        for _ in 0..500 {
            let up = Vec3::new(unit() * 0.3, unit() * 0.3, 1.0) * 0.5;
            let instance = FoliageInstance {
                axes: [
                    Vec3::new(1.0, 0.0, 0.0) + up * unit(),
                    Vec3::new(0.0, 1.0, 0.0) + up * unit(),
                    up,
                ]
                .map(|axis| axis.to_array()),
                origin: [0.0; 3],
                shape: 0,
                seed: 0.0,
            };
            let axes = instance.axes.map(Vec3::from);
            let reach = 10.0;
            for _ in 0..50 {
                let point = Vec3::new(unit(), unit(), unit()).normalize_or_zero() * reach;
                let painted = axes[0] * point.x + axes[1] * point.y + axes[2] * point.z;
                assert!(painted.length() <= instance.reach_m(reach, 1.0) - FOLIAGE_SWAY_M + 1e-4);
            }
        }
    }
}
