//! Foliage: each shape of a world's a mesh a form, drawn many times. A
//! terrain patch carries its foliage in its own space; once a sight, that
//! of the patches it draws is gathered by form around the camera, and each
//! form of each shape is one instanced draw. An instance is drawn in the
//! form its distance calls for and crosses to the next over a band, a share
//! of its pixels at a time, so it never changes at once. At the edge of its
//! reach it rises from its root as the eye comes nearer, out of a ground
//! that already wears its wood's colour, so none ever pops into being. That
//! of the patches that cast is gathered again for each cascade of the sun's
//! shadows it falls in, in one form, and draws only its wood and a round
//! card a mass of leaves. foliage.wgsl paints it.

use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use scene::{
    Effects, FOLIAGE_FLUTTER, FOLIAGE_FORMS, FOLIAGE_SWAY_M, FoliageInstance, FoliageMesh,
    FoliageShape, FoliageVertex,
};
use wgpu::util::DeviceExt;

use crate::shadow::{self, CASCADES};
use crate::{PipelineSpec, Surface, pipeline};

/// The last share of the way to where a form ends that an instance crosses to
/// the next over.
const BAND: f32 = 0.12;
pub use scene::FOLIAGE_GROWING as GROWING;
/// How much of that band an instance's own roll moves where it starts to grow,
/// so a row of instances never rises as one wall.
const STAGGER: f32 = 0.4;

/// One instance as the vertex stage reads it: where each axis of its mesh goes
/// and where its origin stands, relative to the camera, its seed, and how
/// it is drawn. `drawn.x` is how much: 1 all of it; `d` in `0..1`, the
/// share `d` of its pixels; `-d`, the rest of them, so two forms of an instance
/// crossing draw each pixel once. `drawn.y` is the share of its size it has
/// grown to.
#[derive(Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct Instance {
    axes: [[f32; 3]; 3],
    origin: [f32; 3],
    seed: f32,
    drawn: [f32; 2],
}

/// A world's shape as the renderer holds it: each form's mesh, `None` for
/// one with no side to draw.
struct Shape {
    forms: [Option<Mesh>; FOLIAGE_FORMS],
    undergrowth: bool,
    /// How far from its root any form of it is painted, blocks.
    reach: f32,
}

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// The run of indices drawn in view, and the run that casts.
    drawn: Range<u32>,
    casts: Range<u32>,
    /// How far from its root it is painted, blocks.
    reach: f32,
}

pub struct Foliage {
    pipeline: wgpu::RenderPipeline,
    shadow_pipeline: wgpu::RenderPipeline,
    /// A world's foliage shapes, by the index its instances name them by:
    /// those painted so far.
    shapes: Vec<Option<Shape>>,
}

/// What one sight drew of the foliage: how many in each form, and the draws
/// and triangles it took.
#[derive(Clone, Copy, Debug, Default)]
pub struct Counted {
    pub forms: [u32; FOLIAGE_FORMS],
    pub draws: u32,
    pub triangles: u64,
}

/// The foliage's shader, one source for both passes: `casting`, the sun's,
/// where a card turns to the sun. It moves foliage off its mesh as far as
/// [`FOLIAGE_FLUTTER`] and [`FOLIAGE_SWAY_M`] let it, which its bounds hold.
pub(crate) fn source(casting: bool) -> String {
    format!(
        "const CASTING: bool = {casting};\nconst FLUTTER: f32 = {FOLIAGE_FLUTTER:?};\n\
         const SWAY_M: f32 = {FOLIAGE_SWAY_M:?};\n{}",
        include_str!("shaders/foliage.wgsl")
    )
}

