//! Terrain streaming: a quadtree per sector, ground to orbit, for any body.
//!
//! Each node is a square of address space meshed as one patch of
//! `PATCH_GRID^2` quads. Near the camera nodes split, so triangles stay about
//! the same size on screen from the ground to orbit. Patches are generated a
//! few per frame, closest first; until its children are ready a node keeps
//! drawing itself, so there are never holes.
//!
//! This is the far field of the terrain layer: a heightfield of the
//! generator's ground, sea floor included, sampled with the generator's LOD
//! filter so a coarse patch is a smooth version of the fine one. Where a patch
//! dips under a sea it also carries a water surface. Nature is not editable,
//! and what is built over it leaves it as it is: only the grass and the
//! plants give way, and the patches that grow them are built again where
//! what stands over them changes (`Terrain::regrow`). The finest patches of
//! the planet and the few over them carry the plants rooted in them, each
//! stood on the patch's own ground (`plants`).
//!
//! Everything here is relative to the centre of the body: the planet sits at
//! the world origin, the moon moves, and whoever draws a patch adds the body's
//! centre for that frame.

use std::collections::{HashMap, HashSet};

use gaia::flora::{Beds, Flora};
use glam::{DVec3, Vec3};
use scene::{
    Camera, Effects, FoliageInstance, PATCH_GRID, PATCH_VERTICES, PatchId, TerrainChange,
    TerrainMesh, TerrainVertex, WaterVertex,
};
use topology::{Grid, QuadSphere, Sector, SurfacePoint};
use worldgen::{Generator, MOON_RADIUS_M, Material, Sample};

/// A node splits when an eye is closer than this many node widths.
const SPLIT_DISTANCE: f64 = 2.4;

/// An eye the terrain is streamed for: a camera, and how much coarser its
/// ground may be than the view's own: 1 for the view, 4 for a sight drawn
/// at a quarter of its height, the world seen through a portal. A node
/// splits for the nearest eye, each as far as its coarseness makes it.
#[derive(Clone, Copy, Debug)]
pub struct Viewpoint {
    pub camera: Camera,
    pub coarseness: f64,
}
/// Patches generated per frame, every body's together. Each costs about a
/// millisecond or two.
const BUILDS_PER_UPDATE: usize = 6;
/// Of those, patches built again because what stands over their grass
/// changed. Grass is cosmetic: it gives way a patch an update, first so a
/// walk never starves it, and never takes a frame from the ground.
const REGROWS_PER_UPDATE: usize = 1;
/// Patches kept before the least recently used ones are dropped.
const CACHE_PATCHES: usize = 1400;

const G: i32 = PATCH_GRID as i32;

/// Patches left to build: in all, and of those, built again for their
/// grass. A frame's is shared by every body's streamer, each taking what it
/// builds.
#[derive(Clone, Copy, Debug)]
pub struct Budget {
    builds: usize,
    regrows: usize,
}

impl Budget {
    /// What one frame builds, every body's together.
    pub fn frame() -> Budget {
        Budget {
            builds: BUILDS_PER_UPDATE,
            regrows: REGROWS_PER_UPDATE,
        }
    }
}

/// The body a terrain streamer covers. Every body is a quad sphere meshed by
/// the same quadtree; they differ in size, in depth, and in having a sea.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Planet,
    Moon,
}

/// The body a terrain streamer covers: a kind, and the quad sphere it is
/// printed at. Every body is meshed by the same quadtree; they differ in size,
/// in depth, and in having a sea.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Body {
    kind: Kind,
    sphere: QuadSphere,
}

impl Body {
    pub fn new(kind: Kind, sphere: QuadSphere) -> Body {
        Body { kind, sphere }
    }

    /// The grid a body is parametrised on. Seams, neighbours and directions
    /// are pure arithmetic and belong to the grid, not to the metres.
    fn grid(self) -> Grid {
        self.sphere.blocks()
    }

    /// Metres of this body per metre of the reference body. Heights are
    /// written once in reference metres and printed at a world's own size
    /// (50), so anything this file measures in metres scales with it.
    fn scale(self) -> f64 {
        f64::from(self.grid().side()) / f64::from(1u32 << topology::MAX_BITS)
    }

    pub fn radius_m(self) -> f64 {
        match self.kind {
            Kind::Planet => self.sphere.radius_m(),
            Kind::Moon => MOON_RADIUS_M,
        }
    }

    /// Deepest quadtree level. The planet: patches of 32 blocks, one vertex
    /// per block, which is five bits under the sector side. The moon is
    /// smaller, so it gets as fine sooner. A body with no room for a whole
    /// patch stops at its root, which is what the saturating subtraction says.
    fn max_depth(self) -> u32 {
        match self.kind {
            Kind::Planet => self.sphere.bits().saturating_sub(5),
            Kind::Moon => 9,
        }
    }

    fn has_sea(self) -> bool {
        self.kind == Kind::Planet
    }

    /// Whether plants grow on it in a world with life: the moon is bare.
    fn has_plants(self) -> bool {
        self.kind == Kind::Planet
    }

    /// No ground lies deeper than this under the datum, whatever the recipe.
    fn deepest_m(self) -> f64 {
        match self.kind {
            Kind::Planet => 2000.0 * self.scale(),
            Kind::Moon => 1500.0,
        }
    }

    pub fn sample(self, generator: &Generator, direction: [f64; 3], footprint_m: f64) -> Sample {
        match self.kind {
            Kind::Planet => generator.sample_at(direction, footprint_m),
            Kind::Moon => generator.moon_sample_at(direction, footprint_m),
        }
    }
}

/// A quadtree node: a square of `SECTOR_SIDE >> depth` cells of one body.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct Node {
    body: Body,
    sector: Sector,
    depth: u32,
    x: u32,
    y: u32,
}

impl Node {
    fn id(self) -> PatchId {
        let bits = u64::from(self.body.kind == Kind::Moon) << 63
            | (self.sector.index() as u64) << 59
            | u64::from(self.depth) << 52
            | u64::from(self.x) << 26
            | u64::from(self.y);
        PatchId(bits)
    }

    /// Share of a sector side this node spans.
    fn span(self) -> f64 {
        1.0 / f64::from(1u32 << self.depth)
    }

