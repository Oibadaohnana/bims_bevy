// The fight's particles, simulated on the GPU (`particles.rs`).
//
// One storage buffer: a header — the particle clock, the view's scale and
// the camera's origin on the window in points — then a ring of spray
// records (`bims::fx::Spray`, packed by `particles::record`). One mesh of
// quads, SPRAY_MOST a record, whose third coordinate is the quad's number.
// The vertex stage works out the particle that quad is from its record and
// the clock alone — where it has flown, how big, how hot, how faded — and
// lays the quad round it as a capsule from its tail to its head; the
// fragment stage feathers the capsule. Nothing is stepped and nothing is
// kept between frames: the clock moving is the particles moving.
//
// Every particle's own numbers are a hash of the spray's seed, the
// particle and what is asked — never a roll — so a spray looks the same
// every frame it is drawn.

#import bevy_sprite::mesh2d_functions as mesh_functions

const TAU: f32 = 6.28318530717958647692;
const PI: f32 = 3.14159265358979323846;
// Particles a record: `bims::fx::SPRAY_MOST`.
const MOST: u32 = 16u;

// Sixteen floats, as `particles::record` lays them out:
// head  = kind, where x, where y, born (on the clock)
// way   = to x, to y, reach, life
// color = straight colour, sRGB-encoded, past one to glow; alpha
// more  = count, seed, spread, dot
struct Spray {
    head: vec4<f32>,
    way: vec4<f32>,
    color: vec4<f32>,
    more: vec4<f32>,
};

struct Sprays {
    // The clock, the view's scale, the camera's origin x and y.
    view: vec4<f32>,
    unused: vec4<f32>,
    items: array<Spray>,
};

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> clip: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<storage, read> sprays: Sprays;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // x and y: the quad's corner, -1 or 1 each way; z: the quad.
    @location(0) position: vec3<f32>,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    // Where the fragment is, in points on the window.
    @location(0) at: vec2<f32>,
    // The capsule: its tail and head in points, its radius.
    @location(1) @interpolate(flat) tail: vec2<f32>,
    @location(2) @interpolate(flat) tip: vec2<f32>,
    @location(3) @interpolate(flat) radius: f32,
    // Premultiplied linear colour, the alpha nought for a light added to
    // the picture.
    @location(4) @interpolate(flat) color: vec4<f32>,
};

fn pcg(v: u32) -> u32 {
    let s = v * 747796405u + 2891336453u;
    let w = ((s >> ((s >> 28u) + 4u)) ^ s) * 277803737u;
    return (w >> 22u) ^ w;
}

// A number in [0, 1) for particle `p` of the spray seeded `seed`, the
// `k`th thing asked of it.
fn rnd(seed: u32, p: u32, k: u32) -> f32 {
    let h = pcg(seed * 0x9E3779B9u ^ pcg(p * 64u + k));
    return f32(h >> 8u) / 16777216.0;
}

fn turn(a: f32) -> vec2<f32> {
    return vec2<f32>(cos(a), sin(a));
}

// Out fast and settling, nought to one.
fn ease(u: f32) -> f32 {
    let v = 1.0 - clamp(u, 0.0, 1.0);
    return 1.0 - v * v;
}

fn ease3(u: f32) -> f32 {
    let v = 1.0 - clamp(u, 0.0, 1.0);
    return 1.0 - v * v * v;
}

// 0-1 linear from 0-1 sRGB gamma, and on past one for a glowing channel:
// `shape.wgsl`'s, the same arithmetic.
fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

fn nothing() -> Varyings {
    var out: Varyings;
    out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    out.at = vec2<f32>(0.0);
    out.tail = vec2<f32>(0.0);
    out.tip = vec2<f32>(0.0);
    out.radius = 0.0;
    out.color = vec4<f32>(0.0);
    return out;
}

// A point of a lightning chain: chain `c` of the spray, vertex `k` of
// five, jumping to a new shape every fifteenth of a second (`flick`).
fn bolt_point(seed: u32, c: u32, k: u32, flick: u32, at: vec2<f32>, reach: f32) -> vec2<f32> {
    let salt = 40u + flick * 8u;
    let way = rnd(seed, c, salt) * TAU;
    let out = reach * (0.35 + 0.65 * rnd(seed, c, salt + 1u));
    let along = f32(k) / 4.0;
    let base = at + turn(way) * (out * along);
    if k == 0u {
        return base;
    }
    let jag = (rnd(seed, c * 8u + k, salt + 2u) - 0.5) * reach * 0.28;
    return base + turn(way + PI * 0.5) * jag;
}

