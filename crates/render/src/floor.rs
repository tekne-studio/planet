//! The floor every shader is held to: WebGL2. Each module the renderer builds
//! is validated as the device validates it, with only what WebGL2 offers, and
//! every entry point is translated to GLSL ES 3.0 as wgpu's GL backend does.
//! Each entry point stays within WebGL2's vertex attributes and the
//! variables passed between stages, which no translation counts. A shader
//! the floor cannot run fails here with its name and naga's reason, instead
//! of in a browser before the first picture.

use naga::back::glsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{Binding, Handle, Module, Type, TypeInner};

use crate::compose::DepthRead;

/// The way the floor reads a depth: what GL is given.
fn floor_read() -> DepthRead {
    DepthRead::for_backend(wgpu::Backend::Gl)
}

/// Every module the renderer builds, by the label it is built with, from the
/// same assembly the renderer uses, reading the depth as `read` says.
fn modules(read: DepthRead) -> Vec<(&'static str, String)> {
    let shader = crate::with_prelude;
    let stage = |source: &str| shader(&crate::compose::stage_source(source, read));
    vec![
        ("sky", shader(include_str!("shaders/sky.wgsl"))),
        ("terrain", shader(&crate::terrain::ground_source())),
        ("grass", shader(include_str!("shaders/grass.wgsl"))),
        ("foliage", shader(&crate::foliage::source(false))),
        ("foliage casting", shader(&crate::foliage::source(true))),
        (
            "foliage count",
            shader(&crate::measure::count_source(false)),
        ),
        (
            "foliage count casting",
            shader(&crate::measure::count_source(true)),
        ),
        ("water", shader(&crate::terrain::water_source(read))),
        ("boxes", shader(include_str!("shaders/boxes.wgsl"))),
        ("pane", shader(include_str!("shaders/pane.wgsl"))),
        ("picture", shader(include_str!("shaders/picture.wgsl"))),
        ("skinned", shader(include_str!("shaders/skinned.wgsl"))),
        ("volume", shader(&crate::volumes::source())),
        ("guide", shader(include_str!("shaders/guide.wgsl"))),
        ("model", shader(&crate::models::source())),
        ("textured", shader(include_str!("shaders/textured.wgsl"))),
        (
            "cloud noise",
            shader(include_str!("shaders/cloud_noise.wgsl")),
        ),
        ("clouds", stage(include_str!("shaders/clouds.wgsl"))),
        ("bloom", stage(include_str!("shaders/bloom.wgsl"))),
        ("warp", stage(include_str!("shaders/warp.wgsl"))),
        ("output", stage(include_str!("shaders/output.wgsl"))),
        ("overlay", shader(include_str!("shaders/overlay.wgsl"))),
        ("mip", include_str!("shaders/mip.wgsl").to_owned()),
    ]
}

/// What WebGL2 lets a shader use: no optional feature, no downlevel flag
/// (none of those naga asks about, cube arrays, sample shading, f16 in f32,
/// is in WebGL2), as wgpu derives it for a device.
fn webgl2() -> Capabilities {
    wgpu_naga_bridge::features_to_naga_capabilities(
        wgpu::Features::empty(),
        wgpu::DownlevelFlags::empty(),
    )
}

/// Why `source` cannot run on the floor, one line an entry point, or none.
fn floor_errors(source: &str) -> Vec<String> {
    let module = match naga::front::wgsl::parse_str(source) {
        Ok(module) => module,
        Err(error) => return vec![format!("parse: {}", error.emit_to_string(source))],
    };
    let info = match Validator::new(ValidationFlags::all(), webgl2()).validate(&module) {
        Ok(info) => info,
        Err(error) => return vec![format!("validate: {}", error.emit_to_string(source))],
    };
    let options = glsl::Options {
        version: glsl::Version::Embedded {
            version: 300,
            is_webgl: true,
        },
        writer_flags: glsl::WriterFlags::ADJUST_COORDINATE_SPACE
            | glsl::WriterFlags::FORCE_POINT_SIZE,
        binding_map: Default::default(),
        zero_initialize_workgroup_memory: true,
    };
    // As the GL backend checks on GLES: nothing, the language's own aside.
    let unchecked = naga::proc::BoundsCheckPolicy::Unchecked;
    let policies = naga::proc::BoundsCheckPolicies {
        index: unchecked,
        buffer: unchecked,
        image_load: unchecked,
        binding_array: unchecked,
    };
    let mut errors = over_limits(&module);
    for entry in &module.entry_points {
        let pipeline = glsl::PipelineOptions {
            shader_stage: entry.stage,
            entry_point: entry.name.clone(),
            multiview: None,
        };
        let mut out = String::new();
        let written = glsl::Writer::new(&mut out, &module, &info, &options, &pipeline, policies)
            .and_then(|mut writer| writer.write());
        if let Err(error) = written {
            errors.push(format!("{}: {error}", entry.name));
        }
    }
    errors
}