    /// Width on the ground, metres: a sector side is a quarter of a great circle.
    fn side_m(self) -> f64 {
        self.span() * self.body.radius_m() * core::f64::consts::FRAC_PI_2
    }

    fn children(self) -> [Node; 4] {
        let (depth, x, y) = (self.depth + 1, self.x * 2, self.y * 2);
        [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(i, j)| Node {
            body: self.body,
            sector: self.sector,
            depth,
            x: x + i,
            y: y + j,
        })
    }

    /// The unit direction of a point of the node, `s` and `t` in `0..=1`.
    /// Values outside reach past the node, and past the sector: the projection
    /// stays continuous there, which is all the mesher needs for normals at
    /// the rim. Every body uses the planet's sector grid as its parametrisation.
    fn direction(self, s: f64, t: f64) -> [f64; 3] {
        let grid = self.body.grid();
        let side = f64::from(grid.side()) * self.span();
        let point = SurfacePoint::new(
            self.sector,
            (f64::from(self.x) + s) * side,
            (f64::from(self.y) + t) * side,
        );
        grid.direction(point)
    }
}

/// What stands over the ground, as the grass asks: whether anything covers a
/// column of a sector, `u` and `v` in blocks, between two heights in metres
/// over the datum.
pub type Cover<'a> = &'a dyn Fn(Sector, [f64; 2], [f64; 2]) -> bool;

/// What the streamer remembers about a built patch.
struct Built {
    center: DVec3,
    /// Bounding sphere radius of its ground and sea around `center`, metres.
    radius_m: f64,
    /// That of all it draws, its plants as painted: what a view culls it
    /// by, metres.
    seen_m: f64,
    last_used: u64,
    /// Whether the patch is near enough the ground to grow grass or
    /// plants, covered or not.
    grows: bool,
}

pub struct Terrain {
    body: Body,
    built: HashMap<Node, Built>,
    /// Built patches whose grass has changed: each keeps being drawn until
    /// the one that replaces it is built.
    stale: HashSet<Node>,
    changes: Vec<TerrainChange>,
    frame: u64,
    casters: Vec<PatchId>,
    /// Whether the last update found nothing to build.
    settled: bool,
    /// Of each of this world's plant shapes, by index, how far from its
    /// root it is painted, blocks, once the renderer holds it.
    painted: Vec<Option<f32>>,
    /// Depths short of the finest whose patches carry their plants, for
    /// the reach they are drawn to.
    afar: u32,
}

impl Terrain {
    pub fn new(body: Body) -> Terrain {
        Terrain {
            body,
            built: HashMap::new(),
            stale: HashSet::new(),
            changes: Vec::new(),
            frame: 0,
            casters: Vec::new(),
            settled: false,
            painted: Vec::new(),
            afar: afar(body, Effects::default().foliage_reach_m),
        }
    }

    pub fn shadow_patches(&self) -> &[PatchId] {
        &self.casters
    }

    /// Forgets everything: the recipe changed.
    pub fn clear(&mut self) {
        for node in self.built.keys() {
            self.changes.push(TerrainChange::Remove(node.id()));
        }
        self.built.clear();
        self.stale.clear();
        self.casters.clear();
        self.unpaint();
    }

    /// What stands over the ground changed within `angle` radians of
    /// `middle` (unit, from the body's centre): every patch that grows grass
    /// there is built again. Nothing comes down first, so there is never a
    /// hole.
    pub fn regrow(&mut self, middle: DVec3, angle: f64) {
        let radius_m = self.body.radius_m();
        for (node, built) in self.built.iter().filter(|(_, built)| built.grows) {
            let reach = (angle + built.radius_m / radius_m).min(core::f64::consts::PI);
            if built.center.normalize().dot(middle) >= reach.cos() {
                self.stale.insert(*node);
            }
        }
    }

    /// How far plants are drawn, metres: a patch carries its plants only
    /// where some of them would be drawn grown, and the patches built again
    /// where that changes.
    pub fn set_reach(&mut self, reach_m: f32) {
        let afar = afar(self.body, reach_m);
        if afar != self.afar {
            self.afar = afar;
            self.replant();
        }
    }

    /// What grows on the body changed: every patch is built again, each
    /// plant shape painted anew with the first patch that draws it. Nothing
    /// comes down first, so there is never a hole.
    pub fn replant(&mut self) {
        self.stale.extend(self.built.keys().copied());
        self.unpaint();
    }

    /// Lets the renderer drop every plant shape it holds.
    fn unpaint(&mut self) {
        if !self.painted.is_empty() {
            self.changes.push(TerrainChange::Unpaint);
        }
        self.painted.clear();
    }

    /// Sends the renderer each shape a patch's plants name that it does not
    /// hold yet: a shape is painted when the first patch to draw it comes,
    /// never all at once. Says how far from the patch's origin its plants
    /// are painted, metres.
    fn paint(&mut self, flora: &Flora, plants: &[FoliageInstance]) -> f32 {
        if self.painted.is_empty() {
            self.painted = vec![None; flora.shapes().len()];
        }
        let mut seen_m = 0.0f32;
        for plant in plants {
            let reach = *self.painted[plant.shape as usize].get_or_insert_with(|| {
                let shape = super::plants::shape(flora, plant.shape);
                let reach = shape.reach();
                self.changes.push(TerrainChange::Paint(plant.shape, shape));
                reach
            });
            let origin_m = Vec3::from(plant.origin).length();
            seen_m = seen_m.max(origin_m + plant.reach_m(reach, 1.0));
        }
        seen_m
    }

    /// Mesh uploads and removals since the last call, in order.
    pub fn drain_changes(&mut self) -> Vec<TerrainChange> {
        core::mem::take(&mut self.changes)
    }

    /// Picks the patches to draw from each eye, the view's first, building
    /// missing ones within what is left of `budget`, and taking from it what
    /// it builds. Every camera is relative to the body's centre; `aspect` is
    /// width over height. One list an eye.
    pub fn update(
        &mut self,
        generator: &Generator,
        flora: Option<&Flora>,
        cover: Cover,
        eyes: &[Viewpoint],
        aspect: f32,
        budget: &mut Budget,
    ) -> Vec<Vec<PatchId>> {
        self.select(generator, flora, cover, eyes, aspect, budget)
    }