impl Foliage {
    pub fn new(
        device: &wgpu::Device,
        view_layout: &wgpu::BindGroupLayout,
        shadow_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Foliage {
        let buffers = &[
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<FoliageVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![
                    0 => Float32x3, 1 => Float32x3, 2 => Unorm8x4, 3 => Float32x4,
                    9 => Float32x3
                ],
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: size_of::<Instance>() as u64,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &wgpu::vertex_attr_array![
                    4 => Float32x3, 5 => Float32x3, 6 => Float32x3, 7 => Float32x3, 8 => Float32,
                    10 => Float32x2
                ],
            }),
        ];
        let make = |layout: &wgpu::BindGroupLayout, surface: Surface, casting: bool| {
            let source = source(casting);
            pipeline(
                device,
                format,
                PipelineSpec {
                    label: "foliage",
                    source: &source,
                    layouts: &[layout],
                    buffers,
                    surface,
                },
            )
        };
        Foliage {
            pipeline: make(view_layout, Surface::Foliage, false),
            shadow_pipeline: make(shadow_layout, Surface::ShadowCutout, true),
            shapes: Vec::new(),
        }
    }

    /// Holds one of a world's foliage shapes at its index, in place of any
    /// held there.
    pub fn paint(&mut self, device: &wgpu::Device, index: u32, shape: &FoliageShape) {
        let index = index as usize;
        if self.shapes.len() <= index {
            self.shapes.resize_with(index + 1, || None);
        }
        self.shapes[index] = Some(Shape {
            forms: shape.forms.each_ref().map(|form| Mesh::new(device, form)),
            undergrowth: shape.undergrowth,
            reach: shape.reach(),
        });
    }

    /// How far from a patch's origin the foliage rooted in it is painted,
    /// metres, of the shapes held: none of it is drawn past.
    pub fn reach_m(&self, foliage: &[FoliageInstance]) -> f32 {
        foliage
            .iter()
            .filter_map(|placed| {
                let shape = self.shapes.get(placed.shape as usize)?.as_ref()?;
                Some(Vec3::from(placed.origin).length() + placed.reach_m(shape.reach, 1.0))
            })
            .fold(0.0, f32::max)
    }

    /// Lets every shape go.
    pub fn unpaint(&mut self) {
        self.shapes.clear();
    }

    /// Gathers into a sight's instances the foliage it draws, each patch by
    /// the centre of its space relative to the camera, and of the patches
    /// that cast, the foliage each cascade of the sun's shadows holds, each
    /// in the forms `effects` calls for at its distance. An instance naming
    /// a shape the renderer does not hold is skipped.
    pub fn place<'a>(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        instances: &mut Instances,
        effects: &Effects,
        seen: impl Iterator<Item = (Vec3, &'a [FoliageInstance])>,
        casting: Option<(
            impl Iterator<Item = (Vec3, &'a [FoliageInstance])>,
            &[Mat4; CASCADES],
        )>,
    ) -> Placed {
        let reach = Reach::new(effects);
        let mut sets = core::mem::take(&mut instances.sets);
        sets.iter_mut().for_each(Vec::clear);
        for (center, foliage) in seen {
            for placed in foliage {
                let Some((shape, mut instance)) = self.instance(center, placed, &reach) else {
                    continue;
                };
                let distance = Vec3::from(instance.origin).length();
                for (form, drawn) in reach.forms(distance, placed.seed) {
                    if shape.forms[form].is_some() {
                        instance.drawn = drawn;
                        sets[0].push((key(placed.shape, form), instance));
                    }
                }
            }
        }
        if let Some((casters, matrices)) = casting {
            for (center, foliage) in casters {
                for placed in foliage {
                    let Some((shape, mut instance)) = self.instance(center, placed, &reach) else {
                        continue;
                    };
                    let origin = Vec3::from(instance.origin);
                    let Some((form, grown)) = reach.casting(origin.length(), placed.seed) else {
                        continue;
                    };
                    let Some(mesh) = &shape.forms[form] else {
                        continue;
                    };
                    instance.drawn = [1.0, grown];
                    let radius_m = placed.reach_m(mesh.reach, grown);
                    for (cascade, matrix) in matrices.iter().enumerate() {
                        if shadow::intersects(*matrix, origin, radius_m) {
                            sets[1 + cascade].push((key(placed.shape, form), instance));
                        }
                    }
                }
            }
        }
        let [view, cascades @ ..] = instances.write(device, queue, &mut sets);
        instances.sets = sets;
        instances.counted = self.count(&view);
        Placed { view, cascades }
    }

    /// An instance of a patch around the camera, whole, and its shape; none
    /// for undergrowth `reach` thins out.
    fn instance(
        &self,
        center: Vec3,
        placed: &FoliageInstance,
        reach: &Reach,
    ) -> Option<(&Shape, Instance)> {
        let shape = self.shapes.get(placed.shape as usize)?.as_ref()?;
        if shape.undergrowth && placed.seed >= reach.undergrowth {
            return None;
        }
        let instance = Instance {
            axes: placed.axes,
            origin: (center + Vec3::from(placed.origin)).to_array(),
            seed: placed.seed,
            drawn: [1.0, 1.0],
        };
        Some((shape, instance))
    }

    /// What a gathering in view draws.
    fn count(&self, gathered: &Gathered) -> Counted {
        let mut counted = Counted::default();
        for (key, placed) in &gathered.runs {
            let Some(mesh) = self.mesh(*key) else {
                continue;
            };
            counted.forms[*key as usize % FOLIAGE_FORMS] += placed.len() as u32;
            counted.draws += 1;
            counted.triangles += u64::from(mesh.drawn.len() as u32 / 3) * placed.len() as u64;
        }
        counted
    }

    /// The mesh of one form of one shape, by its key.
    fn mesh(&self, key: u32) -> Option<&Mesh> {
        let key = key as usize;
        self.shapes.get(key / FOLIAGE_FORMS)?.as_ref()?.forms[key % FOLIAGE_FORMS].as_ref()
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, instances: &Instances, placed: &Placed) {
        self.draw_with(pass, &self.pipeline, instances, &placed.view, |mesh| {
            mesh.drawn.clone()
        });
    }

    pub fn draw_shadow(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        instances: &Instances,
        placed: &Placed,
        cascade: usize,
    ) {
        let gathered = &placed.cascades[cascade];
        self.draw_with(pass, &self.shadow_pipeline, instances, gathered, |mesh| {
            mesh.casts.clone()
        });
    }

    /// Draws a gathering, of each mesh the run of indices `run` says.
    fn draw_with(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        instances: &Instances,
        gathered: &Gathered,
        run: impl Fn(&Mesh) -> Range<u32>,
    ) {
        if gathered.runs.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        let stride = size_of::<Instance>() as u64;
        for (key, placed) in &gathered.runs {
            let Some(mesh) = self.mesh(*key) else {
                continue;
            };
            let indices = run(mesh);
            if indices.is_empty() {
                continue;
            }
            let bytes = u64::from(placed.start) * stride..u64::from(placed.end) * stride;
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            pass.set_vertex_buffer(1, instances.buffer.slice(bytes));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(indices, 0, 0..placed.len() as u32);
        }
    }
}

