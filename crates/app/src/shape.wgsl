// The world canvas's shapes, drawn on the GPU (`scene.rs`, task 121).
//
// A layer is one storage buffer of records (`shapes::Record`, packed by
// `shapes::pack`) and one mesh of quads, a quad a record, whose vertices say
// which corner they are and which record they belong to. The vertex stage
// puts the quad round its shape; the fragment stage works out how much of
// the pixel the triangles `shapes::ShapeBuf` would have tessellated cover,
// and draws the shape's colour that much — premultiplied and sRGB-encoded
// as a vertex colour was, carried to linear the way `canvas.wgsl` carries
// one, so an emissive channel past one is still past one for the bloom.
// The coverage multiplies the colour once it is linear (where egui, and
// `BIMS_SHAPES=cpu`, multiply it before), so an edge blends evenly and two
// shapes of one colour that meet show no seam.
//
// Coverage: every polygon the painters' shapes come to is convex, and
// `ShapeBuf` feathers it by moving its ring out and in half a pixel along
// the mitres and ramping the colour across the band between. Inside that
// band the colour is linear in the distance to the edge's line, and the
// bands meet on the mitres, where two edges' lines are equally far — so a
// pixel gets `0.5 - D / f`, clamped, `D` being the largest signed distance
// to any edge's line of the polygon `ShapeBuf` builds (its own points: the
// chords of a rounded corner, the sides of a many-sided ellipse), `f` the
// feather. A stroke is two of those, one each way.

#import bevy_sprite::mesh2d_functions as mesh_functions

const TAU: f32 = 6.28318530717958647692;
const HALF_PI: f32 = 1.57079632679489661923;

// Sixteen floats, as `shapes::Record` lays them out:
// head  = kind, centre x, centre y, the feather's width
// body  = half width, half height, sin and cos of the turn
// more  = corner radius, line width, points on a curve, unused — for a
//         surface (kinds 7 and 8), its anchor in repeats, the points a
//         repeat spans and the surface (`shapes::surface_record`)
// color = premultiplied, sRGB-encoded, a channel past one if emissive
struct Shape {
    head: vec4<f32>,
    body: vec4<f32>,
    more: vec4<f32>,
    color: vec4<f32>,
};

struct Shapes {
    // How many of `items` are this frame's; the rest of the quads are
    // folded to nothing.
    count: vec4<u32>,
    items: array<Shape>,
};