    /// Like [`Terrain::update`], but builds until nothing is missing. For
    /// headless renders, where the first frame is the only frame.
    pub fn settle(
        &mut self,
        generator: &Generator,
        flora: Option<&Flora>,
        cover: Cover,
        eyes: &[Viewpoint],
        aspect: f32,
    ) -> Vec<Vec<PatchId>> {
        loop {
            let mut everything = Budget {
                builds: usize::MAX,
                regrows: usize::MAX,
            };
            let patches = self.select(generator, flora, cover, eyes, aspect, &mut everything);
            if self.settled {
                return patches;
            }
        }
    }

    /// Whether the last update found nothing to build: what is drawn is the
    /// view at the detail it is meant to have. A step, a leap or a new recipe
    /// unsettle it until the streamer catches up.
    pub fn settled(&self) -> bool {
        self.settled
    }

    fn select(
        &mut self,
        generator: &Generator,
        flora: Option<&Flora>,
        cover: Cover,
        eyes: &[Viewpoint],
        aspect: f32,
        budget: &mut Budget,
    ) -> Vec<Vec<PatchId>> {
        self.frame += 1;
        self.casters.clear();
        let mut draw: Vec<Vec<PatchId>> = eyes.iter().map(|_| Vec::new()).collect();
        // How near a node is: to the nearest eye, each as far as its
        // coarseness makes it.
        let nearness = |center: DVec3| {
            eyes.iter()
                .map(|eye| (center - eye.camera.position).length() * eye.coarseness)
                .fold(f64::INFINITY, f64::min)
        };
        let mut missing: Vec<(f64, Node)> = Vec::new();
        let mut regrown: Vec<(f64, Node)> = Vec::new();

        let body = self.body;
        let mut stack: Vec<Node> = Sector::ALL
            .into_iter()
            .map(|sector| Node {
                body,
                sector,
                depth: 0,
                x: 0,
                y: 0,
            })
            .collect();
        while let Some(node) = stack.pop() {
            let Some(built) = self.built.get_mut(&node) else {
                // Only roots get here unbuilt: children are visited once ready.
                missing.push((0.0, node));
                continue;
            };
            built.last_used = self.frame;
            let (center, seen_m) = (built.center, built.seen_m);

            let distance = nearness(center);
            if self.stale.contains(&node) {
                regrown.push((distance, node));
            }
            if node.depth < body.max_depth() && distance < node.side_m() * SPLIT_DISTANCE {
                let children = node.children();
                let mut ready = true;
                for child in children {
                    match self.built.get_mut(&child) {
                        // Siblings of a drawn node must not age out under it.
                        Some(built) => built.last_used = self.frame,
                        None => {
                            ready = false;
                            missing.push((distance, child));
                        }
                    }
                }
                if ready {
                    stack.extend(children);
                    continue;
                }
            }
            for (eye, drawn) in eyes.iter().zip(&mut draw) {
                if visible(body, &eye.camera, aspect, center, seen_m) {
                    drawn.push(node.id());
                }
            }
        }

        self.settled = missing.is_empty() && regrown.is_empty();
        // Near before far for grass, then coarse before fine, near before
        // far, for ground: the picture sharpens evenly.
        regrown.sort_by(|a, b| a.0.total_cmp(&b.0));
        let regrows = budget.regrows.min(budget.builds);
        missing.sort_by(|a, b| (a.1.depth, a.0).partial_cmp(&(b.1.depth, b.0)).unwrap());
        let regrow = regrown.into_iter().take(regrows);
        let fill = missing.into_iter().take(budget.builds - regrow.len());
        budget.regrows -= regrow.len();
        budget.builds -= regrow.len() + fill.len();
        for (_, node) in regrow.chain(fill) {
            let mesh = build(generator, flora, node, self.afar, cover);
            // The sea counts: over deep water the ground is far below the surface
            // that is actually in view.
            let ground = mesh.vertices.iter().map(|v| v.position);
            let sea = mesh.water.iter().flatten().map(|v| v.position);
            let radius_m = ground
                .chain(sea)
                .map(|position| Vec3::from(position).length())
                .fold(0.0, f32::max);
            let plants_m = flora.map_or(0.0, |flora| self.paint(flora, &mesh.foliage));
            self.built.insert(
                node,
                Built {
                    center: mesh.origin,
                    radius_m: f64::from(radius_m),
                    seen_m: f64::from(radius_m.max(plants_m)),
                    last_used: self.frame,
                    grows: grows(node) || plants(node, self.afar),
                },
            );
            self.stale.remove(&node);
            self.changes.push(TerrainChange::Add(node.id(), mesh));
        }
        // The sun's shadows are the view's own.
        if let Some(eye) = eyes.first() {
            self.select_shadow_lod(eye.camera.position);
        }
        self.evict();
        draw
    }

    /// Shadows reuse built ancestors, never schedule extra generation. Nearby
    /// contacts keep metre detail; distant terrain casts from a coarser mesh.
    fn select_shadow_lod(&mut self, eye: DVec3) {
        let mut stack: Vec<_> = Sector::ALL
            .into_iter()
            .map(|sector| Node {
                body: self.body,
                sector,
                depth: 0,
                x: 0,
                y: 0,
            })
            .collect();
        while let Some(node) = stack.pop() {
            let Some(built) = self.built.get(&node) else {
                continue;
            };
            let distance = (built.center - eye).length() - built.radius_m;
            let target = if distance < 250.0 {
                0.0
            } else if distance < 350.0 {
                4.0
            } else {
                16.0
            };
            let children = node.children();
            if node.depth < self.body.max_depth()
                && node.side_m() / f64::from(PATCH_GRID) > target
                && children.iter().all(|child| self.built.contains_key(child))
            {
                stack.extend(children);
            } else {
                // Ground coarser than a shadow texel cannot be in a shadow map
                // at all: one of its triangles spans hundreds of texels and its
                // depth varies across a single one by more than any bias can
                // lift, so every one of them fails its own depth test and turns
                // black. Nothing is lost by leaving it out, because ground seen
                // from that far is lit by the angle of the sun, which is what
                // lights a body from space anyway.
                if node.side_m() / f64::from(PATCH_GRID) > scene::SHADOW_CASTER_CELL_M {
                    continue;
                }
                self.casters.push(node.id());
                self.built
                    .get_mut(&node)
                    .expect("selected built node")
                    .last_used = self.frame;
            }
        }
    }

