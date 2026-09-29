// The softening of the crew's light map (task 140): soft shadows and the
// corners' shade, over the picture `lightmap.wgsl` draws and never the
// rule. Compiled after that file, as one module (`lightmap.rs`), so its
// bindings — group 0 — and its `compose` are this file's too; the passes
// of task 121 are made from `lightmap.wgsl` alone, unchanged, so a run with
// `BIMS_SHADOWS=0 BIMS_AO=0` is their picture and their cost.
//
// 1. `distance`, whenever the cells change: a pixel's distance to the
//    nearest tile in the lamps' way, and to the nearest wall.
// 2. `soften_across`, `soften_down`, whenever that or the lamps' light
//    changes: the shown light's edges given a penumbra that widens away
//    from what casts the shadow, held on their side of lit and dark.
// 3. `compose_soft` in `compose`'s place: the softened light composed,
//    each seen pixel's corner shaded; then `sight_across`, `sight_down`:
//    the edge of what is seen softened the same way, held on its side of
//    half seen. `blur` and the colouring follow as ever.

// The softening's own (task 140), a group of their own that only the
// softening's passes are made with. A pixel: how far the nearest thing in
// the lamps' way is and the nearest wall, sixteenths of a pixel, in the
// low and the high half.
@group(1) @binding(0) var<storage, read_write> dist: array<u32>;
// A pixel: the shown light with the shadows' edges softened.
@group(1) @binding(1) var<storage, read_write> soft: array<u32>;
// The softening across, before the pass down.
@group(1) @binding(2) var<storage, read_write> across: array<f32>;
// A pixel: one where anybody sees it, nought where nobody does.
@group(1) @binding(3) var<storage, read_write> vis: array<f32>;

// `compose` with the softening on: the shown light softened, a seen
// pixel's corner shaded, and whether anybody sees the pixel kept for the
// sight's edge.
@compute @workgroup_size(8, 8)
fn compose_soft(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = id.x;
    let y = id.y;
    if x >= params.width || y >= params.height {
        return;
    }
    let i = y * params.width + x;
    let c = cells[(y >> 3u) * params.columns + (x >> 3u)];
    let word = i / 32u;
    let bit = 1u << (i % 32u);
    var any = false;
    for (var v = 0u; v < params.views; v++) {
        if (atomicLoad(&seen[v * params.words + word]) & bit) != 0u {
            any = true;
            break;
        }
    }
    vis[i] = select(0.0, 1.0, any);
    if (c & 2u) == 0u {
        map[i] = 0u;
        return;
    }
    let l = levels(i, c, x, y, any);
    map[i] = l.x | (l.y << 8u);
}

// A fogged pixel's darkness and lamplight, seen or under the fog: the
// tables' levels for the light shown there — softened, with the lamps'
// shadows on — and a seen pixel's corner shaded, with the corners on.
fn levels(i: u32, c: u32, x: u32, y: u32, is_seen: bool) -> vec2<u32> {
    var light = (fields[i] >> 8u) & 255u;
    if (params.flags & 1u) != 0u {
        light = soft[i];
    }
    if !is_seen {
        return vec2<u32>(params.fog, tables[512u + light]);
    }
    var alpha = tables[light];
    if (params.flags & 2u) != 0u && (c & CELL_FIXED) == 0u {
        alpha = occluded(i, x, y, light, alpha);
    }
    return vec2<u32>(alpha, tables[256u + light]);
}

// --- the softening (task 140) ---------------------------------------------
//
// Over the picture and never the rule: the lamps' shadows are given a
// penumbra that widens with the distance from what casts them, and the
// deck is shaded a little in its corners and along its walls. What the
// room works out is the truth for what is lit and what is dark, and no
// pixel is moved across that line: a pixel the shown field lights at
// [`LIT`] or more is never softened below it, one under it never up to
// it, and a corner's shade never darkens a lit pixel past the dimmest lit
// one or a seen one into the fog.

