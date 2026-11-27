//! What a frame costs, for a shell that asks: the GPU's time of each pass
//! where the device writes timestamps, the CPU's of each step of placing
//! the foliage by a clock the shell lends, and the foliage's fragments
//! counted in each pass that draws it. A GPU may run the next pass while
//! the last one finishes, so a pass's time is no share of the frame's: a
//! draw is priced by what the frame saves without it ([`Without`]). In the
//! world none of it is on, and a frame draws as it always does.

use std::sync::{Arc, Mutex};

use scene::FOLIAGE_FORMS;

use crate::foliage::{self, Counted, Spent};
use crate::shadow::{self, CASCADES};

/// What the last frames measured of the first view.
#[derive(Clone, Debug, Default)]
pub struct Measured {
    /// Whether the device writes timestamps: without, `passes` stays empty.
    pub timed: bool,
    /// The frames drawn since the measure began: `spent` is the last one's.
    pub drawn: u64,
    /// The frame, by `drawn`'s count, that `passes` were read back from: a
    /// shell that reads while frames go on tells a new reading by it.
    pub read: u64,
    /// What that frame left out.
    pub without: Without,
    /// The GPU's milliseconds of each pass of the frame read back, in the
    /// order drawn; `"frame"` first, from the start of its first pass to
    /// the end of its last.
    pub passes: Vec<(&'static str, f64)>,
    /// What the CPU spent placing the foliage, the last frame.
    pub spent: Spent,
    /// What the foliage drew, in view then in each cascade.
    pub counted: [Counted; 1 + CASCADES],
    /// The foliage's fragments, in view then in each cascade, of the last
    /// frame counted.
    pub fragments: Option<[Fragments; 1 + CASCADES]>,
}

impl Measured {
    /// All of it as JSON, for a shell to write down or hand to a page.
    pub fn json(&self) -> String {
        let list = |items: Vec<String>| items.join(", ");
        let passes = list(
            self.passes
                .iter()
                .map(|(label, ms)| format!("\"{label}\": {ms:.4}"))
                .collect(),
        );
        let counted = list(
            self.counted
                .iter()
                .map(|counted| {
                    format!(
                        "{{\"forms\": {:?}, \"draws\": {}, \"triangles\": {:?}}}",
                        counted.forms, counted.draws, counted.triangles
                    )
                })
                .collect(),
        );
        let fragments = self.fragments.map_or("null".to_owned(), |all| {
            let or_null = |value: Option<String>| value.unwrap_or_else(|| "null".to_owned());
            let each = all.iter().map(|f| {
                format!(
                    "{{\"pixels\": {}, \"rasterized\": {}, \"kept\": {}, \"before\": {}, \"covered\": {}, \"forms\": {}}}",
                    f.pixels,
                    f.rasterized,
                    f.kept,
                    or_null(f.before.map(|before| before.to_string())),
                    f.covered,
                    or_null(f.forms.map(|forms| format!("{forms:?}"))),
                )
            });
            format!("[{}]", list(each.collect()))
        });
        let Spent {
            gather_ms,
            sort_ms,
            write_ms,
        } = self.spent;
        format!(
            "{{\"timed\": {}, \"drawn\": {}, \"read\": {}, \"without\": {{\"view\": {}, \"cascades\": {:?}, \"forms\": {:?}}}, \
             \"passes_ms\": {{{passes}}}, \"spent_ms\": {{\"gather\": {gather_ms:.4}, \"sort\": {sort_ms:.4}, \"write\": {write_ms:.4}}}, \
             \"counted\": [{counted}], \"fragments\": {fragments}}}",
            self.timed,
            self.drawn,
            self.read,
            self.without.view,
            self.without.cascades,
            self.without.forms,
        )
    }
}

/// Which of the foliage's draws a measure leaves out of the frame, each
/// priced by the time the frame saves without it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Without {
    pub view: bool,
    pub cascades: [bool; CASCADES],
    /// The forms of the view's foliage left out, near to far.
    pub forms: [bool; FOLIAGE_FORMS],
}