    /// The spacing of the finest mesh built so far over a direction from the
    /// body's centre: the footprint the ground is *drawn* with there. Collision
    /// uses the full terrain, and a coarse mesh can sit well above it (small
    /// craters are faded out of it), so whatever must stay above what is on
    /// screen, the camera and a flyer, asks for the ground at this footprint.
    pub fn drawn_footprint_m(&self, direction: DVec3) -> f64 {
        let grid = self.body.grid();
        let point = grid.surface_point(direction.to_array());
        let side = f64::from(grid.side());
        let (s, t) = (
            (point.u / side).clamp(0.0, 1.0 - 1e-12),
            (point.v / side).clamp(0.0, 1.0 - 1e-12),
        );
        let mut finest = Node {
            body: self.body,
            sector: point.sector,
            depth: 0,
            x: 0,
            y: 0,
        };
        for depth in 1..=self.body.max_depth() {
            let cells = f64::from(1u32 << depth);
            let node = Node {
                body: self.body,
                sector: point.sector,
                depth,
                x: (s * cells) as u32,
                y: (t * cells) as u32,
            };
            if !self.built.contains_key(&node) {
                break;
            }
            finest = node;
        }
        finest.side_m() / f64::from(G)
    }

    /// Drops the patches unused for the longest while the cache is over size.
    fn evict(&mut self) {
        if self.built.len() <= CACHE_PATCHES {
            return;
        }
        let mut idle: Vec<(u64, Node)> = self
            .built
            .iter()
            .filter(|(_, built)| built.last_used < self.frame)
            .map(|(node, built)| (built.last_used, *node))
            .collect();
        idle.sort_by_key(|(last_used, node)| (*last_used, core::cmp::Reverse(node.depth)));
        let excess = self.built.len() - CACHE_PATCHES;
        for (_, node) in idle.into_iter().take(excess) {
            self.built.remove(&node);
            self.stale.remove(&node);
            self.changes.push(TerrainChange::Remove(node.id()));
        }
    }
}

/// Inside the view frustum and not behind the body's own horizon.
fn visible(body: Body, camera: &Camera, aspect: f32, center: DVec3, radius_m: f64) -> bool {
    // Horizon: compare angles at the body's centre. Ground never sits below
    // the deepest floor, so a sphere at that depth is the occluder.
    let eye = camera.position.length();
    let reach = center.length() + radius_m;
    let angle = (camera.position / eye)
        .dot(center.normalize())
        .clamp(-1.0, 1.0)
        .acos();
    let occluder = body.radius_m() - body.deepest_m();
    let horizon = (occluder / eye).min(1.0).acos() + (occluder / reach).min(1.0).acos();
    if angle - radius_m / body.radius_m() > horizon {
        return false;
    }

    // Frustum: four side planes through the eye, in camera space.
    let p = camera.rotation.inverse() * (center - camera.position);
    let half_y = f64::from(camera.fov_y / 2.0);
    let half_x = (half_y.tan() * f64::from(aspect)).atan();
    let outside = |lateral: f64, half: f64| {
        // Signed distance to the plane tilted `half` from the view axis.
        lateral.abs() * half.cos() + p.z * half.sin() > radius_m
    };
    !(outside(p.x, half_x) || outside(p.y, half_y))
}

/// Whether a node carries tufts: on a body that grows them, at a depth
/// that holds any.
fn grows(node: Node) -> bool {
    let side = node.body.grid().side() >> node.depth;
    node.body.kind == Kind::Planet && super::grass::grows(side)
}

/// The most subdivisions short of the finest a patch still carries the
/// plants rooted in it, afar: a 128 m patch, drawn to about 600 m.
const PLANTS_AFAR: u32 = 3;

/// How far a viewer may have plants drawn, metres: never short of the
/// trunks that stop a body, kept a few finest patches about it, and never
/// past the coarsest patches that carry plants.
pub const PLANTS_M: [f32; 2] = [128.0, 600.0];

/// Depths short of the finest whose patches carry the plants rooted in
/// them, for a reach: those whose nearest root, drawn, can stand nearer
/// than where plants come in grown, and never past [`PLANTS_AFAR`]. A patch
/// is drawn while its middle is [`SPLIT_DISTANCE`] of its sides away, so
/// its nearest root is that less half its diagonal; past it, the patch
/// wears its wood's colour instead.
fn afar(body: Body, reach_m: f32) -> u32 {
    let grown_m = f64::from(reach_m * (1.0 - scene::FOLIAGE_GROWING));
    let finest_m =
        body.radius_m() * core::f64::consts::FRAC_PI_2 / f64::from(1u32 << body.max_depth());
    let nearest_m = |afar: u32| {
        finest_m * f64::from(1u32 << afar) * (SPLIT_DISTANCE - core::f64::consts::FRAC_1_SQRT_2)
    };
    (1..=PLANTS_AFAR)
        .take_while(|&afar| nearest_m(afar) < grown_m)
        .last()
        .unwrap_or(0)
}

/// Whether a node carries the plants rooted in it: on a body that grows
/// them, the finest block by block and the next `afar` depths.
fn plants(node: Node, afar: u32) -> bool {
    node.body.has_plants() && node.depth + afar >= node.body.max_depth()
}

/// The ground of a node at its grid of samples, row by row, each with
/// where it stands from the body's centre: one ring past the rim, so rim
/// normals match the neighbours'.
fn samples(generator: &Generator, node: Node) -> Vec<(DVec3, Sample)> {
    let body = node.body;
    let spacing_m = node.side_m() / f64::from(G);
    let stride = (G + 3) as usize;
    let mut samples = Vec::with_capacity(stride * stride);
    for j in -1..=G + 1 {
        for i in -1..=G + 1 {
            let direction =
                node.direction(f64::from(i) / f64::from(G), f64::from(j) / f64::from(G));
            let sample = body.sample(generator, direction, spacing_m);
            samples.push((
                DVec3::from(direction) * (body.radius_m() + sample.height_m),
                sample,
            ));
        }
    }
    samples
}