// A cell's bits past the two the march reads: in the way of a lamp (the
// walls and the tall parts, never a door) and furniture rather than wall.
const CELL_FIXED: u32 = 4u;
const CELL_SOFT: u32 = 8u;
// A tile, in map pixels, and how far the distance is looked for: two
// tiles every way, which is past everything that reads it.
const PX: f32 = 8.0;
const SEARCH: i32 = 2;
const FAR: f32 = 16.0;
// The shown light from which a pixel counts as lit.
const LIT: u32 = 32u;
// The most of its neighbourhood a pixel nobody sees may be drawn as seen.
const SIGHT_BELOW_HALF: f32 = 0.49;
// The penumbra: its reach grows by this much a pixel of distance from
// the nearest thing in the lamps' way, to at most a tile either side.
const SOFT_SLOPE: f32 = 0.5;
const SOFT_REACH: i32 = 8;
// The corners' shade: how far from a wall or a piece of furniture it
// reaches, and how dark it is against each at the foot of it.
const AO_REACH: f32 = 12.0;
const AO_WALL: f32 = 0.30;
const AO_FURNITURE: f32 = 0.18;

// How far a pixel is from the map's edge, as a share of a tile: the
// softening fades out over the outermost tile, where a planet's plain
// meets its own picture, which has none.
fn edge_fade(x: u32, y: u32) -> f32 {
    let edge = min(min(x, y), min(params.width - 1u - x, params.height - 1u - y));
    return clamp(f32(edge) / PX, 0.0, 1.0);
}

// A thread a pixel, whenever the cells change: the distance from its
// middle to the nearest tile in the lamps' way, and to the nearest wall,
// within `SEARCH` tiles (`FAR` past that). The tiles are squares, so this
// is exact: no flood is wanted for a field two tiles deep.
@compute @workgroup_size(8, 8)
fn distance(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = id.x;
    let y = id.y;
    if x >= params.width || y >= params.height {
        return;
    }
    let p = vec2<f32>(f32(x) + 0.5, f32(y) + 0.5);
    let tx = i32(x >> 3u);
    let ty = i32(y >> 3u);
    let rows = i32(params.height >> 3u);
    let columns = i32(params.columns);
    var any = FAR;
    var wall = FAR;
    for (var dy = -SEARCH; dy <= SEARCH; dy++) {
        for (var dx = -SEARCH; dx <= SEARCH; dx++) {
            let t = vec2<i32>(tx + dx, ty + dy);
            if t.x < 0 || t.y < 0 || t.x >= columns || t.y >= rows {
                continue;
            }
            let c = cells[u32(t.y * columns + t.x)];
            if (c & CELL_FIXED) == 0u {
                continue;
            }
            let lo = vec2<f32>(t) * PX;
            let d = length(max(max(lo - p, p - (lo + PX)), vec2<f32>(0.0)));
            any = min(any, d);
            if (c & CELL_SOFT) == 0u {
                wall = min(wall, d);
            }
        }
    }
    dist[y * params.width + x] = u32(min(any, FAR) * 16.0) | (u32(min(wall, FAR) * 16.0) << 16u);
}

// The penumbra's reach at a pixel: nought against what casts the shadow,
// a tile out where nothing is near.
fn reach_at(i: u32, x: u32, y: u32) -> f32 {
    let d = f32(dist[i] & 65535u) / 16.0;
    return min(d * SOFT_SLOPE, f32(SOFT_REACH)) * edge_fade(x, y);
}

// A tap's weight: a Gaussian whose spread is half the reach, so a reach
// of nought is the pixel alone.
fn weight(k: i32, r: f32) -> f32 {
    let sigma = max(r * 0.5, 0.01);
    let t = f32(k) / sigma;
    return exp(-0.5 * t * t);
}

// A thread a pixel, whenever the lamps' light changes: the shown light
// softened across, at the pixel's own reach.
@compute @workgroup_size(8, 8)
fn soften_across(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = i32(id.x);
    let y = i32(id.y);
    let w = i32(params.width);
    if x >= w || y >= i32(params.height) {
        return;
    }
    let i = u32(y * w + x);
    let r = reach_at(i, id.x, id.y);
    var sum = 0.0;
    var total = 0.0;
    for (var k = -SOFT_REACH; k <= SOFT_REACH; k++) {
        let col = clamp(x + k, 0, w - 1);
        let v = f32((fields[u32(y * w + col)] >> 8u) & 255u);
        let wk = weight(k, r);
        sum += v * wk;
        total += wk;
    }
    across[i] = sum / total;
}

