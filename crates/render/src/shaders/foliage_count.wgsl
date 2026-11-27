// The foliage's fragments counted, for a measure, after foliage.wgsl: each
// adds one to its pixel of a count, every fragment its triangles cover or
// only those its cut keeps, the view's or the sun's as `CASTING` says.

@fragment
fn fs_every(in: Varying) -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 0.0);
}

@fragment
fn fs_kept(in: Varying) -> @location(0) vec4<f32> {
    if (CASTING && !casts(in)) || (!CASTING && cut(in)) {
        discard;
    }
    return vec4<f32>(1.0, 0.0, 0.0, 0.0);
}