/// The fragments of the foliage in one pass: what a weak GPU pays for, in
/// numbers no GPU changes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fragments {
    /// The pass's pixels: the view's as drawn, or a cascade's texels.
    pub pixels: u64,
    /// Every fragment the foliage's triangles cover: those a GPU shades
    /// where nothing drawn before them hides them.
    pub rasterized: u64,
    /// Those its cut keeps: what is left of each card once its clump, and
    /// its share of a crossing, are cut out of it.
    pub kept: u64,
    /// Of `rasterized`, those before what was drawn before the foliage, the
    /// view's ground: what an early depth test leaves to shade when no
    /// leaf hides another. `None` in a cascade.
    pub before: Option<u64>,
    /// The pixels where the foliage is what is seen: what a pass that shades
    /// each pixel once would shade.
    pub covered: u64,
    /// Of `rasterized`, each form's, nearest first: the view's alone.
    pub forms: Option<[u64; FOLIAGE_FORMS]>,
}

/// The passes of a frame, as a measure names them.
pub(crate) const FRAME: &str = "frame";
pub(crate) const OUTPUT: &str = "output";
pub(crate) const VIEW: &str = "view";
pub(crate) const CASCADE: [&str; CASCADES] = ["cascade 0", "cascade 1", "cascade 2"];

/// A measure running on a renderer.
pub(crate) struct Measure {
    pub timer: Option<Timer>,
    pub clock: Option<fn() -> f64>,
    pub without: Without,
    /// Set for the frame that counts fragments.
    pub counter: Option<Counter>,
    /// Whether the next frame counts.
    pub count_next: bool,
    pub measured: Arc<Mutex<Measured>>,
}

impl Measure {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, clock: Option<fn() -> f64>) -> Measure {
        let timer = Timer::new(device, queue);
        Measure {
            measured: Arc::new(Mutex::new(Measured {
                timed: timer.is_some(),
                ..Measured::default()
            })),
            timer,
            clock,
            without: Without::default(),
            counter: None,
            count_next: false,
        }
    }

    /// Whether this frame draws the foliage in passes of its own, to count
    /// the fragments before the ground between them.
    pub fn apart(&self) -> bool {
        self.counter.as_ref().is_some_and(Counter::counting)
    }

    /// The timestamps a pass named `label` writes this frame, if any.
    pub fn writes(&mut self, label: &'static str) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.timer.as_mut()?.writes(label)
    }
}

/// The most passes a frame times.
const PASSES: u32 = 16;

/// Timestamps at the start and end of each pass, resolved and read back a
/// frame at a time: a frame times nothing while the last one is still
/// being read.
pub(crate) struct Timer {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    readback: wgpu::Buffer,
    /// Nanoseconds a tick.
    period_ns: f64,
    /// The passes this frame times, each a pair of queries in this order.
    labels: Vec<&'static str>,
    /// This frame, by the measure's count, and what it leaves out.
    frame: (u64, Without),
    /// Whether this frame times its passes.
    timing: bool,
    /// Whether the readback is in flight.
    busy: Arc<Mutex<bool>>,
}