@vertex
fn vertex(v: Vertex) -> Varyings {
    let q = u32(v.position.z);
    let slot = q / MOST;
    let p = q % MOST;
    let s = sprays.items[slot];
    let count = u32(s.more.x + 0.5);
    let life = s.way.w;
    if p >= count || life <= 0.0 {
        return nothing();
    }
    let kind = u32(s.head.x + 0.5);
    let seed = u32(s.more.y + 0.5);
    let at = s.head.yz;
    let to = s.way.xy;
    let reach = s.way.z;
    let spread = s.more.z;
    let grain = s.more.w;
    let clock = sprays.view.x;
    var age = clock - s.head.w;
    // Motes and swirls come out over the first part of their spray's life
    // rather than all at once.
    if kind == 6u || kind == 7u {
        age = age - rnd(seed, p, 9u) * life * 0.35;
    }
    let lived = life * (0.55 + 0.45 * rnd(seed, p, 0u));
    let u = age / lived;
    if u < 0.0 || u >= 1.0 {
        return nothing();
    }
    var way = to - at;
    if dot(way, way) < 1e-8 {
        way = vec2<f32>(0.0, -1.0);
    }
    way = normalize(way);
    let up = vec2<f32>(0.0, -1.0);

    // Head and tail in camera units, radius, colour and alpha, and
    // whether it is laid over the picture (smoke) rather than added.
    var head = at;
    var tail = at;
    var size = grain * 0.5;
    var rgb = s.color.rgb;
    var alpha = s.color.a;
    var over = false;
    switch kind {
        // Sparks: thrown along the way within the spread, slowing,
        // streaked along their flight, cooling as they go.
        case 0u: {
            let a = atan2(way.y, way.x) + (rnd(seed, p, 1u) * 2.0 - 1.0) * spread;
            let d = turn(a);
            let out = reach * (0.4 + 0.6 * rnd(seed, p, 2u));
            head = at + d * (out * ease(u));
            tail = head - d * (out * 0.3 * (1.0 - u) + grain * 0.5);
            size = grain * 0.5 * (1.0 - 0.4 * u);
            rgb = rgb * (1.0 - 0.45 * u);
            alpha = alpha * (1.0 - u);
        }
        // A wake: motes along the line, drifting off it and fading.
        case 1u: {
            let t = rnd(seed, p, 1u);
            let along = at + (to - at) * t;
            let across = vec2<f32>(-way.y, way.x) * ((rnd(seed, p, 2u) - 0.5) * grain * 2.0);
            let drift = turn(rnd(seed, p, 3u) * TAU) * (reach * u * (0.4 + 0.6 * rnd(seed, p, 4u)));
            head = along + across + drift;
            tail = head;
            size = grain * 0.5 * (1.0 - 0.6 * u);
            let left = 1.0 - u;
            alpha = alpha * left * sqrt(left);
        }
        // Embers: flung every way, slowing hard, drifting up a little,
        // cooling to a dull red.
        case 2u: {
            let d = turn(rnd(seed, p, 1u) * TAU);
            let out = reach * (0.25 + 0.75 * sqrt(rnd(seed, p, 2u)));
            head = at + d * (out * ease3(u)) + up * (reach * 0.12 * u);
            tail = head - d * (out * 0.1 * (1.0 - u));
            size = grain * 0.5 * (0.6 + 0.6 * rnd(seed, p, 3u)) * (1.0 - 0.4 * u);
            let cool = rgb * vec3<f32>(0.7, 0.22, 0.08);
            rgb = mix(rgb, cool, smoothstep(0.0, 0.8, u));
            alpha = alpha * (1.0 - u * u);
        }
        // Smoke: puffs pushed out and rising, swelling and thinning,
        // laid over the picture.
        case 3u: {
            let d = turn(rnd(seed, p, 1u) * TAU);
            let out = reach * (0.2 + 0.8 * rnd(seed, p, 2u));
            head = at + d * (out * ease(u)) + up * (reach * 0.35 * u);
            tail = head;
            size = grain * 0.5 * (0.6 + 0.5 * rnd(seed, p, 3u)) * (1.0 + 1.6 * u);
            let left = 1.0 - u;
            alpha = alpha * left * sqrt(left) * min(u * 8.0, 1.0);
            over = true;
        }
        // Lightning: chains of four jagged strokes from the middle out,
        // jumping to a new shape every fifteenth of a second, flickering.
        case 4u: {
            let flick = u32(max(age, 0.0) * 15.0);
            let c = p / 4u;
            let k = p % 4u;
            tail = bolt_point(seed, c, k, flick, at, reach);
            head = bolt_point(seed, c, k + 1u, flick, at, reach);
            size = grain * 0.5;
            let lit = select(0.25, 1.0, rnd(seed, c, 100u + flick) > 0.3);
            alpha = alpha * (1.0 - u) * lit;
        }
        // A ring running out: dashes at even angles, reaching `reach`.
        case 5u: {
            let a = (f32(p) + rnd(seed, p, 1u) * 0.6) / f32(count) * TAU + rnd(seed, 0u, 2u) * TAU;
            let d = turn(a);
            let out = reach * ease(u);
            head = at + d * out;
            tail = head - d * (reach * 0.1 * (1.0 - u) + grain * 0.3);
            size = grain * 0.5 * (1.0 - 0.5 * u);
            alpha = alpha * pow(1.0 - u, 1.3);
        }
        // Motes: out of the disc, rising and swaying, twinkling out.
        case 6u: {
            let d = turn(rnd(seed, p, 1u) * TAU);
            let start = at + d * (reach * sqrt(rnd(seed, p, 2u)));
            let rise = (reach * 0.2 + 18.0) * u * (0.5 + 0.5 * rnd(seed, p, 3u));
            let sway = sin(u * 6.0 + rnd(seed, p, 4u) * TAU) * grain * 1.5;
            head = start + up * rise + vec2<f32>(sway, 0.0);
            tail = head;
            size = grain * 0.5 * (0.6 + 0.6 * rnd(seed, p, 5u));
            let twinkle = 0.65 + 0.35 * sin(age * 18.0 + rnd(seed, p, 6u) * TAU);
            alpha = alpha * sin(PI * u) * twinkle;
        }
        // A swirl: in from the circle of `reach` to the middle, turning.
        case 7u: {
            let a = rnd(seed, p, 1u) * TAU + u * 2.6;
            let r = reach * (1.0 - ease(u)) * (0.7 + 0.3 * rnd(seed, p, 2u));
            head = at + turn(a) * r;
            tail = head - turn(a + PI * 0.5) * (grain * 1.2 * (1.0 - u));
            size = grain * 0.5;
            alpha = alpha * sin(PI * u);
        }
        default: {
            return nothing();
        }
    }
    if alpha <= 0.002 {
        return nothing();
    }

    // Onto the window: points from the camera's units.
    let scale = sprays.view.y;
    let origin = sprays.view.zw;
    let h = origin + head * scale;
    let t = origin + tail * scale;
    var r = size * scale;
    // Thinner than a pixel: drawn a pixel wide and that much fainter.
    if r < 0.6 {
        alpha = alpha * (r / 0.6);
        r = 0.6;
    }
    let axis = h - t;
    let len = length(axis);
    var along = vec2<f32>(1.0, 0.0);
    if len > 1e-4 {
        along = axis / len;
    }
    let across = vec2<f32>(-along.y, along.x);
    let middle = (h + t) * 0.5;
    let pad = r + 1.0;
    let corner = middle + along * (v.position.x * (len * 0.5 + pad)) + across * (v.position.y * pad);

    var out: Varyings;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    out.position = mesh_functions::mesh2d_position_local_to_clip(
        world_from_local,
        vec4<f32>(corner, 0.0, 1.0),
    );
    out.at = corner;
    out.tail = t;
    out.tip = h;
    out.radius = r;
    let lin = linear_from_gamma_rgb(max(rgb, vec3<f32>(0.0))) * alpha;
    out.color = vec4<f32>(lin, select(0.0, alpha, over));
    return out;
}

@fragment
fn fragment(in: Varyings) -> @location(0) vec4<f32> {
    let px = in.position.xy;
    if px.x < clip.x || px.y < clip.y || px.x >= clip.z || px.y >= clip.w {
        discard;
    }
    // How far from the capsule's spine, as a share of its radius: solid in
    // the middle, feathered out to the edge.
    let seg = in.tip - in.tail;
    let l2 = dot(seg, seg);
    var k = 0.0;
    if l2 > 1e-6 {
        k = clamp(dot(in.at - in.tail, seg) / l2, 0.0, 1.0);
    }
    let d = length(in.at - (in.tail + seg * k)) / max(in.radius, 1e-3);
    if d >= 1.0 {
        discard;
    }
    let f = 1.0 - d;
    let cover = f * f * (3.0 - 2.0 * f);
    return in.color * cover;
}