/// Meshes one node.
fn build(
    generator: &Generator,
    flora: Option<&Flora>,
    node: Node,
    afar: u32,
    cover: Cover,
) -> TerrainMesh {
    let body = node.body;
    let radius = body.radius_m();
    let stride = (G + 3) as usize;
    let spacing_m = node.side_m() / f64::from(G);
    let samples = samples(generator, node);
    let at = |i: i32, j: i32| &samples[(j + 1) as usize * stride + (i + 1) as usize];

    let origin = at(G / 2, G / 2).0;
    // Past the plants drawn, a patch wears the wood's colour where they
    // would stand, so a forest stays one past them.
    let canopy = flora.filter(|_| node.body.has_plants()).map(|flora| {
        let direction = |[i, j]: [u32; 2]| at(i as i32, j as i32).0.normalize().to_array();
        flora.canopy(generator, node.side_m(), G as u32, direction)
    });
    let mut vertices = Vec::with_capacity(PATCH_VERTICES as usize);
    for j in 0..=G {
        for i in 0..=G {
            let (position, sample) = *at(i, j);
            let along_u = at(i + 1, j).0 - at(i - 1, j).0;
            let along_v = at(i, j + 1).0 - at(i, j - 1).0;
            let normal = along_u.cross(along_v).normalize();
            // A vertex under the shore wears the cover of the highest land
            // beside it, so no beach bleeds uphill across a coarse triangle.
            let cover = if shore(sample) {
                let around = (-1..=1).flat_map(|dj| (-1..=1).map(move |di| (di, dj)));
                around
                    .map(|(di, dj)| at(i + di, j + dj).1)
                    .filter(|beside| !shore(*beside))
                    .max_by(|a, b| a.height_m.total_cmp(&b.height_m))
                    .unwrap_or(sample)
            } else {
                sample
            };
            let canopy = canopy
                .as_ref()
                .map_or(0.0, |canopy| canopy.at([i as u32, j as u32], sample));
            vertices.push(TerrainVertex {
                position: (position - origin).as_vec3().to_array(),
                normal: normal.as_vec3().to_array(),
                color: color(cover),
                canopy: canopy as f32,
            });
        }
    }

    // The square of the sector the patch spans, in blocks.
    let side = body.grid().side() >> node.depth;
    let corner = [node.x, node.y].map(|n| n * side);

    // Skirts hide the cracks between neighbours of different depth.
    let drop_m = spacing_m * 2.0;
    let runs = [
        (0..=G).map(|k| (k, 0)).collect::<Vec<_>>(),
        (0..=G).map(|k| (k, G)).collect(),
        (0..=G).map(|k| (0, k)).collect(),
        (0..=G).map(|k| (G, k)).collect(),
    ];
    for &(i, j) in runs.iter().flatten() {
        let mut vertex = vertices[(j * (G + 1) + i) as usize];
        let down = -(at(i, j).0.normalize() * drop_m).as_vec3();
        vertex.position = (Vec3::from(vertex.position) + down).to_array();
        vertices.push(vertex);
    }

    // The sea over this patch: the same grid flattened to sea level. The ring
    // counts too, so a neighbour's shoreline never leaves a gap at the rim.
    let wet = body.has_sea() && samples.iter().any(|(_, sample)| sample.height_m < 0.0);
    let water = wet.then(|| {
        let sea = |(i, j): (i32, i32)| {
            let (position, sample) = at(i, j);
            WaterVertex {
                position: (position.normalize() * radius - origin)
                    .as_vec3()
                    .to_array(),
                depth_m: -sample.height_m as f32,
            }
        };
        // The sea is see-through, so a lowered skirt would show through the
        // neighbour's surface. The sphere is smooth enough to need none: the
        // skirt run collapses onto the rim.
        let grid = (0..=G).flat_map(|j| (0..=G).map(move |i| (i, j)));
        grid.chain(runs.iter().flatten().copied())
            .map(sea)
            .collect()
    });
    let grass = if grows(node) {
        let beds = flora.map_or_else(Beds::default, |flora| {
            flora.beds(generator, node.sector, corner, side)
        });
        super::grass::build(
            super::grass::Patch {
                seed: generator.recipe().seed,
                sector: node.sector.index() as u32,
                cell: [node.x, node.y],
                depth: node.depth,
                sector_side: body.grid().side(),
            },
            origin,
            &vertices,
            &samples,
            &|column, heights| cover(node.sector, column, heights),
            &beds,
        )
    } else {
        Vec::new()
    };
    let carries = plants(node, afar);
    let plants = match flora.filter(|_| carries) {
        None => Vec::new(),
        Some(flora) if node.depth == body.max_depth() => super::plants::rooted(
            generator,
            flora,
            node.sector,
            corner,
            side,
            origin,
            &vertices,
            cover,
        ),
        Some(flora) => super::plants::standing(
            generator,
            flora,
            node.sector,
            corner,
            side,
            origin,
            &vertices,
            &samples,
            cover,
        ),
    };
    TerrainMesh {
        grass,
        foliage: plants,
        bare: !carries,
        origin,
        vertices,
        // A heightfield patch is a regular grid, so it shares one index buffer
        // with every other patch and carries none of its own.
        indices: Vec::new(),
        water,
    }
}

/// Ground the renderer paints by height over the sea, never by vertex.
fn shore(sample: Sample) -> bool {
    matches!(
        sample.material,
        Material::Sand | Material::Seabed | Material::Water
    )
}

/// Albedo and gloss of the ground cover. Rock on slopes and the shore (sand,
/// sea floor) are the renderer's job: it sees the slope and the height per
/// pixel, the same at every LOD. A shore color here only shows where no land
/// is near, under the renderer's own.
fn color(sample: Sample) -> [u8; 4] {
    match sample.material {
        Material::Snow => [236, 238, 240, 70],
        Material::Sand => [206, 192, 150, 8],
        Material::Grass => [104, 138, 70, 0],
        // A shade off the meadow: open ground that waits for its trees, no tufts.
        Material::Forest => [92, 128, 66, 0],
        Material::Rock => [118, 112, 106, 12],
        // v1 calls everything under the sea water; it is sea floor all the same.
        Material::Seabed | Material::Water => [112, 116, 98, 0],
        Material::Regolith => [112, 110, 105, 0],
        // Worked earth under a volume: bare, a little warmer than rock.
        Material::Plot => [128, 112, 90, 0],
    }
}