// And down, and held on its own side of the line between lit and dark.
@compute @workgroup_size(8, 8)
fn soften_down(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = i32(id.x);
    let y = i32(id.y);
    let w = i32(params.width);
    let h = i32(params.height);
    if x >= w || y >= h {
        return;
    }
    let i = u32(y * w + x);
    let r = reach_at(i, id.x, id.y);
    var sum = 0.0;
    var total = 0.0;
    for (var k = -SOFT_REACH; k <= SOFT_REACH; k++) {
        let row = clamp(y + k, 0, h - 1);
        let wk = weight(k, r);
        sum += across[u32(row * w + x)] * wk;
        total += wk;
    }
    var s = u32(round(clamp(sum / total, 0.0, 255.0)));
    let raw = (fields[i] >> 8u) & 255u;
    if raw >= LIT {
        s = max(s, LIT);
    } else {
        s = min(s, LIT - 1u);
    }
    soft[i] = s;
}

// A seen pixel's darkness with its corner's shade laid over it, darkness
// over darkness, faded out as the wall or the furniture falls away — and
// held on its side: a lit pixel no darker than the dimmest lit one, a
// dark one short of the fog. Never lighter than it was.
fn occluded(i: u32, x: u32, y: u32, light: u32, alpha: u32) -> u32 {
    let d = dist[i];
    let any = f32(d & 65535u) / 16.0;
    let wall = f32(d >> 16u) / 16.0;
    let near_any = 1.0 - clamp(any / AO_REACH, 0.0, 1.0);
    let near_wall = 1.0 - clamp(wall / AO_REACH, 0.0, 1.0);
    let ao = max(AO_WALL * near_wall * near_wall, AO_FURNITURE * near_any * near_any) * edge_fade(x, y);
    let a = f32(alpha) / 255.0;
    var out = u32((a + ao * (1.0 - a)) * 255.0);
    if light >= LIT {
        out = min(out, tables[LIT]);
    } else {
        out = min(out, params.fog - 1u);
    }
    return max(out, alpha);
}

// The edge of what the crew see, softened the same way (task 140) — the
// shadow a wall's corner throws across the eyes' fan, which is the line
// on the deck most like a shadow. A thread a pixel whenever the picture is
// composed: how much of the pixel's neighbourhood is seen, across.
@compute @workgroup_size(8, 8)
fn sight_across(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = i32(id.x);
    let y = i32(id.y);
    let w = i32(params.width);
    if x >= w || y >= i32(params.height) {
        return;
    }
    let i = u32(y * w + x);
    let r = reach_at(i, id.x, id.y);
    var sum = 0.0;
    var total = 0.0;
    for (var k = -SOFT_REACH; k <= SOFT_REACH; k++) {
        let col = clamp(x + k, 0, w - 1);
        let wk = weight(k, r);
        sum += vis[u32(y * w + col)] * wk;
        total += wk;
    }
    across[i] = sum / total;
}

// And down; the share held on the pixel's own side of half — a pixel
// anybody sees at least half seen, one nobody sees under half — so the
// middle of the ramp is the room's own edge, and the pixel is drawn that
// share of the way from the fog to what is seen there.
@compute @workgroup_size(8, 8)
fn sight_down(@builtin(global_invocation_id) id: vec3<u32>) {
    let x = i32(id.x);
    let y = i32(id.y);
    let w = i32(params.width);
    let h = i32(params.height);
    if x >= w || y >= h {
        return;
    }
    let i = u32(y * w + x);
    let c = cells[(u32(y) >> 3u) * params.columns + (u32(x) >> 3u)];
    if (c & 2u) == 0u {
        return;
    }
    let r = reach_at(i, id.x, id.y);
    var sum = 0.0;
    var total = 0.0;
    for (var k = -SOFT_REACH; k <= SOFT_REACH; k++) {
        let row = clamp(y + k, 0, h - 1);
        let wk = weight(k, r);
        sum += across[u32(row * w + x)] * wk;
        total += wk;
    }
    var share = sum / total;
    if vis[i] > 0.5 {
        share = max(share, 0.5);
    } else {
        share = min(share, SIGHT_BELOW_HALF);
    }
    let seen = levels(i, c, id.x, id.y, true);
    let fog = levels(i, c, id.x, id.y, false);
    let mixed = mix(vec2<f32>(fog), vec2<f32>(seen), share);
    map[i] = u32(round(mixed.x)) | (u32(round(mixed.y)) << 8u);
}