/// The entry points that pass WebGL2 more vertex attributes, or more
/// variables from one stage to the next, than it holds, one line each.
fn over_limits(module: &Module) -> Vec<String> {
    let limits = wgpu::Limits::downlevel_webgl2_defaults();
    let mut errors = Vec::new();
    for entry in &module.entry_points {
        let function = &entry.function;
        let inputs: u32 = function
            .arguments
            .iter()
            .map(|argument| locations(module, argument.ty, argument.binding.as_ref()))
            .sum();
        let outputs = function.result.as_ref().map_or(0, |result| {
            locations(module, result.ty, result.binding.as_ref())
        });
        let mut over = |what: &str, count: u32, limit: u32| {
            if count > limit {
                errors.push(format!(
                    "{}: {count} {what}, WebGL2 holds {limit}",
                    entry.name
                ));
            }
        };
        let between = limits.max_inter_stage_shader_variables;
        match entry.stage {
            naga::ShaderStage::Vertex => {
                over("vertex attributes", inputs, limits.max_vertex_attributes);
                over("variables out", outputs, between);
            }
            naga::ShaderStage::Fragment => over("variables in", inputs, between),
            _ => {}
        }
    }
    errors
}

/// How many `@location`s a value of type `ty` passes, bound as `binding`:
/// one, or one a member of a struct of them.
fn locations(module: &Module, ty: Handle<Type>, binding: Option<&Binding>) -> u32 {
    let located = |binding: Option<&Binding>| matches!(binding, Some(Binding::Location { .. }));
    match (binding, &module.types[ty].inner) {
        (Some(binding), _) => u32::from(located(Some(binding))),
        (None, TypeInner::Struct { members, .. }) => members
            .iter()
            .filter(|member| located(member.binding.as_ref()))
            .count() as u32,
        (None, _) => 0,
    }
}

/// The floor counts what it holds: a module over WebGL2's limits fails.
#[test]
fn the_floor_counts_attributes_and_variables() {
    let module = |count: usize| {
        let fields: String = (0..count)
            .map(|i| format!("@location({i}) v{i}: vec4<f32>,"))
            .collect();
        format!(
            "struct In {{ {fields} }}
            struct Out {{ @builtin(position) at: vec4<f32>, {fields} }}
            @vertex fn main(input: In) -> Out {{ var out: Out; out.at = input.v0; return out; }}"
        )
    };
    assert!(
        floor_errors(&module(15)).is_empty(),
        "{:?}",
        floor_errors(&module(15))
    );
    let over = floor_errors(&module(17));
    assert!(
        over.iter()
            .any(|error| error.contains("17 vertex attributes"))
            && over.iter().any(|error| error.contains("17 variables out")),
        "{over:?}"
    );
}

#[test]
fn every_shader_runs_on_webgl2() {
    let failures: Vec<String> = modules(floor_read())
        .iter()
        .flat_map(|(label, source)| {
            floor_errors(source)
                .into_iter()
                .map(move |error| format!("{label}: {error}"))
        })
        .collect();
    assert!(
        failures.is_empty(),
        "shaders WebGL2 cannot run:\n{}",
        failures.join("\n")
    );
}

/// A load reads a depth texture as zero on GL, which no translation shows:
/// the floor samples it.
#[test]
fn the_floor_samples_the_depth() {
    assert_eq!(floor_read(), DepthRead::Sample);
}

/// A new file under `shaders/` is reached by a module above, or the floor
/// would not see it.
#[test]
fn every_shader_file_is_held() {
    let modules = [modules(DepthRead::Load), modules(DepthRead::Sample)].concat();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shaders");
    for file in std::fs::read_dir(dir).expect("shaders folder") {
        let path = file.expect("shader file").path();
        let text = std::fs::read_to_string(&path).expect("shader text");
        assert!(
            modules.iter().any(|(_, source)| source.contains(&text)),
            "{} is in no module the floor checks: add it to `modules`",
            path.display()
        );
    }
}

/// The view's uniform is laid out as the shaders read it: the struct the
/// CPU writes takes the bytes WGSL's `View` spans, lamps and their aims
/// included.
#[test]
fn the_view_uniform_is_as_long_as_the_shaders_read_it() {
    let source = crate::with_prelude(include_str!("shaders/boxes.wgsl"));
    let module = naga::front::wgsl::parse_str(&source).expect("boxes parses");
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(module.to_ctx()).expect("the types lay out");
    let (view, _) = module
        .types
        .iter()
        .find(|(_, ty)| ty.name.as_deref() == Some("View"))
        .expect("a View struct");
    assert_eq!(
        layouter[view].size as usize,
        core::mem::size_of::<crate::ViewUniform>()
    );
}