#[cfg(test)]
mod tests {
    use topology::{Address, Column};
    use worldgen::Recipe;

    use super::*;

    /// How far a card of leaves is drawn from its middle, a share of its
    /// half side: the rim of the shader's cut (`inside`, foliage.wgsl)
    /// before the leaves rag it.
    const CUT: f32 = 0.82;
    /// How far over the ground drawn at its root a plant's lowest point may
    /// stand, metres: a hair, never a gap.
    const HAIR_M: f64 = 0.02;
    /// How deep under the ground drawn beneath it the deepest mass of a
    /// plant that stands on its leaves sinks, the least and the most, a
    /// share of its height: in the grass, never on it nor buried.
    const SUNK: [f64; 2] = [0.2, 0.5];

    struct Planet {
        generator: Generator,
        flora: Flora,
        body: Body,
    }

    impl Planet {
        fn new(seed: u64) -> Planet {
            let generator = Generator::new(Recipe::new(seed)).unwrap();
            let flora = Flora::grow(&generator);
            let body = Body::new(Kind::Planet, generator.sphere());
            Planet {
                generator,
                flora,
                body,
            }
        }

        /// The node of a depth that holds a column.
        fn node(&self, column: Column, depth: u32) -> Node {
            let side = self.body.grid().side() >> depth;
            Node {
                body: self.body,
                sector: column.sector,
                depth,
                x: u32::from(column.u) / side,
                y: u32::from(column.v) / side,
            }
        }

        fn build(&self, node: Node) -> TerrainMesh {
            let nothing = |_: Sector, _: [f64; 2], _: [f64; 2]| false;
            build(
                &self.generator,
                Some(&self.flora),
                node,
                PLANTS_AFAR,
                &nothing,
            )
        }

        /// The ground a patch draws under a point, metres from the body's
        /// centre, and its normal there.
        fn ground(&self, node: Node, mesh: &TerrainMesh, at: Vec3) -> (f64, Vec3) {
            let grid = self.body.grid();
            let side = grid.side() >> node.depth;
            let point = grid.surface_point((mesh.origin + at.as_dvec3()).normalize().to_array());
            let samples = [(point.u, node.x), (point.v, node.y)]
                .map(|(t, n)| ((t - f64::from(n * side)) / f64::from(side) * f64::from(G)) as f32);
            let (ground, normal) = super::super::grass::surface(
                &mesh.vertices,
                samples[0].clamp(0.0, G as f32),
                samples[1].clamp(0.0, G as f32),
            );
            ((mesh.origin + ground.as_dvec3()).length(), normal)
        }
    }

    /// A coarse patch stands a plant only where the ground it draws there
    /// takes one: the same wood, its samples turned to rock, carries none.
    #[test]
    fn a_coarse_patch_stands_plants_only_where_its_ground_takes_them() {
        let planet = Planet::new(1);
        let wood = topology::place(planet.body.grid(), "0-9AR0000")
            .unwrap()
            .column;
        let node = planet.node(wood, planet.body.max_depth() - 2);
        let mesh = planet.build(node);
        let side = planet.body.grid().side() >> node.depth;
        let corner = [node.x, node.y].map(|n| n * side);
        let nothing = |_: Sector, _: [f64; 2], _: [f64; 2]| false;
        let stand = |samples: &[(DVec3, Sample)]| {
            super::super::plants::standing(
                &planet.generator,
                &planet.flora,
                node.sector,
                corner,
                side,
                mesh.origin,
                &mesh.vertices,
                samples,
                &nothing,
            )
            .len()
        };
        let drawn = samples(&planet.generator, node);
        assert!(stand(&drawn) > 50, "{} plants in the wood", stand(&drawn));
        let rock: Vec<_> = drawn
            .iter()
            .map(|&(at, sample)| {
                (
                    at,
                    Sample {
                        material: worldgen::Material::Rock,
                        ..sample
                    },
                )
            })
            .collect();
        assert_eq!(stand(&rock), 0, "plants stand on rock");
    }

    /// A preset that draws plants nearer carries them on fewer depths: at
    /// Light the 128 m patches stay bare, as no plant of theirs would be
    /// drawn grown, and at Medium and Full they carry them.
    #[test]
    fn the_reach_says_how_far_patches_carry_plants() {
        let planet = Planet::new(1);
        let afar = |effects: Effects| afar(planet.body, effects.foliage_reach_m);
        assert_eq!(afar(Effects::light()), 2);
        assert_eq!(afar(Effects::medium()), PLANTS_AFAR);
        assert_eq!(afar(Effects::default()), PLANTS_AFAR);
    }

