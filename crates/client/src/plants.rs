//! The plants gaia grows, as the ground carries them, painted:
//! every shape of a world's flora meshed once, its wood as tubes along its
//! branches and its leaves as cards about the round masses of its crown,
//! each card lit as its mass is: near, every leaf card; mid range, a few
//! big ones a mass; far, one card a mass.
//! Every patch that holds plants stands them on its own ground, as its
//! grass is, in the column of the cell grid each is rooted in: the finest
//! the plants the full detail of the ground grows, the coarser ones those
//! their own ground takes. So a plant is drawn by whichever patch draws the
//! ground under it, and moves with that ground when the patch splits. A bed
//! of flowers is the grass's: it takes only the colour and form of its
//! petals here.

use core::f64::consts::{PI, TAU};

use gaia::flora::{Bed, Flora, Plant, Puff, Seedling, Segment, Shape, Tissue};
use glam::{DVec3, Vec3};
use scene::{FoliageInstance, FoliageMesh, FoliageShape, FoliageVertex, PATCH_GRID, TerrainVertex};
use topology::{BLOCK_M, Column, Sector, SurfacePoint};
use worldgen::{Generator, Sample};

use crate::terrain::Cover;

/// The forms a shape is painted in, nearest first.
#[derive(Clone, Copy, PartialEq)]
enum Form {
    /// Every leaf card.
    Near,
    /// A few big leaf cards a mass, lit and cut as the near ones.
    Mid,
    /// A card a mass.
    Far,
}

/// Sides of a tube of wood, by form.
const SIDES: [u32; 3] = [7, 5, 4];
/// Wood thinner than this, blocks, is hidden in the leaves and never drawn,
/// by form.
const TWIG: [f64; 3] = [0.12, 0.22, 0.35];
/// The fewest and the most leaf cards a mass is painted with, near and mid.
const CARDS: [[usize; 2]; 2] = [[10, 48], [3, 9]];
/// How much of a mass's skin its cards cover, counted over: enough that no
/// gap shows through.
const COVER: f64 = 1.0;
/// A leaf card's half side against the radius of its mass, and its bounds,
/// blocks, near and mid.
const CARD: [f64; 2] = [0.42, 0.72];
const CARD_BLOCKS: [[f64; 2]; 2] = [[0.9, 8.0], [1.6, 12.0]];
/// How far out in their mass cards stand, shares of its radii: a low shell,
/// so the mass is full from every side.
const SHELL: [f64; 2] = [0.5, 0.85];
/// A card standing for a whole mass is as wide as it, and this much over.
const MASS_CARD: f64 = 1.05;
/// How deep into a mass, a share of its radii, a bough is seen before its
/// leaves hide it.
const HIDES: f64 = 0.8;

/// Wood, as painted (sRGB): a warm mid brown, and a birch's warm grey. The
/// shade turns either violet; moss is muted and low. None is lighter than a
/// blade of grass, or a trunk glows in the sun.
const WOOD: [f64; 3] = [112.0, 78.0, 52.0];
const BARK: [f64; 3] = [104.0, 92.0, 80.0];
const MOSS: [f64; 3] = [92.0, 104.0, 62.0];
/// The share of the sky a crowned tree's wood sees: it stands in its
/// crown's shade, and the moon, which casts no shadow, reaches it as little.
const UNDER: f64 = 0.85;
/// Metres up a trunk the moss gives out over.
const MOSS_M: [f64; 2] = [0.3, 1.8];

/// One shape of a world's flora, by the index plants name it by, painted
/// in each of its forms. A mesh is in blocks around its root: the origin
/// is where the ground meets the middle of the root's column and `z` is up,
/// so a quarter turn about the origin is the quarter turn a plant takes. A
/// shape whose wood hides in its leaves is undergrowth: it stops no body.
pub fn shape(flora: &Flora, index: u32) -> FoliageShape {
    let shape = &flora.shapes()[index as usize];
    FoliageShape {
        forms: [Form::Near, Form::Mid, Form::Far].map(|form| painted(shape, form)),
        undergrowth: !shape.wood_shows(),
    }
}