// The canvas's rectangle on the window, in physical pixels — egui's
// scissor, as `canvas.wgsl` has it.
@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> clip: vec4<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(1) var<storage, read> shapes: Shapes;
// The surfaces (`surfaces.rs`): a layer each, the same size, repeating, with
// their mipmaps; a texel is the surface over its average, a quarter scale.
@group(#{MATERIAL_BIND_GROUP}) @binding(2) var surfaces: texture_2d_array<f32>;
@group(#{MATERIAL_BIND_GROUP}) @binding(3) var surfaces_sampler: sampler;

// What a surface texel is multiplied by: its average is a quarter, so a
// shape of a surface is its colour on average and up to four times it.
const SURFACE_GAIN: f32 = 4.0;
// A surface tied to the world has this added to its number, and its
// repeat broken by a second sample this much smaller in texture space
// (larger on the ground) and turned this way (cos, sin of 1.1 radians).
const TIED: i32 = 64;
const BREAK_SCALE: f32 = 0.61;
const BREAK_TURN: vec2<f32> = vec2<f32>(0.4536, 0.8912);

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    // x and y: the quad's corner, -1 or 1 each way; z: the record.
    @location(0) position: vec3<f32>,
};

struct Varyings {
    @builtin(position) position: vec4<f32>,
    // Where the fragment is in the shape's own frame, in points: from its
    // centre, before the turn.
    @location(0) local: vec2<f32>,
    @location(1) @interpolate(flat) index: u32,
};

@vertex
fn vertex(v: Vertex) -> Varyings {
    var out: Varyings;
    let i = u32(v.position.z);
    out.index = i;
    out.local = vec2<f32>(0.0);
    if i >= shapes.count.x {
        // Past this frame's records: every corner on one point, so the
        // quad has no area and nothing is drawn.
        out.position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
        return out;
    }
    let s = shapes.items[i];
    let half = s.body.xy;
    // A surface's `more` is its anchor and its span, not a corner and a line.
    var line = s.more.y;
    if s.head.x >= 6.5 {
        line = 0.0;
    }
    let f = s.head.w;
    // As far as anything of the shape reaches: its ring out by half the
    // line and the feather beyond — a triangle's sharp corners further,
    // since a mitre there reaches out by up to twice what it moves an edge.
    var reach = half + vec2<f32>(line * 0.5 + f + 1.0);
    if s.head.x >= 4.5 {
        reach = half + vec2<f32>(line + 2.0 * f + 1.0);
    }
    let local = v.position.xy * reach;
    let sc = s.body.zw;
    let turned = vec2<f32>(local.x * sc.y - local.y * sc.x, local.x * sc.x + local.y * sc.y);
    let at = s.head.yz + turned;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    out.position = mesh_functions::mesh2d_position_local_to_clip(
        world_from_local,
        vec4<f32>(at, 0.0, 1.0),
    );
    out.local = local;
    return out;
}

// How much of a pixel `d` from a ring is inside it, over a ramp `f` wide
// centred on the ring. With no ramp at all (a shape with no width to
// feather), the pixel's centre decides.
fn ramp(d: f32, f: f32) -> f32 {
    if f <= 0.0 {
        return select(0.0, 1.0, d < 0.0);
    }
    return clamp(0.5 - d / f, 0.0, 1.0);
}

// The rectangle `shapes::rounded` rings: square, or with `corners` points
// on each quarter circle of its corners. It is the same mirrored every way,
// so the point is folded into the quadrant of positive x and y and only
// that quadrant's two sides and its corner's chords are asked. A side with
// no length — a pill's — is not an edge `ShapeBuf` feathers, and neither is
// a chord of a corner whose radius has gone to nought.
fn rounded_d(p: vec2<f32>, h: vec2<f32>, radius: f32, corners: u32) -> f32 {
    let r = max(min(min(radius, h.x), h.y), 0.0);
    let q = abs(p);
    var d = -1.0e30;
    let arcs = corners > 1u && r > 0.0;
    if !arcs || h.y - r > 5.0e-5 {
        d = max(d, q.x - h.x);
    }
    if !arcs || h.x - r > 5.0e-5 {
        d = max(d, q.y - h.y);
    }
    if arcs {
        let step = HALF_PI / f32(corners - 1u);
        let c0 = h - vec2<f32>(r);
        let reach = r * cos(step * 0.5);
        for (var k = 0u; k < corners - 1u; k++) {
            let m = (f32(k) + 0.5) * step;
            d = max(d, dot(q - c0, vec2<f32>(cos(m), sin(m))) - reach);
        }
    }
    return d;
}

// The ellipse `shapes::ring` makes: `n` points at even angles on it, from
// nought, so its sides are those of a regular polygon stretched to `r`.
fn ngon_d(p: vec2<f32>, r: vec2<f32>, n: u32) -> f32 {
    if min(r.x, r.y) <= 0.0 {
        return 1.0e30;
    }
    let step = TAU / f32(n);
    let reach = r.x * r.y * cos(step * 0.5);
    let turn = vec2<f32>(cos(step), sin(step));
    // Each side's normal is (r.y cos m, r.x sin m), m the angle half way
    // along it, turned a step a side.
    var dir = vec2<f32>(cos(step * 0.5), sin(step * 0.5));
    var d = -1.0e30;
    for (var i = 0u; i < n; i++) {
        let nrm = vec2<f32>(r.y * dir.x, r.x * dir.y);
        d = max(d, (dot(nrm, p) - reach) / length(nrm));
        dir = vec2<f32>(dir.x * turn.x - dir.y * turn.y, dir.x * turn.y + dir.y * turn.x);
    }
    return d;
}

fn perp(a: vec2<f32>, b: vec2<f32>) -> f32 {
    return a.x * b.y - a.y * b.x;
}

// `shapes::edge_normal`, outward for a polygon wound `sign`'s way.
fn edge_normal(e: vec2<f32>, sign: f32) -> vec2<f32> {
    let l = length(e);
    if l == 0.0 {
        return vec2<f32>(0.0);
    }
    return vec2<f32>(e.y, -e.x) / l * sign;
}

fn winding(a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> f32 {
    return select(-1.0, 1.0, perp(a, b) + perp(b, c) + perp(c, a) >= 0.0);
}

// `shapes::normals` at one corner: the mitre, capped at twice its length.
fn mitre(prev: vec2<f32>, here: vec2<f32>, next: vec2<f32>, sign: f32) -> vec2<f32> {
    let n0 = edge_normal(here - prev, sign);
    let n1 = edge_normal(next - here, sign);
    let m = (n0 + n1) / 2.0;
    let l2 = dot(m, m);
    if l2 < 1.0e-6 {
        return n0;
    }
    let mm = m / l2;
    if dot(mm, mm) > 4.0 {
        return normalize(mm) * 2.0;
    }
    return mm;
}

// A triangle's corners, each moved `by` along its mitre: `shapes::offset`
// over `shapes::normals`.
fn moved(t: array<vec2<f32>, 3>, by: f32) -> array<vec2<f32>, 3> {
    let sign = winding(t[0], t[1], t[2]);
    return array<vec2<f32>, 3>(
        t[0] + mitre(t[2], t[0], t[1], sign) * by,
        t[1] + mitre(t[0], t[1], t[2], sign) * by,
        t[2] + mitre(t[1], t[2], t[0], sign) * by,
    );
}

// Where `p` is in the triangle `abc`, as the weights of its corners — or
// a negative weight when it is outside, or the triangle has no area.
fn weights(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>, c: vec2<f32>) -> vec3<f32> {
    let area = perp(b - a, c - a);
    if abs(area) < 1.0e-12 {
        return vec3<f32>(-1.0);
    }
    let u = perp(p - a, c - a) / area;
    let v = perp(b - a, p - a) / area;
    return vec3<f32>(1.0 - u - v, u, v);
}

fn inside(w: vec3<f32>) -> bool {
    return min(min(w.x, w.y), w.z) >= -1.0e-6;
}

// `ShapeBuf::band` between two rings of three, each ring its own share of
// the colour: the two triangles a side it lays, and the share the pixel
// gets interpolated across whichever holds it — what the rasteriser does
// with the vertex colours. Negative when neither does.
fn band(p: vec2<f32>, o: array<vec2<f32>, 3>, oc: f32, n: array<vec2<f32>, 3>, ic: f32) -> f32 {
    for (var i = 0u; i < 3u; i++) {
        let j = (i + 1u) % 3u;
        let w1 = weights(p, o[i], o[j], n[j]);
        if inside(w1) {
            return (w1.x + w1.y) * oc + w1.z * ic;
        }
        let w2 = weights(p, o[i], n[j], n[i]);
        if inside(w2) {
            return w2.x * oc + (w2.y + w2.z) * ic;
        }
    }
    return -1.0;
}

// A triangle filled, feathered by `f`: `ShapeBuf::fill`'s fan and band,
// triangle by triangle. The mitres at a triangle's sharp corners are
// capped, and a capped mitre bends the band along the whole edge, so no
// distance to the edges gives what the rasteriser gave; the triangles do.
fn tri_fill(p: vec2<f32>, t: array<vec2<f32>, 3>, f: f32) -> f32 {
    if f <= 0.0 {
        return select(0.0, 1.0, inside(weights(p, t[0], t[1], t[2])));
    }
    let inner = moved(t, -f * 0.5);
    if inside(weights(p, inner[0], inner[1], inner[2])) {
        return 1.0;
    }
    return max(band(p, moved(t, f * 0.5), 0.0, inner, 1.0), 0.0);
}

// A triangle's outline `line` wide, feathered by `f` along both edges:
// `ShapeBuf::stroke` over the rings `ShapeBuf::triangle` makes.
fn tri_stroke(p: vec2<f32>, t: array<vec2<f32>, 3>, line: f32, f: f32) -> f32 {
    let outer = moved(t, line * 0.5);
    let inner = moved(t, -line * 0.5);
    let outer_in = moved(outer, -f * 0.5);
    let inner_out = moved(inner, f * 0.5);
    var c = band(p, moved(outer, f * 0.5), 0.0, outer_in, 1.0);
    if c < 0.0 {
        c = band(p, outer_in, 1.0, inner_out, 1.0);
    }
    if c < 0.0 {
        c = band(p, inner_out, 1.0, moved(inner, -f * 0.5), 0.0);
    }
    return max(c, 0.0);
}

// The bottom-left half of the box, as the painters define it.
fn triangle(h: vec2<f32>) -> array<vec2<f32>, 3> {
    return array<vec2<f32>, 3>(
        vec2<f32>(-h.x, -h.y),
        vec2<f32>(-h.x, h.y),
        vec2<f32>(h.x, h.y),
    );
}

// 0-1 linear from 0-1 sRGB gamma, and on past one for an emissive channel:
// `canvas.wgsl`'s, the same arithmetic.
fn linear_from_gamma_rgb(srgb: vec3<f32>) -> vec3<f32> {
    let cutoff = srgb < vec3<f32>(0.04045);
    let lower = srgb / vec3<f32>(12.92);
    let higher = pow((srgb + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

// A surface's colour at `p` (points from the shape's centre, before its
// turn): the texel there times the shape's colour `c`, premultiplied as
// `c` is (the coverage goes on after). `more` is the anchor (where the centre is, in
// repeats), how many points a repeat spans and which surface. Sampled
// with the gradient worked out rather than measured — the derivatives
// want uniform control flow, and a pixel a step on is `feather / span`
// of a repeat on — so a far zoom reads the smaller mipmaps and does not
// shimmer. Never past the alpha: a bright surface is white, not emissive.
fn surface(s: Shape, p: vec2<f32>, c: vec4<f32>) -> vec4<f32> {
    let span = max(s.more.z, 1.0e-6);
    let uv = s.more.xy + p / span;
    let g = max(s.head.w, 1.0e-3) / span;
    var code = i32(s.more.w + 0.5);
    let tied = code >= TIED;
    if tied {
        code -= TIED;
    }
    var texel = textureSampleGrad(
        surfaces,
        surfaces_sampler,
        uv,
        code,
        vec2<f32>(g, 0.0),
        vec2<f32>(0.0, g),
    ).rgb;
    // The open ground repeats every eight tiles, which a far zoom shows as
    // a pattern: a second sample of the same texture, half again as large
    // and turned by an angle no repeat lines up with, laid over the first
    // so the two together never repeat. The sum keeps the average and
    // most of the contrast of either.
    if tied {
        let turned = vec2<f32>(
            uv.x * BREAK_TURN.x - uv.y * BREAK_TURN.y,
            uv.x * BREAK_TURN.y + uv.y * BREAK_TURN.x,
        );
        let other = textureSampleGrad(
            surfaces,
            surfaces_sampler,
            turned * BREAK_SCALE + vec2<f32>(0.37, 0.71),
            code,
            vec2<f32>(g * BREAK_SCALE, 0.0),
            vec2<f32>(0.0, g * BREAK_SCALE),
        ).rgb;
        texel = vec3<f32>(0.25) + (texel + other - vec3<f32>(0.5)) * 0.75;
    }
    let rgb = min(c.rgb * texel * SURFACE_GAIN, vec3<f32>(c.a));
    return vec4<f32>(rgb, c.a);
}

@fragment
fn fragment(in: Varyings) -> @location(0) vec4<f32> {
    let px = in.position.xy;
    if px.x < clip.x || px.y < clip.y || px.x >= clip.z || px.y >= clip.w {
        discard;
    }
    let s = shapes.items[in.index];
    let kind = u32(s.head.x + 0.5);
    let p = in.local;
    let f = s.head.w;
    let h = s.body.xy;
    let radius = s.more.x;
    let line = s.more.y;
    let n = u32(s.more.z + 0.5);
    let grow = line * 0.5;
    var cover = 0.0;
    switch kind {
        case 1u: {
            cover = ramp(rounded_d(p, h, radius, n), f);
        }
        case 2u: {
            var outer_r = 0.0;
            var inner_r = 0.0;
            if radius > 0.0 {
                outer_r = radius + grow;
                inner_r = max(radius - grow, 0.0);
            }
            cover = min(
                ramp(rounded_d(p, h + vec2<f32>(grow), outer_r, n), f),
                ramp(-rounded_d(p, h - vec2<f32>(grow), inner_r, n), f),
            );
        }
        case 3u: {
            cover = ramp(ngon_d(p, h, n), f);
        }
        case 4u: {
            cover = min(
                ramp(ngon_d(p, h + vec2<f32>(grow), n), f),
                ramp(-ngon_d(p, h - vec2<f32>(grow), n), f),
            );
        }
        case 5u: {
            cover = tri_fill(p, triangle(h), f);
        }
        case 6u: {
            cover = tri_stroke(p, triangle(h), line, f);
        }
        case 7u: {
            cover = ramp(rounded_d(p, h, 0.0, 1u), f);
        }
        case 8u: {
            cover = tri_fill(p, triangle(h), f);
        }
        default: {}
    }
    if cover <= 0.0 {
        discard;
    }
    var c = s.color;
    if kind == 7u || kind == 8u {
        c = surface(s, p, c);
    }
    // The coverage after the colour is carried to linear, not before: a
    // pixel half covered is half the light. Taken through the sRGB curve
    // with the colour it came to a fifth, so every edge was a dark fringe
    // and two shapes of one colour side by side — a town's rows of
    // ground, a run over the ground beyond it — showed a dark seam where
    // they met.
    return vec4<f32>(linear_from_gamma_rgb(c.rgb) * cover, c.a * cover);
}