    /// Every plant of every form, at every level that draws plants, stands
    /// on the ground drawn under it: its lowest point is at or under that
    /// ground at its root, and never more than a hair over it, on a slope
    /// too. Its lowest point is its lowest wood drawn or the bottom of its
    /// lowest card of leaves.
    #[test]
    fn every_plant_stands_on_the_ground_drawn_under_it() {
        let mut steepest = 1.0f32;
        // Trees and undergrowth stood.
        let mut stood = [0; 2];
        for seed in [0x8d7e_6661_1e10_84c0, 1, 0xdead_beef] {
            let planet = Planet::new(seed);
            let shapes = 0..planet.flora.shapes().len() as u32;
            let forms: Vec<_> = shapes
                .map(|index| super::super::plants::shape(&planet.flora, index))
                .collect();
            let finest = planet.body.max_depth();
            let side = planet.body.grid().side();
            let woods = Sector::ALL
                .into_iter()
                .flat_map(|sector| (1..48).map(move |k| (sector, k)))
                .map(|(sector, k)| Column::new(sector, (k * side / 48) as u16, (side / 3) as u16))
                .filter(|column| {
                    let node = planet.node(*column, finest);
                    let corner = [node.x, node.y].map(|n| n * (side >> finest));
                    let plants = planet.flora.plants(
                        &planet.generator,
                        column.sector,
                        corner,
                        side >> finest,
                    );
                    plants.len() >= 2
                })
                .take(6);
            for column in woods {
                for depth in finest - PLANTS_AFAR..=finest {
                    let node = planet.node(column, depth);
                    let mesh = planet.build(node);
                    for instance in &mesh.foliage {
                        let axes = instance.axes.map(Vec3::from);
                        let origin = Vec3::from(instance.origin);
                        let up = axes[2].normalize();
                        let (root_m, normal) = planet.ground(node, &mesh, origin);
                        steepest = steepest.min(normal.dot(up));
                        // On the ground its patch draws, whatever the
                        // level: so it never jumps over the ground as its
                        // patch splits.
                        let origin_m = (mesh.origin + origin.as_dvec3()).length();
                        assert!(
                            (origin_m - root_m).abs() < 0.01,
                            "seed {seed:x} depth {depth}: a plant stands {:.2} m off its ground",
                            origin_m - root_m
                        );
                        stood[usize::from(forms[instance.shape as usize].undergrowth)] += 1;
                        let shape = &planet.flora.shapes()[instance.shape as usize];
                        if !shape.wood_shows() {
                            let sinks = shape.crown().iter().map(|puff| {
                                let [x, y, z] = puff.middle.map(|p| p as f32);
                                let middle = origin
                                    + axes[0] * (x - 0.5)
                                    + axes[1] * (y - 0.5)
                                    + axes[2] * z;
                                let height_m = f64::from(axes[2].length()) * puff.radii[1];
                                let bottom_m =
                                    (mesh.origin + middle.as_dvec3()).length() - height_m;
                                let (ground_m, _) = planet.ground(node, &mesh, middle);
                                (ground_m - bottom_m) / height_m
                            });
                            let deepest = sinks.fold(f64::NEG_INFINITY, f64::max);
                            assert!(
                                (SUNK[0]..=SUNK[1]).contains(&deepest),
                                "seed {seed:x} depth {depth}: shape {} sinks {deepest:.2} of its lowest mass at {column:?}",
                                instance.shape,
                            );
                        }
                        for (form, mesh_of) in
                            forms[instance.shape as usize].forms.iter().enumerate()
                        {
                            let drawn = &mesh_of.indices
                                [mesh_of.drawn.start as usize..mesh_of.drawn.end as usize];
                            let lowest_m = drawn
                                .iter()
                                .map(|&k| {
                                    let vertex = &mesh_of.vertices[k as usize];
                                    let [x, y, z] = vertex.position;
                                    let card = vertex.card[2] * axes[2].length() * CUT;
                                    let at = origin + axes[0] * x + axes[1] * y + axes[2] * z
                                        - up * card;
                                    (mesh.origin + at.as_dvec3()).length()
                                })
                                .fold(f64::INFINITY, f64::min);
                            let over_m = lowest_m - root_m;
                            assert!(
                                over_m <= HAIR_M,
                                "seed {seed:x} depth {depth}: shape {} in form {form} stands {over_m:.2} m over the ground at {column:?}",
                                instance.shape,
                            );
                        }
                    }
                }
            }
        }
        // Slopes were among the places: ground tilted past where plants
        // begin to thin out.
        assert!(
            steepest < 0.9,
            "the steepest ground under a plant: {steepest}"
        );
        assert!(
            stood.iter().all(|&n| n >= 100),
            "trees and undergrowth: {stood:?}"
        );
    }

    /// The finest patches draw a tree in the blocks it fills: each block's
    /// middle in its own column, at its own layer over the root's, all of
    /// them lifted alike onto the ground drawn at the root, within the root
    /// block. Undergrowth leans with the ground, and stops no body.
    #[test]
    fn a_tree_is_drawn_in_the_blocks_it_fills() {
        let planet = Planet::new(1);
        let sphere = planet.generator.sphere();
        let side = sphere.blocks().side();
        let sector = Sector::ALL[0];
        let finest = planet.body.max_depth();
        let node = (1..30)
            .map(|k| {
                planet.node(
                    Column::new(sector, (k * side / 32) as u16, (side / 2) as u16),
                    finest,
                )
            })
            .find(|node| {
                let corner = [node.x, node.y].map(|n| n * (side >> finest));
                let plants = planet
                    .flora
                    .plants(&planet.generator, sector, corner, side >> finest);
                let shapes = planet.flora.shapes();
                let trees = plants
                    .iter()
                    .filter(|plant| shapes[usize::from(plant.shape)].wood_shows());
                trees.count() >= 2
            })
            .expect("a wood");
        let corner = [node.x, node.y].map(|n| n * (side >> finest));
        let mesh = planet.build(node);
        let plants = planet
            .flora
            .plants(&planet.generator, sector, corner, side >> finest);
        assert_eq!(mesh.foliage.len(), plants.len());
        let mut trees = 0;
        for (plant, instance) in plants.iter().zip(&mesh.foliage) {
            let shape = &planet.flora.shapes()[usize::from(plant.shape)];
            if !shape.wood_shows() {
                continue;
            }
            trees += 1;
            let axes = instance.axes.map(Vec3::from);
            let up = axes[2].normalize();
            let placed = |p: [i32; 3]| {
                let middle = [p[0] as f32, p[1] as f32, p[2] as f32 + 0.5];
                Vec3::from(instance.origin)
                    + axes[0] * middle[0]
                    + axes[1] * middle[1]
                    + axes[2] * middle[2]
            };
            let lift = |p: [i32; 3], turned: [i32; 3]| {
                let root = plant.root;
                let column = Column::new(
                    sector,
                    (i32::from(root.column.u) + turned[0]) as u16,
                    (i32::from(root.column.v) + turned[1]) as u16,
                );
                let block = Address::new(column, root.h + turned[2] as i16);
                let exact = (DVec3::from(sphere.block_centre(block)) - mesh.origin).as_vec3();
                let off = placed(p) - exact;
                let across = (off - up * off.dot(up)).length();
                assert!(
                    across < 0.01,
                    "{plant:?} block {p:?} is {across} m off its column"
                );
                off.dot(up)
            };
            let mut blocks = shape.blocks().zip(planet.flora.blocks(*plant));
            let ((p, _), (turned, _)) = blocks.next().expect("a plant has blocks");
            let lifted = lift(p, turned);
            let block_m = topology::BLOCK_M as f32;
            assert!(
                (-0.01..block_m + 0.01).contains(&lifted),
                "{plant:?} lifted {lifted} m"
            );
            for ((p, _), (turned, _)) in blocks {
                let off = lift(p, turned) - lifted;
                assert!(
                    off.abs() < 0.01,
                    "{plant:?} block {p:?} is {off} m off its layer"
                );
            }
        }
        assert!(trees >= 2, "{trees} trees");
    }