/// A bed's flower, as a tuft that blooms carries it: the linear albedo of
/// its petals, and their form, `1` to `4`.
pub fn flower(bed: &Bed) -> [u8; 4] {
    let [r, g, b] = painted_tone(bed.petals).map(|c| ((c / 255.0).powf(2.2) * 255.0).round() as u8);
    let form = match bed.petals {
        Tissue::Daisy => 2,
        Tissue::Bellflower => 3,
        Tissue::Campion => 4,
        _ => 1,
    };
    [r, g, b, form]
}

/// The middle tone a tissue is painted in, sRGB: leaves the grass's family,
/// petals bright against it.
fn painted_tone(tissue: Tissue) -> [f64; 3] {
    // Leaves are the meadow's green, leaning: dappled a step deeper, needles
    // a step cooler.
    let grass = |lean: [f64; 3]| {
        [0, 1, 2].map(|i| {
            let blade = f64::from(super::grass::BLADE[0][i] + super::grass::BLADE[1][i]) / 2.0;
            (blade * lean[i]).powf(1.0 / 2.2) * 255.0
        })
    };
    match tissue {
        Tissue::Wood => WOOD,
        Tissue::Bark => BARK,
        Tissue::Leaf => grass([1.0, 1.0, 1.0]),
        Tissue::Shade => grass([0.90, 0.93, 0.98]),
        Tissue::Needle => grass([0.80, 0.88, 1.15]),
        Tissue::Buttercup => [255.0, 212.0, 36.0],
        Tissue::Daisy => [250.0, 247.0, 236.0],
        Tissue::Bellflower => [146.0, 108.0, 232.0],
        Tissue::Campion => [244.0, 116.0, 168.0],
    }
}

/// One shape painted in one form: its leaf cards, its wood and a card a
/// mass, the first two drawn and the last two cast; far, no leaf cards and
/// the thicker wood, and all of it drawn.
fn painted(shape: &Shape, form: Form) -> FoliageMesh {
    let tier = form as usize;
    let mut mesh = FoliageMesh::default();
    let crown = shape.crown();
    // The middle of the crown, and how far it reaches, for the normal the
    // whole of it lends its leaves.
    let middle = crown
        .iter()
        .map(|puff| DVec3::from(puff.middle))
        .sum::<DVec3>()
        / crown.len().max(1) as f64;
    let reach = crown
        .iter()
        .map(|puff| (DVec3::from(puff.middle) - middle).length() + puff.radii[0])
        .fold(1.0, f64::max);
    let heights = crown.iter().map(|puff| puff.middle[2]);
    let low = heights.clone().fold(f64::MAX, f64::min);
    let high = heights.fold(f64::MIN, f64::max);
    let [wood, leaves] = shape.tissues();
    let tone = painted_tone(leaves);
    if form != Form::Far {
        for (index, puff) in crown.iter().enumerate() {
            let lit = lift(puff.middle[2], low, high);
            leaf_cards(&mut mesh, puff, index, middle, reach, lit, tone, tier);
        }
    }
    let leafy = mesh.indices.len() as u32;
    if shape.wood_shows() {
        let open = if crown.is_empty() { 1.0 } else { UNDER };
        for segment in seen_wood(shape, crown) {
            if segment.radius >= TWIG[tier] {
                tube(&mut mesh, shape, segment, SIDES[tier], wood, open);
            }
        }
    }
    let wooded = mesh.indices.len() as u32;
    for puff in crown {
        let normal = crown_normal(DVec3::from(puff.middle), middle, reach);
        let lit = lift(puff.middle[2], low, high);
        let half = puff.radii[0].max(puff.radii[1]) * MASS_CARD;
        card(&mut mesh, puff.middle, puff, normal, tone, lit, half, 2.0);
    }
    let all = mesh.indices.len() as u32;
    mesh.drawn = 0..if form == Form::Far { all } else { wooded };
    mesh.casts = leafy..all;
    mesh
}

