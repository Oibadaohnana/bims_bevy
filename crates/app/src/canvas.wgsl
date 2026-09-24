// The world canvas's one material (`scene.rs`): egui's own fragment, so a
// picture that moved from egui's pass to Bevy's comes out the same, with a
// clip and a colour allowed past one.
//
// A vertex colour is what egui's vertices carry — premultiplied, sRGB
// encoded, interpolated as it is — with one difference: it is a float
// rather than a byte, so an **emissive** channel can go past 1.0. The sRGB
// curve carries on past white for it, so 1.5 is half again as bright as
// white in the encoding and about two and a half times in light, and it
// is that light, above 1.0 in the HDR target, that the bloom picks up and
// nothing else.
//
// A mesh with UVs (the fog, a picture) samples its texture the way egui
// samples one: linear out of the sRGB texture, back to the encoding,
// multiplied by the vertex colour there — "the only way to get text to
// look right", egui says — and linear again for the blend.

#import bevy_sprite::mesh2d_vertex_output::VertexOutput

// The canvas's rectangle on the window, in physical pixels: min x, min y,
// max x, max y — egui's scissor, rounded the way bevy_egui rounds it.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> clip: vec4<f32>;
#ifdef VERTEX_UVS
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var picture: texture_2d<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var picture_sampler: sampler;
#endif

// 0-1 linear from 0-1 sRGB gamma, and on past one for an emissive channel.
fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

// 0-1 sRGB gamma from 0-1 linear.
fn gamma_from_linear_rgb(rgb: vec3<f32>) -> vec3<f32> {
    let cutoff = rgb < vec3<f32>(0.0031308);
    let lower = rgb * vec3<f32>(12.92);
    let higher = vec3<f32>(1.055) * pow(rgb, vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055);
    return select(higher, lower, cutoff);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
#ifdef VERTEX_UVS
    // Sampled before the clip can discard: a sample wants uniform control.
    let texel = textureSample(picture, picture_sampler, in.uv);
    let texture_gamma = vec4<f32>(gamma_from_linear_rgb(texel.rgb), texel.a);
#else
    let texture_gamma = vec4<f32>(1.0);
#endif
    let p = in.position.xy;
    if p.x < clip.x || p.y < clip.y || p.x >= clip.z || p.y >= clip.w {
        discard;
    }
    let color_gamma = texture_gamma * in.color;
    return vec4<f32>(linear_from_gamma_rgb(color_gamma.rgb), color_gamma.a);
}