impl Timer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Timer> {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return None;
        }
        let size = u64::from(PASSES) * 2 * size_of::<u64>() as u64;
        Some(Timer {
            set: device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("measure"),
                ty: wgpu::QueryType::Timestamp,
                count: PASSES * 2,
            }),
            resolve: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("measure resolve"),
                size,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }),
            readback: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("measure readback"),
                size,
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            period_ns: f64::from(queue.get_timestamp_period()),
            labels: Vec::new(),
            frame: (0, Without::default()),
            timing: false,
            busy: Arc::new(Mutex::new(false)),
        })
    }

    /// Starts a frame: the `drawn`th, leaving out what `without` says.
    pub fn begin(&mut self, drawn: u64, without: Without) {
        self.labels.clear();
        self.frame = (drawn, without);
        self.timing = !*self.busy.lock().expect("unpoisoned");
    }

    fn writes(&mut self, label: &'static str) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        if !self.timing || self.labels.len() as u32 == PASSES {
            return None;
        }
        let at = self.labels.len() as u32 * 2;
        self.labels.push(label);
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &self.set,
            beginning_of_pass_write_index: Some(at),
            end_of_pass_write_index: Some(at + 1),
        })
    }

    /// Ends a frame: every timestamp resolved and copied to be read.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if !self.timing || self.labels.is_empty() {
            self.timing = false;
            return;
        }
        let queries = self.labels.len() as u32 * 2;
        encoder.resolve_query_set(&self.set, 0..queries, &self.resolve, 0);
        let bytes = u64::from(queries) * size_of::<u64>() as u64;
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, bytes);
    }

    /// Reads the frame just submitted back into `measured`, when the GPU is
    /// done with it.
    pub fn read(&mut self, measured: &Arc<Mutex<Measured>>) {
        if !self.timing {
            return;
        }
        *self.busy.lock().expect("unpoisoned") = true;
        let (busy, measured) = (self.busy.clone(), measured.clone());
        let (readback, labels, period_ns, (frame, without)) = (
            self.readback.clone(),
            self.labels.clone(),
            self.period_ns,
            self.frame,
        );
        let bytes = labels.len() as u64 * 2 * size_of::<u64>() as u64;
        self.readback
            .slice(..bytes)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_ok() {
                    let mapped = readback.slice(..bytes).get_mapped_range().expect("mapped");
                    let ticks: &[u64] = bytemuck::cast_slice(&mapped);
                    let ms = |ticks: u64| ticks as f64 * period_ns / 1e6;
                    let first = ticks.iter().step_by(2).min().copied().unwrap_or(0);
                    let last = ticks.iter().skip(1).step_by(2).max().copied().unwrap_or(0);
                    let passes = core::iter::once((FRAME, ms(last.saturating_sub(first))))
                        .chain(
                            labels
                                .iter()
                                .zip(ticks.chunks_exact(2))
                                .map(|(label, pair)| (*label, ms(pair[1].saturating_sub(pair[0])))),
                        )
                        .collect();
                    drop(mapped);
                    readback.unmap();
                    let mut measured = measured.lock().expect("unpoisoned");
                    measured.passes = passes;
                    measured.read = frame;
                    measured.without = without;
                }
                *busy.lock().expect("unpoisoned") = false;
            });
    }
}

/// The foliage's shader with the count's entry points, the view's or the
/// sun's as `casting` says: the same vertices as the draw it counts.
pub(crate) fn count_source(casting: bool) -> String {
    format!(
        "{}\n{}",
        foliage::source(casting),
        include_str!("shaders/foliage_count.wgsl")
    )
}

/// What a count adds one to each pixel of.
const COUNT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The counts of one pass: every fragment, those kept, those before the
/// ground (the view's alone), the pixels covered, and in view every
/// fragment of each form.
const VIEW_COUNTS: usize = 4 + FOLIAGE_FORMS;
const CAST_COUNTS: usize = 3;

/// Counts the foliage's fragments, a frame at a time, each count a pass of
/// its own into a texture of halves whose pixels add, read back and summed.
pub(crate) struct Counter {
    every: wgpu::RenderPipeline,
    kept: wgpu::RenderPipeline,
    before: wgpu::RenderPipeline,
    covered: wgpu::RenderPipeline,
    cast_every: wgpu::RenderPipeline,
    cast_kept: wgpu::RenderPipeline,
    cast_covered: wgpu::RenderPipeline,
    /// The view's counts, at its size, and each cascade's.
    view: Option<(
        [u32; 2],
        [wgpu::TextureView; VIEW_COUNTS],
        [wgpu::Texture; VIEW_COUNTS],
    )>,
    cascades: [[(wgpu::Texture, wgpu::TextureView); CAST_COUNTS]; CASCADES],
    /// Whether this frame counts.
    counting: bool,
}