/// The wood that shows: each bough from the ground up until it goes into
/// the crown. What grows on inside it the leaves hide, and drawn it would
/// poke through them as stubs with nothing to hold them.
fn seen_wood<'a>(shape: &'a Shape, crown: &[Puff]) -> Vec<&'a Segment> {
    let inside = |p: [f64; 3]| {
        crown.iter().any(|puff| {
            let [across, up] = puff.radii.map(|r| r * HIDES);
            let d = [0, 1, 2].map(|i| p[i] - puff.middle[i]);
            (d[0] * d[0] + d[1] * d[1]) / (across * across) + d[2] * d[2] / (up * up) < 1.0
        })
    };
    let mut seen: Vec<&Segment> = Vec::new();
    for (k, segment) in shape.segments().iter().enumerate() {
        let held = k == 0 || seen.iter().any(|bough| bough.to == segment.from);
        if held && !inside(segment.from) {
            seen.push(segment);
        }
    }
    seen
}

/// How open to the sky a mass is by its height in its crown: the lowest in
/// the shade of the rest.
fn lift(z: f64, low: f64, high: f64) -> f64 {
    let t = if high > low {
        (z - low) / (high - low)
    } else {
        1.0
    };
    0.85 + 0.15 * t
}

/// The normal the whole crown lends a point of it: out from its middle, the
/// crown as one round body.
fn crown_normal(at: DVec3, middle: DVec3, reach: f64) -> DVec3 {
    let out = (at - middle) / reach;
    (out + DVec3::Z * 0.25).normalize()
}

/// A mass of leaves as cards spread over a shell inside it, as evenly as a
/// spiral lays them, each lit as the mass and the crown about it are: the
/// shader takes a point of a card as that point of the mass. `tier` is the
/// form's: mid range, fewer and bigger cards.
#[allow(clippy::too_many_arguments)]
fn leaf_cards(
    mesh: &mut FoliageMesh,
    puff: &Puff,
    index: usize,
    middle: DVec3,
    reach: f64,
    lit: f64,
    tone: [f64; 3],
    tier: usize,
) {
    let [across, up] = puff.radii;
    let [least, most] = CARD_BLOCKS[tier];
    let half = (across * CARD[tier]).clamp(least, most);
    let skin = 4.0 * PI * (across * across + 2.0 * across * up) / 3.0;
    let [fewest, most] = CARDS[tier];
    let count = ((skin * COVER / (PI * half * half)).round() as usize).clamp(fewest, most);
    let mut roll = Roll(0x9e37_79b9_7f4a_7c15 ^ index as u64);
    let golden = PI * (3.0 - 5.0f64.sqrt());
    for k in 0..count {
        let z = 1.0 - 2.0 * (k as f64 + 0.5) / count as f64;
        let ring = (1.0 - z * z).sqrt();
        let angle = golden * k as f64 + roll.unit() * 0.6;
        let out = DVec3::new(ring * angle.cos(), ring * angle.sin(), z);
        let shell = SHELL[0] + (SHELL[1] - SHELL[0]) * roll.unit();
        let at = DVec3::from(puff.middle) + out * DVec3::new(across, across, up) * shell;
        let shade = 1.0 + 0.04 * (roll.unit() - 0.5);
        card(
            mesh,
            at.to_array(),
            puff,
            crown_normal(at, middle, reach),
            tone.map(|c| c * shade),
            lit,
            half,
            1.0,
        );
    }
}

/// A card of a mass: four corners at its middle, `half` blocks a side from
/// it, turned to the eye where it is drawn.
#[allow(clippy::too_many_arguments)]
fn card(
    mesh: &mut FoliageMesh,
    at: [f64; 3],
    puff: &Puff,
    normal: DVec3,
    tone: [f64; 3],
    open: f64,
    half: f64,
    what: f32,
) {
    let base = mesh.vertices.len() as u32;
    let position = rooted_at(at);
    let mass = rooted_at(puff.middle);
    let color = painted_color(tone, open);
    for corner in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]] {
        mesh.vertices.push(FoliageVertex {
            position,
            normal: normal.as_vec3().to_array(),
            color,
            card: [corner[0], corner[1], half as f32, what],
            mass,
        });
    }
    mesh.indices
        .extend([0, 1, 2, 0, 2, 3].map(|offset| base + offset));
}

