//! A picture laid over a finished view ([`scene::Overlay`]): one draw
//! recorded in the output stage's own pass after its picture, its viewport
//! the overlay's rectangle, the picture's own texture, faded in by a blend
//! constant. It costs the pixels it covers and no pass of its own.

use scene::Overlay;

/// Fades the picture over what is there by the blend constant's alpha.
const FADE: wgpu::BlendComponent = wgpu::BlendComponent {
    src_factor: wgpu::BlendFactor::Constant,
    dst_factor: wgpu::BlendFactor::OneMinusConstant,
    operation: wgpu::BlendOperation::Add,
};

pub(crate) struct Overlays {
    pipeline: wgpu::RenderPipeline,
}

impl Overlays {
    pub(crate) fn new(
        device: &wgpu::Device,
        view_layout: &wgpu::BindGroupLayout,
        picture_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> Overlays {
        let module = crate::shader(device, "overlay", include_str!("shaders/overlay.wgsl"));
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay"),
            bind_group_layouts: &[Some(view_layout), Some(picture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
                        color: FADE,
                        alpha: FADE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Overlays { pipeline }
    }

    /// Lays `overlay` over the picture `pass` finished, the overlay's
    /// texture bound by `picture`.
    pub(crate) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        picture: &wgpu::BindGroup,
        overlay: &Overlay,
    ) {
        let [x, y, width, height] = overlay.rect;
        pass.set_viewport(x as f32, y as f32, width as f32, height as f32, 0.0, 1.0);
        pass.set_pipeline(&self.pipeline);
        let opacity = f64::from(overlay.opacity.min(1.0));
        pass.set_blend_constant(wgpu::Color {
            r: opacity,
            g: opacity,
            b: opacity,
            a: opacity,
        });
        pass.set_bind_group(1, picture, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Whether `overlay` is laid on a view `size` pixels: seen at all, and
/// wholly inside it.
pub(crate) fn lays(overlay: &Overlay, size: [u32; 2]) -> bool {
    let [x, y, width, height] = overlay.rect;
    overlay.opacity > 0.0
        && width > 0
        && height > 0
        && x.checked_add(width).is_some_and(|right| right <= size[0])
        && y.checked_add(height)
            .is_some_and(|bottom| bottom <= size[1])
}
