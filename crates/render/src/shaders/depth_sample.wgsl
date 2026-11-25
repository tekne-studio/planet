// The depth at a pixel, its texel's centre sampled and never filtered: what
// a load reads (`compose::DepthRead::Sample`).
fn depth_at(depth: texture_2d<f32>, nearest: sampler, pixel: vec2<i32>) -> f32 {
    let uv = (vec2<f32>(pixel) + 0.5) / vec2<f32>(textureDimensions(depth));
    return textureSampleLevel(depth, nearest, uv, 0.0).r;
}