impl Mesh {
    fn new(device: &wgpu::Device, mesh: &FoliageMesh) -> Option<Mesh> {
        if mesh.indices.is_empty() {
            return None;
        }
        let reach = mesh.reach();
        Some(Mesh {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("foliage"),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("foliage indices"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            drawn: mesh.drawn.clone(),
            casts: mesh.casts.clone(),
            reach,
        })
    }
}

/// Where one form of one shape is gathered: its runs are drawn in the
/// order of shapes, nearest form first.
fn key(shape: u32, form: usize) -> u32 {
    shape * FOLIAGE_FORMS as u32 + form as u32
}

/// How far each form of foliage is drawn: what [`Effects`] asks, ordered.
struct Reach {
    /// Where each form ends, metres from the eye, the last the reach.
    ends: [f32; FOLIAGE_FORMS],
    undergrowth: f32,
}

impl Reach {
    fn new(effects: &Effects) -> Reach {
        let reach = effects.foliage_reach_m.max(0.0);
        let mid = effects.foliage_mid_m.clamp(0.0, reach);
        Reach {
            ends: [effects.foliage_near_m.clamp(0.0, mid), mid, reach],
            undergrowth: effects.undergrowth,
        }
    }

    /// The form an instance `distance` metres off is drawn in, and how far
    /// through its band to the next it is, `0..1`; none past the reach.
    /// The last form has no band: an instance grows out of it instead.
    fn at(&self, distance: f32) -> Option<(usize, f32)> {
        let form = self.ends.iter().position(|&end| distance < end)?;
        if form + 1 == FOLIAGE_FORMS {
            return Some((form, 0.0));
        }
        let end = self.ends[form];
        // A band never reaches back past the form before: an instance there
        // has only just come to this one.
        let before = form.checked_sub(1).map_or(0.0, |before| self.ends[before]);
        let start = (end * (1.0 - BAND)).max(before);
        let through = if distance > start {
            (distance - start) / (end - start)
        } else {
            0.0
        };
        Some((form, through))
    }