/// A branch as a tube, tapering to the branch that grows on from its tip,
/// its sides facing out, `open` to that share of the sky.
fn tube(
    mesh: &mut FoliageMesh,
    shape: &Shape,
    segment: &Segment,
    sides: u32,
    wood: Tissue,
    open: f64,
) {
    let on = shape
        .segments()
        .iter()
        .filter(|next| next.from == segment.to)
        .map(|next| next.radius)
        .fold(None, |most: Option<f64>, r| {
            Some(most.map_or(r, |m| m.max(r)))
        });
    let radii = [segment.radius, on.unwrap_or(segment.radius * 0.6)];
    let ends = [segment.from, segment.to].map(DVec3::from);
    let axis = (ends[1] - ends[0]).normalize_or(DVec3::Z);
    let helper = if axis.z.abs() < 0.9 {
        DVec3::Z
    } else {
        DVec3::X
    };
    let u = axis.cross(helper).normalize();
    let w = axis.cross(u);
    let base = mesh.vertices.len() as u32;
    for (end, radius) in ends.into_iter().zip(radii) {
        for k in 0..sides {
            let angle = TAU * f64::from(k) / f64::from(sides);
            let out = u * angle.cos() + w * angle.sin();
            let at = end + out * radius;
            let tone = bark(wood, at.z * BLOCK_M, out);
            let position = rooted_at(at.to_array());
            mesh.vertices.push(FoliageVertex {
                position,
                normal: out.as_vec3().to_array(),
                color: painted_color(tone, open),
                card: [0.0; 4],
                mass: position,
            });
        }
    }
    for k in 0..sides {
        let next = (k + 1) % sides;
        let [a, b, c, d] = [k, next, sides + next, sides + k].map(|i| base + i);
        mesh.indices.extend([a, b, c, a, c, d]);
    }
}

/// Wood's colour at a height, metres over the ground: mossy low on the side
/// turned to `v`.
fn bark(wood: Tissue, height_m: f64, out: DVec3) -> [f64; 3] {
    let base = painted_tone(wood);
    let moss = (1.0 - smoothstep(MOSS_M[0], MOSS_M[1], height_m)) * (0.6 + 0.4 * out.y);
    [0, 1, 2].map(|i| base[i] + (MOSS[i] - base[i]) * moss.clamp(0.0, 1.0) * 0.85)
}