impl Counter {
    pub fn new(
        device: &wgpu::Device,
        view_layout: &wgpu::BindGroupLayout,
        shadow_layout: &wgpu::BindGroupLayout,
    ) -> Counter {
        let make = |label: &str,
                    layout: &wgpu::BindGroupLayout,
                    casting: bool,
                    entry: &str,
                    depth: Option<wgpu::CompareFunction>| {
            let module = crate::shader(device, label, &count_source(casting));
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            let add = wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    buffers: &foliage::buffers(),
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COUNT_FORMAT,
                        blend: Some(wgpu::BlendState {
                            color: add,
                            alpha: add,
                        }),
                        write_mask: wgpu::ColorWrites::RED,
                    })],
                    compilation_options: Default::default(),
                }),
                // As the pass counted draws it: a card of the view seen from
                // both sides, a caster's back faces culled.
                primitive: wgpu::PrimitiveState {
                    cull_mode: casting.then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: depth.map(|compare| wgpu::DepthStencilState {
                    format: crate::DEPTH_FORMAT,
                    depth_write_enabled: Some(false),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    // A caster's depth is biased: tested against what it
                    // wrote, it is biased alike.
                    bias: if casting {
                        shadow::BIAS
                    } else {
                        Default::default()
                    },
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::CompareFunction::{Equal, GreaterEqual};
        let cascade = || {
            let texture = count_texture(device, "count", [shadow::SIZE; 2]);
            let view = texture.create_view(&Default::default());
            (texture, view)
        };
        Counter {
            every: make("count: every", view_layout, false, "fs_every", None),
            kept: make("count: kept", view_layout, false, "fs_kept", None),
            before: make(
                "count: before",
                view_layout,
                false,
                "fs_every",
                Some(GreaterEqual),
            ),
            covered: make("count: covered", view_layout, false, "fs_kept", Some(Equal)),
            cast_every: make("count: casts", shadow_layout, true, "fs_every", None),
            cast_kept: make("count: casts kept", shadow_layout, true, "fs_kept", None),
            cast_covered: make(
                "count: casts covered",
                shadow_layout,
                true,
                "fs_kept",
                Some(Equal),
            ),
            view: None,
            cascades: core::array::from_fn(|_| core::array::from_fn(|_| cascade())),
            counting: false,
        }
    }

    pub fn counting(&self) -> bool {
        self.counting
    }

    /// Arms the next frame drawn at `size` to count.
    pub fn arm(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.view.as_ref().is_none_or(|(at, ..)| *at != size) {
            let textures: [wgpu::Texture; VIEW_COUNTS] =
                core::array::from_fn(|_| count_texture(device, "count", size));
            let views = textures
                .each_ref()
                .map(|t| t.create_view(&Default::default()));
            self.view = Some((size, views, textures));
        }
        self.counting = true;
    }

    /// Counts in view, before the foliage is drawn: the fragments before
    /// the ground, tested against the depth drawn so far.
    pub fn view_before(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        draw: &dyn Fn(&mut wgpu::RenderPass<'_>, &wgpu::RenderPipeline, Option<usize>),
    ) {
        let (_, views, _) = self.view.as_ref().expect("armed");
        count_pass(encoder, &views[2], Some(depth), |pass| {
            draw(pass, &self.before, None);
        });
    }

    /// Counts in view once the picture's depth is whole: every fragment,
    /// those kept, and the pixels covered.
    pub fn view_after(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
        draw: &dyn Fn(&mut wgpu::RenderPass<'_>, &wgpu::RenderPipeline, Option<usize>),
    ) {
        let (_, views, _) = self.view.as_ref().expect("armed");
        count_pass(encoder, &views[0], None, |pass| {
            draw(pass, &self.every, None)
        });
        count_pass(encoder, &views[1], None, |pass| {
            draw(pass, &self.kept, None)
        });
        count_pass(encoder, &views[3], Some(depth), |pass| {
            draw(pass, &self.covered, None);
        });
        for form in 0..FOLIAGE_FORMS {
            count_pass(encoder, &views[4 + form], None, |pass| {
                draw(pass, &self.every, Some(form));
            });
        }
    }

    /// Counts in a cascade once its map is whole.
    pub fn cascade(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        cascade: usize,
        depth: &wgpu::TextureView,
        draw: &dyn Fn(&mut wgpu::RenderPass<'_>, &wgpu::RenderPipeline),
    ) {
        let [every, kept, covered] = &self.cascades[cascade];
        count_pass(encoder, &every.1, None, |pass| draw(pass, &self.cast_every));
        count_pass(encoder, &kept.1, None, |pass| draw(pass, &self.cast_kept));
        count_pass(encoder, &covered.1, Some(depth), |pass| {
            draw(pass, &self.cast_covered);
        });
    }

    /// Reads this frame's counts back into `measured` once the frame is
    /// submitted, and disarms.
    pub fn read(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        measured: &Arc<Mutex<Measured>>,
    ) {
        if !self.counting {
            return;
        }
        self.counting = false;
        let (size, _, view) = self.view.as_ref().expect("armed");
        let mut textures: Vec<(&wgpu::Texture, [u32; 2])> =
            view.iter().map(|texture| (texture, *size)).collect();
        for cascade in &self.cascades {
            textures.extend(
                cascade
                    .iter()
                    .map(|(texture, _)| (texture, [shadow::SIZE; 2])),
            );
        }
        // One buffer holds every count, each at an offset a copy can start.
        let mut regions = Vec::new();
        let mut bytes = 0;
        for &(_, [width, height]) in &textures {
            let stride = (width * 2).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
            regions.push((bytes, stride, [width, height]));
            bytes += u64::from(stride) * u64::from(height);
        }
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("count readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("count readback"),
        });
        for (&(texture, _), &(offset, stride, [width, height])) in textures.iter().zip(&regions) {
            encoder.copy_texture_to_buffer(
                texture.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(height),
                    },
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }
        queue.submit([encoder.finish()]);
        let measured = measured.clone();
        let mapped = readback.clone();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_err() {
                    return;
                }
                let data = mapped.slice(..).get_mapped_range().expect("mapped");
                let sums: Vec<(u64, u64)> = regions
                    .iter()
                    .map(|&(offset, stride, [width, height])| {
                        let mut sum = 0u64;
                        for row in 0..height {
                            let at = (offset + u64::from(row * stride)) as usize;
                            let row: &[u8] = &data[at..at + width as usize * 2];
                            sum += row
                                .chunks_exact(2)
                                .map(|half| {
                                    half_to_f32(u16::from_le_bytes([half[0], half[1]])) as u64
                                })
                                .sum::<u64>();
                        }
                        (sum, u64::from(width) * u64::from(height))
                    })
                    .collect();
                drop(data);
                mapped.unmap();
                let view = Fragments {
                    pixels: sums[0].1,
                    rasterized: sums[0].0,
                    kept: sums[1].0,
                    before: Some(sums[2].0),
                    covered: sums[3].0,
                    forms: Some(core::array::from_fn(|form| sums[4 + form].0)),
                };
                let cascades: [Fragments; CASCADES] = core::array::from_fn(|cascade| {
                    let at = VIEW_COUNTS + cascade * CAST_COUNTS;
                    Fragments {
                        pixels: sums[at].1,
                        rasterized: sums[at].0,
                        kept: sums[at + 1].0,
                        before: None,
                        covered: sums[at + 2].0,
                        forms: None,
                    }
                });
                let [c0, c1, c2] = cascades;
                measured.lock().expect("unpoisoned").fragments = Some([view, c0, c1, c2]);
            });
    }
}

fn count_texture(device: &wgpu::Device, label: &str, [width, height]: [u32; 2]) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COUNT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

/// One count: cleared, then drawn into, tested against `depth` if any,
/// which it reads and never writes.
fn count_pass(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    depth: Option<&wgpu::TextureView>,
    draw: impl FnOnce(&mut wgpu::RenderPass<'_>),
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("count"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
            depth_slice: None,
        })],
        depth_stencil_attachment: depth.map(|view| wgpu::RenderPassDepthStencilAttachment {
            view,
            depth_ops: None,
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    draw(&mut pass);
}

/// A half's value: a count, a whole number up to 2048.
fn half_to_f32(bits: u16) -> f32 {
    let exponent = i32::from((bits >> 10) & 0x1f);
    let mantissa = f32::from(bits & 0x3ff);
    match exponent {
        0 => mantissa * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + mantissa / 1024.0) * 2f32.powi(exponent - 15),
    }
}

#[cfg(test)]
mod tests {
    use super::half_to_f32;

    #[test]
    fn a_half_reads_as_the_count_it_holds() {
        assert_eq!(half_to_f32(0), 0.0);
        assert_eq!(half_to_f32(0x3c00), 1.0);
        assert_eq!(half_to_f32(0x4000), 2.0);
        assert_eq!(half_to_f32(0x4b00), 14.0);
        assert_eq!(half_to_f32(0x6800), 2048.0);
    }
}
