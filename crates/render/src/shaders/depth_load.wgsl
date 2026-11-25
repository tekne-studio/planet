// The depth at a pixel, loaded (`compose::DepthRead::Load`).
fn depth_at(depth: texture_2d<f32>, nearest: sampler, pixel: vec2<i32>) -> f32 {
    return textureLoad(depth, pixel, 0).r;
}