    /// A plant whose painted shape meets the view is drawn: from an eye low
    /// in a wood and walking through it, turned through a full circle and
    /// from the feet to overhead, at each preset's reach, every patch with
    /// a mass of leaves or a piece of wood of its plants in the view, past
    /// the near plane and within the reach, is among the patches drawn.
    #[test]
    fn every_plant_in_view_is_drawn() {
        let planet = Planet::new(1);
        let grid = planet.body.grid();
        let wood = topology::place(grid, "0-9AR0000").unwrap().column;
        let up = DVec3::from(grid.column_direction(wood));
        let ground_m = planet.body.radius_m()
            + planet
                .body
                .sample(&planet.generator, up.to_array(), 1.0)
                .height_m;
        let north = up.any_orthonormal_vector();
        let east = up.cross(north);
        // The eye stands low, then higher as a boom hangs it, stepping
        // forward through the trunks.
        let eyes: Vec<DVec3> = [(0.0, 1.7), (4.0, 1.7), (8.0, 1.7), (12.0, 4.0)]
            .into_iter()
            .map(|(ahead_m, over_m)| up * (ground_m + over_m) + north * ahead_m)
            .collect();
        let camera = |position: DVec3, forward: DVec3, fov_y: f32| {
            let up = position.normalize();
            let back = -forward.normalize();
            let right = up.cross(back).normalize_or(north);
            let rotation =
                glam::DQuat::from_mat3(&glam::DMat3::from_cols(right, back.cross(right), back));
            Viewpoint {
                camera: Camera {
                    position,
                    rotation,
                    fov_y,
                    near: 0.1,
                },
                coarseness: 1.0,
            }
        };
        let aspect = 16.0 / 9.0;
        let nothing = |_: Sector, _: [f64; 2], _: [f64; 2]| false;
        let mut missed = Vec::new();
        for effects in [Effects::light(), Effects::medium(), Effects::default()] {
            let reach_m = f64::from(effects.foliage_reach_m);
            let mut terrain = Terrain::new(planet.body);
            terrain.set_reach(effects.foliage_reach_m);
            let mut meshes: HashMap<PatchId, TerrainMesh> = HashMap::new();
            // Each shape's points as painted, blocks about its root: every
            // mass's middle and every corner of its wood.
            let mut shapes: HashMap<u32, Vec<Vec3>> = HashMap::new();
            for &eye in &eyes {
                let ahead = camera(eye, north, 1.0);
                terrain.settle(
                    &planet.generator,
                    Some(&planet.flora),
                    &nothing,
                    &[ahead],
                    aspect,
                );
                for change in terrain.drain_changes() {
                    match change {
                        TerrainChange::Add(id, mesh) => {
                            meshes.insert(id, mesh);
                        }
                        TerrainChange::Remove(id) => {
                            meshes.remove(&id);
                        }
                        TerrainChange::Paint(index, shape) => {
                            let points = shape.forms[0].vertices.iter().map(|v| Vec3::from(v.mass));
                            shapes.insert(index, points.collect());
                        }
                        TerrainChange::Unpaint => shapes.clear(),
                    }
                }
                // The patches drawn from here, whichever way the eye turns:
                // a wide view down each axis.
                let around: HashSet<PatchId> = [up, -up, north, -north, east, -east]
                    .into_iter()
                    .flat_map(|axis| {
                        let wide = camera(eye, axis + north * 1e-3, 2.4);
                        terrain.update(
                            &planet.generator,
                            Some(&planet.flora),
                            &nothing,
                            &[wide],
                            1.0,
                            &mut Budget::frame(),
                        )
                    })
                    .flatten()
                    .collect();
                for bearing in (0..16).map(|k| f64::from(k) * core::f64::consts::TAU / 16.0) {
                    for pitch_deg in [-80.0, -45.0, -20.0, 0.0, 20.0, 45.0, 80.0f64] {
                        let pitch = pitch_deg.to_radians();
                        let level = north * bearing.cos() + east * bearing.sin();
                        let forward = level * pitch.cos() + up * pitch.sin();
                        let view = camera(eye, forward, 1.0);
                        let drawn: HashSet<PatchId> = terrain
                            .update(
                                &planet.generator,
                                Some(&planet.flora),
                                &nothing,
                                &[view],
                                aspect,
                                &mut Budget::frame(),
                            )
                            .into_iter()
                            .flatten()
                            .collect();
                        let inverse = view.camera.rotation.inverse();
                        let half_y = 0.5f64;
                        let half_x = (half_y.tan() * f64::from(aspect)).atan();
                        let seen = |point: DVec3| {
                            let at = inverse * (point - eye);
                            let depth = -at.z;
                            depth > 0.1
                                && at.length() < reach_m
                                && at.x.abs() <= depth * half_x.tan()
                                && at.y.abs() <= depth * half_y.tan()
                        };
                        for id in around.difference(&drawn) {
                            let Some(mesh) = meshes.get(id) else { continue };
                            let in_view = mesh.foliage.iter().any(|plant| {
                                let axes = plant.axes.map(|axis| Vec3::from(axis).as_dvec3());
                                let root = mesh.origin + Vec3::from(plant.origin).as_dvec3();
                                shapes[&plant.shape].iter().any(|p| {
                                    let p = p.as_dvec3();
                                    seen(root + axes[0] * p.x + axes[1] * p.y + axes[2] * p.z)
                                })
                            });
                            if in_view {
                                missed.push((
                                    effects.foliage_reach_m,
                                    eye,
                                    bearing.to_degrees(),
                                    pitch_deg,
                                ));
                            }
                        }
                    }
                }
            }
        }
        assert!(
            missed.is_empty(),
            "{} patches with plants in view were not drawn, the first {:?}",
            missed.len(),
            missed.first()
        );
    }
}