fn smoothstep(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A point of a shape, where the root block spans `0..1`, about the mesh's
/// origin, the middle of the root block's bottom: drawn where the ground
/// meets the root's column.
fn rooted_at(at: [f64; 3]) -> [f32; 3] {
    [at[0] - 0.5, at[1] - 0.5, at[2]].map(|x| x as f32)
}

fn painted_color(tone: [f64; 3], open: f64) -> [u8; 4] {
    let [r, g, b] = tone.map(|c| c.round().clamp(0.0, 255.0) as u8);
    [r, g, b, (open * 255.0).round().clamp(0.0, 255.0) as u8]
}

/// A stream of rolls in `0..1` from a seed, for the cards of one mass: the
/// same every time it is painted.
struct Roll(u64);

impl Roll {
    fn unit(&mut self) -> f64 {
        // SplitMix64.
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Where a plant stands, as what is built asks of it: the column its root
/// is in, and the heights it spans, metres over the datum.
#[derive(Clone, Copy, Debug)]
pub struct Rooting {
    root: Column,
    span_m: [f64; 2],
}

impl Rooting {
    /// A plant rooted in `root`'s column spanning `span_m`, metres over
    /// the datum: a tree of a test's own.
    #[cfg(test)]
    pub(crate) fn standing(root: Column, span_m: [f64; 2]) -> Rooting {
        Rooting { root, span_m }
    }

    pub fn of(flora: &Flora, plant: Plant) -> Rooting {
        let shape = &flora.shapes()[usize::from(plant.shape)];
        let top = shape.low()[2] + shape.size()[2] as i32;
        let bottom = i32::from(plant.root.h);
        Rooting {
            root: plant.root.column,
            span_m: [bottom, bottom + top].map(|h| f64::from(h) * BLOCK_M),
        }
    }

    /// The column the plant's root is in: which plant it is.
    pub fn root(self) -> Column {
        self.root
    }

    /// Whether the plant grows: no cell stands in its root's column
    /// anywhere over its height, for nothing grows up through a floor, as
    /// no tuft of grass does. The patch that draws a plant and the trunks
    /// that stop a body ask this one question, so a plant is drawn where it
    /// stops a body and stops one where it is drawn, in a volume or out.
    pub fn grows(self, cover: Cover) -> bool {
        !cover(self.root.sector, column_middle(self.root), self.span_m)
    }
}

/// The plants rooted in a finest patch of a sector, `side` blocks from
/// `corner`, as the full detail of the ground grows them: the plants whose
/// trunks stop a body. Each stands on the patch's own ground, as its grass
/// does, in the column of blocks its root is in, where it [grows](Rooting::grows).
/// `origin` and `vertices` are the patch's, as `grass::build` takes them.
#[allow(clippy::too_many_arguments)]
pub fn rooted(
    generator: &Generator,
    flora: &Flora,
    sector: Sector,
    corner: [u32; 2],
    side: u32,
    origin: DVec3,
    vertices: &[TerrainVertex],
    cover: Cover,
) -> Vec<FoliageInstance> {
    let plants = flora.plants(generator, sector, corner, side).into_iter();
    plants
        .filter_map(|plant| {
            if !Rooting::of(flora, plant).grows(cover) {
                return None;
            }
            let column = column_middle(plant.root.column);
            let (root, ..) = on_ground(vertices, corner, side, column);
            let radius_m = (origin + root.as_dvec3()).length();
            let lean = leaning(flora, plant, vertices, corner, side, column);
            Some(instance(generator, plant, radius_m, root.as_dvec3(), lean))
        })
        .collect()
}

/// The plants rooted in a coarse patch of a sector, `side` blocks from
/// `corner`, stood on the patch's own ground as [`rooted`] stands them, so
/// the patch asks nothing more of the generator than where they were sown;
/// drawn afar. `samples` are the patch's, as `grass::build` takes them, and
/// the rest as [`rooted`] takes it.
#[allow(clippy::too_many_arguments)]
pub fn standing(
    generator: &Generator,
    flora: &Flora,
    sector: Sector,
    corner: [u32; 2],
    side: u32,
    origin: DVec3,
    vertices: &[TerrainVertex],
    samples: &[(DVec3, Sample)],
    cover: Cover,
) -> Vec<FoliageInstance> {
    let g = PATCH_GRID as usize;
    let datum_m = generator.sphere().radius_m();
    let seedlings = flora.seedlings(generator, sector, corner, side).into_iter();
    seedlings
        .filter_map(|seedling: Seedling| {
            let column = column_middle(seedling.root);
            let (root, normal, [x, y]) = on_ground(vertices, corner, side, column);
            let nearest = |t: f32| (t.round() as usize).min(g);
            let ground = samples[(nearest(y) + 1) * (g + 3) + nearest(x) + 1].1;
            let up = (origin + root.as_dvec3()).normalize().as_vec3();
            let radius_m = (origin + root.as_dvec3()).length();
            let upright = f64::from(normal.dot(up));
            if !seedling.takes(ground.material, radius_m - datum_m, upright) {
                return None;
            }
            let plant = seedling.rooted(radius_m - datum_m);
            if !Rooting::of(flora, plant).grows(cover) {
                return None;
            }
            let lean = leaning(flora, plant, vertices, corner, side, column);
            Some(instance(generator, plant, radius_m, root.as_dvec3(), lean))
        })
        .collect()
}

/// Where a patch, `side` blocks from `corner`, draws its ground at the
/// middle of a column, `u` and `v` in blocks: the point from the patch's
/// origin, the ground's normal there, and where that is on the patch's
/// grid, in samples.
fn on_ground(
    vertices: &[TerrainVertex],
    corner: [u32; 2],
    side: u32,
    column: [f64; 2],
) -> (Vec3, Vec3, [f32; 2]) {
    let g = PATCH_GRID as f64;
    let at = [0, 1].map(|i| ((column[i] - f64::from(corner[i])) / f64::from(side) * g) as f32);
    let (root, normal) = super::grass::surface(vertices, at[0], at[1]);
    (root, normal, at)
}

/// The ground a plant that stands on its leaves leans with: the normal of
/// the plane through the ground a patch draws across the plant's reach
/// along `u` and along `v`, within the patch, about its root's column, in
/// blocks. `None` for a plant its trunk stands upright.
fn leaning(
    flora: &Flora,
    plant: Plant,
    vertices: &[TerrainVertex],
    corner: [u32; 2],
    side: u32,
    column: [f64; 2],
) -> Option<DVec3> {
    let shape = &flora.shapes()[usize::from(plant.shape)];
    if shape.wood_shows() {
        return None;
    }
    let reach = f64::from(shape.reach().max(1));
    let at = |axis: usize, step: f64| {
        let mut point = column;
        let edges = [f64::from(corner[axis]), f64::from(corner[axis] + side)];
        point[axis] = (point[axis] + step).clamp(edges[0], edges[1]);
        on_ground(vertices, corner, side, point).0.as_dvec3()
    };
    let across = at(0, reach) - at(0, -reach);
    let along = at(1, reach) - at(1, -reach);
    Some(across.cross(along).normalize())
}

/// The middle of a column of a sector, in blocks.
fn column_middle(column: Column) -> [f64; 2] {
    let middle = SurfacePoint::center_of(column);
    [middle.u, middle.v]
}

/// A plant drawn as its shape, the ground at the middle of its root's
/// column `at` from a patch's origin and `radius_m` from the body's centre:
/// a block of the grid where its root stands along `u`, `v` and up, in
/// metres, turned as the plant is. A plant spans a few metres, over which
/// the grid bends by under a millimetre: one map places all of it. A plant
/// that stands on its leaves leans with the ground whose normal is `lean`:
/// its blocks keep their height over that ground rather than over the
/// level, so on a slope its masses sink alike, none buried uphill and none
/// lifted downhill.
fn instance(
    generator: &Generator,
    plant: Plant,
    radius_m: f64,
    at: DVec3,
    lean: Option<DVec3>,
) -> FoliageInstance {
    let tangents = generator
        .sphere()
        .blocks()
        .tangents(SurfacePoint::center_of(plant.root.column));
    let up = DVec3::from(tangents.up);
    let along = |axis: DVec3| match lean {
        Some(normal) => axis - up * (normal.dot(axis) / normal.dot(up)),
        None => axis,
    };
    let block = [
        along(DVec3::from(tangents.du) * radius_m),
        along(DVec3::from(tangents.dv) * radius_m),
        up * BLOCK_M,
    ];
    let axes = [[1, 0, 0], [0, 1, 0], [0, 0, 1]].map(|unit| {
        let turned = plant.turned(unit);
        let axis: DVec3 = (0..3).map(|i| block[i] * f64::from(turned[i])).sum();
        axis.as_vec3().to_array()
    });
    let root = plant.root.column;
    let mut roll =
        Roll((root.sector.index() as u64) << 40 | u64::from(root.u) << 20 | u64::from(root.v));
    FoliageInstance {
        axes,
        origin: at.as_vec3().to_array(),
        shape: u32::from(plant.shape),
        seed: roll.unit() as f32,
    }
}
