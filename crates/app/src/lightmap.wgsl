// The crew's light map, worked out on the GPU (`lightmap.rs`, task 121).
//
// Three passes over what `bims::sight::LightInputs` hands over, each the
// CPU's arithmetic step for step, so the picture is the one
// `Sight::light_map_on_cpu` and `fogmap.rs` make, byte for byte:
//
// 1. `march`: a thread a ray of an eye that moved — `sight::march_rays`'s
//    walk from the pixel the eye is in, crossing to whichever pixel edge
//    comes next, the first crossings divided on the CPU so the walk is
//    additions and comparisons — marking every pixel it reaches that the
//    eye may add and that is lit or near enough to its body, into that
//    body's bits. A set of bits: the order the rays run in cannot matter.
// 2. `compose`: a thread a pixel — `Sight::compose`: the fog's, the dark's
//    and the lamplight's levels out of tables made on the CPU; the one fog
//    over every fogged pixel nobody sees (task 128).
// 3. `blur`: a thread a pixel — `fogmap::blurred`'s five-tap binomial each
//    way, in integers, and `fogmap::texel`'s colour out of a table — into
//    the rows the fog texture is copied from.

struct Params {
    width: u32,
    height: u32,
    columns: u32,
    views: u32,
    // Words of bits a body's view takes, and a body's near tiles.
    words: u32,
    tile_words: u32,
    eyes: u32,
    // The texture's rows, in texels (a row padded to 256 bytes).
    stride: u32,
    fog: u32,
    // Bit 0: the lamps' shadows softened (`BIMS_SHADOWS`); bit 1: the
    // corners shaded (`BIMS_AO`) — read by `lightsoften.wgsl` alone.
    flags: u32,
    pad1: u32,
    pad2: u32,
};

// One eye of a body whose view is marched this frame.
struct Eye {
    start: vec2<i32>,
    from_tile: vec2<i32>,
    beyond_tile: vec2<i32>,
    beyond_dir: vec2<i32>,
    // Bit 0: the eye's own tile stops a line of sight. Bit 1: a peek.
    flags: u32,
    // Which body's bits it marks.
    view: u32,
    far: f32,
    pad: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
// A byte a tile: 1 opaque, 2 fogged.
@group(0) @binding(1) var<storage, read> cells: array<u32>;
// A pixel: the light the eyes read in the low byte, the shown light above.
@group(0) @binding(2) var<storage, read> fields: array<u32>;
@group(0) @binding(3) var<storage, read_write> seen: array<atomic<u32>>;
// A pixel: the darkness in the low byte, the lamplight above.
@group(0) @binding(4) var<storage, read_write> map: array<u32>;
@group(0) @binding(5) var<storage, read_write> texels: array<u32>;
@group(0) @binding(6) var<storage, read> eyes: array<Eye>;
// Each marched eye's rays' first crossings, across and down.
@group(0) @binding(7) var<storage, read> starts: array<vec2<f32>>;
@group(0) @binding(8) var<storage, read> near: array<u32>;
// Each ray: its direction, and a pixel's step along it across and down.
@group(0) @binding(9) var<storage, read> rays: array<vec4<f32>>;
// `fogmap::texel` for every darkness and lamplight, a byte each.
@group(0) @binding(10) var<storage, read> colours: array<u32>;
// The composing's tables: the dark, the lamplight seen, the lamplight
// under the fog, 256 each.
@group(0) @binding(11) var<storage, read> tables: array<u32>;

const RAYS: u32 = 4096u;

@compute @workgroup_size(64)
fn march(@builtin(global_invocation_id) id: vec3<u32>) {
    let r = id.x;
    let e = id.y;
    if r >= RAYS || e >= params.eyes {
        return;
    }
    let eye = eyes[e];
    let ray = rays[r];
    let t0 = starts[e * RAYS + r];
    let w = i32(params.width);
    let h = i32(params.height);
    var x = eye.start.x;
    var y = eye.start.y;
    let step_x = select(-1, 1, ray.x > 0.0);
    let step_y = select(-1, 1, ray.y > 0.0);
    var t_x = t0.x;
    var t_y = t0.y;
    var t = 0.0;
    let from_opaque = (eye.flags & 1u) != 0u;
    let peek = (eye.flags & 2u) != 0u;
    let base = eye.view * params.words;
    let near_base = eye.view * params.tile_words;
    // A ray crosses no more pixels than the map is across and down.
    let steps = params.width + params.height + 4u;
    for (var n = 0u; n < steps; n++) {
        if x < 0 || y < 0 || x >= w || y >= h || t > eye.far {
            break;
        }
        let tile = vec2<i32>(x >> 3u, y >> 3u);
        let ti = u32(tile.y) * params.columns + u32(tile.x);
        let i = u32(y) * params.width + u32(x);
        let stop = (cells[ti] & 1u) != 0u
            && !(from_opaque && tile.x == eye.from_tile.x && tile.y == eye.from_tile.y);
        var admits = true;
        if peek {
            let d = (tile - eye.beyond_tile) * eye.beyond_dir;
            admits = d.x + d.y >= 1;
        }
        let close = ((near[near_base + ti / 32u] >> (ti % 32u)) & 1u) != 0u;
        if admits && ((fields[i] & 255u) > 0u || close) {
            let word = base + i / 32u;
            let bit = 1u << (i % 32u);
            if (atomicLoad(&seen[word]) & bit) == 0u {
                atomicOr(&seen[word], bit);
            }
        }
        if stop {
            break;
        }
        if t_x < t_y {
            t = t_x;
            x += step_x;
            t_x += ray.z;
        } else {
            t = t_y;
            y += step_y;
            t_y += ray.w;
        }
    }
}

@compute @workgroup_size(8, 8)
fn compose(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = id.x;
    let y = id.y;
    if x >= params.width || y >= params.height {
        return;
    }
    let i = y * params.width + x;
    let c = cells[(y >> 3u) * params.columns + (x >> 3u)];
    if (c & 2u) == 0u {
        map[i] = 0u;
        return;
    }
    let word = i / 32u;
    let bit = 1u << (i % 32u);
    var any = false;
    for (var v = 0u; v < params.views; v++) {
        if (atomicLoad(&seen[v * params.words + word]) & bit) != 0u {
            any = true;
            break;
        }
    }
    let light = (fields[i] >> 8u) & 255u;
    // Not seen and fogged: the one fog, whoever's the deck is.
    var alpha = params.fog;
    var glow = tables[512u + light];
    if any {
        alpha = tables[light];
        glow = tables[256u + light];
    }
    map[i] = alpha | (glow << 8u);
}

@compute @workgroup_size(8, 8)
fn blur(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = i32(id.x);
    let y = i32(id.y);
    let w = i32(params.width);
    let h = i32(params.height);
    if x >= w || y >= h {
        return;
    }
    // A `var`, since only a variable may be indexed by a value.
    var taps = array<u32, 5>(1u, 4u, 6u, 4u, 1u);
    var a = 0u;
    var g = 0u;
    for (var k = 0; k < 5; k++) {
        let row = clamp(y + k - 2, 0, h - 1);
        var across_a = 0u;
        var across_g = 0u;
        for (var j = 0; j < 5; j++) {
            let col = clamp(x + j - 2, 0, w - 1);
            let m = map[u32(row * w + col)];
            across_a += (m & 255u) * taps[j];
            across_g += ((m >> 8u) & 255u) * taps[j];
        }
        a += across_a * taps[k];
        g += across_g * taps[k];
    }
    texels[u32(y) * params.stride + u32(x)] = colours[(a / 256u) * 256u + g / 256u];
}