    /// The share of its size an instance `distance` metres off has grown to,
    /// by its seed: nothing at the reach, all of it a band nearer, eased so
    /// the last of it comes slowly.
    fn grown(&self, distance: f32, seed: f32) -> f32 {
        let reach = self.ends[FOLIAGE_FORMS - 1];
        let band = reach * GROWING;
        if band <= 0.0 {
            return 0.0;
        }
        let nearer = ((reach - distance) / band).clamp(0.0, 1.0);
        // Its own start, by a roll apart from the seed's other uses.
        let start = (seed * 7.31).fract() * STAGGER;
        let own = ((nearer - start) / (1.0 - STAGGER)).clamp(0.0, 1.0);
        1.0 - (1.0 - own).powi(3)
    }

    /// The forms an instance `distance` metres off is drawn in, each with how:
    /// one whole, or in a band the one going and the one coming; none
    /// before it has begun to grow.
    fn forms(&self, distance: f32, seed: f32) -> impl Iterator<Item = (usize, [f32; 2])> {
        let grown = self.grown(distance, seed);
        let (going, coming) = match self.at(distance) {
            _ if grown <= 0.0 => (None, None),
            None => (None, None),
            Some((form, through)) if through <= 0.0 => (Some((form, [1.0, grown])), None),
            Some((form, through)) => (
                Some((form, [-through, grown])),
                Some((form + 1, [through, grown])),
            ),
        };
        going.into_iter().chain(coming)
    }

    /// The one form an instance `distance` metres off casts in, the one it is
    /// drawn in until half way through its band, then the next, and the
    /// share of its size it has grown to.
    fn casting(&self, distance: f32, seed: f32) -> Option<(usize, f32)> {
        let grown = self.grown(distance, seed);
        let (form, through) = self.at(distance).filter(|_| grown > 0.0)?;
        Some((form + usize::from(through > 0.5), grown))
    }
}

/// One gathering of foliage: each form of each shape, and the run of its
/// instances in the buffer.
#[derive(Default)]
struct Gathered {
    runs: Vec<(u32, Range<u32>)>,
}

/// What one sight draws of the foliage this frame: those in view, and those
/// each cascade of the sun's shadows holds.
pub struct Placed {
    view: Gathered,
    cascades: [Gathered; CASCADES],
}

/// One sight's foliage this frame, every gathering one after another. Grows,
/// never shrinks.
pub struct Instances {
    buffer: wgpu::Buffer,
    capacity: usize,
    /// The gatherings of the last frame, the view's then each cascade's,
    /// kept to be filled again.
    sets: [Vec<(u32, Instance)>; 1 + CASCADES],
    /// The last frame's instances as written, kept likewise.
    data: Vec<Instance>,
    /// What the sight drew in view, the last frame.
    pub counted: Counted,
}

impl Instances {
    pub fn new(device: &wgpu::Device) -> Instances {
        let capacity = 256;
        Instances {
            buffer: Self::allocate(device, capacity),
            capacity,
            sets: Default::default(),
            data: Vec::new(),
            counted: Counted::default(),
        }
    }

    fn allocate(device: &wgpu::Device, capacity: usize) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("foliage instances"),
            size: (size_of::<Instance>() * capacity) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Writes each gathering by key, one after another, and says where
    /// each key's run of each one is.
    fn write<const N: usize>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        sets: &mut [Vec<(u32, Instance)>; N],
    ) -> [Gathered; N] {
        let total = sets.iter().map(Vec::len).sum::<usize>();
        if total > self.capacity {
            self.capacity = total.next_power_of_two();
            self.buffer = Self::allocate(device, self.capacity);
        }
        let data = &mut self.data;
        data.clear();
        let gathered = sets.each_mut().map(|set| {
            // Cut leaves write depth and blend nothing: the order within a
            // run draws the same picture.
            set.sort_unstable_by_key(|(key, _)| *key);
            let mut runs: Vec<(u32, Range<u32>)> = Vec::new();
            for &(key, instance) in set.iter() {
                let at = data.len() as u32;
                match runs.last_mut() {
                    Some((last, run)) if *last == key => run.end = at + 1,
                    _ => runs.push((key, at..at + 1)),
                }
                data.push(instance);
            }
            Gathered { runs }
        });
        if !data.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
        }
        gathered
    }
}
