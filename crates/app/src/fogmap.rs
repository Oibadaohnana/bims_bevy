//! The smooth fog: the room's light map, drawn as a picture over the deck.
//!
//! `bims::sight::LightMap` is two bytes a pixel — the darkness: the one
//! fog over everything the crew do not see, their own deck, a stranger's
//! and the plain alike (task 128; nothing is black), the shade over what
//! they see that no light reaches; and the glow: how much lamplight falls
//! there — worked out by the room
//! as the crew move. The shape buffer cannot carry it (it holds rectangles
//! and ellipses), so it comes over as a texture the GPU fills
//! (`lightmap.rs`): from the room's inputs for the deck (task 121), and
//! for a map the CPU worked out — a planet's plain, the deck under
//! `BIMS_LIGHTMAP=cpu` — from its two channels, packed a word a pixel
//! when its version changes, only the box the map says changed when this
//! holds the version before it (task 140). Either way the two bytes are
//! composed on the GPU into one premultiplied pixel (black under
//! lamplight) and drawn as one textured
//! quad on the world's canvas (`scene.rs`) — over the world's shapes,
//! under the shots and the rings the room draws over its fog, and under
//! every word, which egui puts on after the canvas is drawn.
//! The quad's corners are the map's four corners through the ship's
//! camera and heading, so the fog lands on the deck it was traced over at
//! any zoom and heading. The plain
//! beyond the deck, on a planet, is the same picture a chunk at a time
//! (`bims::terrain::Plane::picture`): the game screen keeps a texture a
//! chunk and draws each as `paint_pieces` — the chunk less the deck's
//! box, which the light map covers, and never the picture's apron.
//!
//! The map's edges are **blurred on the way in**. The room marches its
//! rays pixel by pixel, so the edge of what is seen — the line a wall's
//! corner throws — is a stair of one-pixel steps, and eight pixels to a
//! tile is six or seven screen pixels a step at a close zoom, which the
//! texture's own linear filter turns into a ramp a step wide and no
//! softer. So each channel is run through a five-tap binomial each way
//! ([`TAPS`]) as the texture is composed, which turns the stair into a
//! ramp half a tile wide: a penumbra, the way a shadow's edge is. The
//! blur is the picture's alone — the room's map, which the fight reads,
//! is untouched. The blur is the GPU's (`lightmap.wgsl`'s `blur`); the
//! Rust here ([`blurred`], [`texel`]) is the reference it is held to,
//! byte for byte, under `BIMS_LIGHTMAP=check`.

use std::sync::Arc;

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageSampler;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy_egui::egui;
use bims::sight::LightMap;

use crate::lightmap::RawJob;
use crate::scene::WorldCanvas;
use crate::shapes::Rect;

/// The colour a lamp washes the deck with, as the map's glow: the
/// fittings' lamplight, warm.
const LAMPLIGHT: [f32; 3] = [1.0, 0.92, 0.70];

/// The blur over the map's edges: a binomial, one pass across and one
/// down, the weights summing to sixteen a pass.
const TAPS: [u32; 5] = [1, 4, 6, 4, 1];
/// How far the blur reads either side of a pixel, in map pixels.
const REACH: usize = TAPS.len() / 2;

/// The map's two channels — darkness, glow — over the box `(x, y, w, h)`
/// of it, blurred by [`TAPS`] each way, row by row. Reads past the box
/// for the blur's reach and clamps at the map's edge.
fn blurred(map: &LightMap, (x, y, w, h): (usize, usize, usize, usize)) -> Vec<(u8, u8)> {
    let (mw, mh) = (map.width, map.height);
    // Across first, over every row the pass down will read.
    let ry0 = y.saturating_sub(REACH);
    let ry1 = (y + h + REACH).min(mh);
    let mut across = vec![(0u32, 0u32); (ry1 - ry0) * w];
    for (r, row) in (ry0..ry1).enumerate() {
        for i in 0..w {
            let (mut a, mut g) = (0, 0);
            for (k, &t) in TAPS.iter().enumerate() {
                let col = (x + i + k).saturating_sub(REACH).min(mw - 1);
                let j = row * mw + col;
                a += map.alpha[j] as u32 * t;
                g += map.glow[j] as u32 * t;
            }
            across[r * w + i] = (a, g);
        }
    }
    let mut out = Vec::with_capacity(w * h);
    for row in y..y + h {
        for i in 0..w {
            let (mut a, mut g) = (0, 0);
            for (k, &t) in TAPS.iter().enumerate() {
                let r = (row + k).saturating_sub(REACH).clamp(ry0, ry1 - 1) - ry0;
                let (pa, pg) = across[r * w + i];
                a += pa * t;
                g += pg * t;
            }
            out.push(((a / 256) as u8, (g / 256) as u8));
        }
    }
    out
}

/// One pixel of the fog texture from the blurred darkness and lamplight:
/// two layers in one pixel, the darkness black at the map's alpha and the
/// lamplight over it at the map's glow — composed premultiplied, which is
/// what egui's textures were and what the canvas's shader blends. The GPU
/// reads the same pixel out of a table made of this (`lightmap.rs`).
pub fn texel(a: u8, g: u8) -> [u8; 4] {
    let (a, g) = (a as f32 / 255.0, g as f32 / 255.0);
    let over = g + a * (1.0 - g);
    [
        (LAMPLIGHT[0] * g * 255.0) as u8,
        (LAMPLIGHT[1] * g * 255.0) as u8,
        (LAMPLIGHT[2] * g * 255.0) as u8,
        (over * 255.0) as u8,
    ]
}

/// The whole of a map's fog texture as the CPU makes it, row by row: what
/// a check of the GPU's picture compares against (`BIMS_LIGHTMAP=check`).
pub fn texels(map: &LightMap) -> Vec<[u8; 4]> {
    blurred(map, (0, 0, map.width, map.height))
        .into_iter()
        .map(|(a, g)| texel(a, g))
        .collect()
}

/// One room's fog texture, kept between frames: an image of the world's
/// canvas (`scene.rs`), in the bytes egui's textures were — premultiplied,
/// sRGB — so the canvas's shader draws it as egui drew it.
#[derive(Default)]
pub struct FogTexture {
    handle: Option<Handle<Image>>,
    /// The picture's size in pixels.
    size: (usize, usize),
    /// A map the CPU worked out, as the GPU takes it (task 140): its two
    /// channels a word a pixel, which version of the map they are, and
    /// the box that version changed from the one before in.
    packed: Option<Arc<Vec<u32>>>,
    version: u64,
    changed: Option<(usize, usize, usize, usize)>,
}

impl FogTexture {
    /// Keep `image`, of `size`, as the picture: under the handle held
    /// while it is the same size, else under a new one. A canvas
    /// material's bind group is made once over the texture its picture
    /// had then and is never made again for the picture changing under
    /// the same handle; a picture of another size is another texture, so
    /// kept under the old handle the fog went on drawing the texture
    /// before — the station left behind, stretched over this one. A new
    /// handle is a new picture to the canvas (`scene.rs`), which makes its
    /// material again.
    fn keep(&mut self, images: &mut Assets<Image>, image: Image, size: (usize, usize)) {
        match &self.handle {
            Some(handle) if self.size == size => {
                let _ = images.insert(handle.id(), image);
            }
            _ => self.handle = Some(images.add(image)),
        }
        self.size = size;
    }
}

/// A piece of the texture to draw: the part of it between `uv0` and
/// `uv1` (nought to one across the map), at `corners` on the canvas in
/// window points — the piece's origin, then clockwise.
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub uv0: (f32, f32),
    pub uv1: (f32, f32),
    pub corners: [egui::Pos2; 4],
}

impl FogTexture {
    /// Draw `map` with its corners — origin, then clockwise — at `corners`
    /// on the canvas `rect`, in window points: over the world's shapes
    /// painted so far, and under whatever is painted on the canvas after.
    pub fn paint(
        &mut self,
        canvas: &mut WorldCanvas,
        ctx: &egui::Context,
        rect: Rect,
        map: &LightMap,
        corners: [egui::Pos2; 4],
    ) {
        self.paint_pieces(
            canvas,
            ctx,
            rect,
            map,
            &[Piece {
                uv0: (0.0, 0.0),
                uv1: (1.0, 1.0),
                corners,
            }],
        );
    }

    /// Draw these pieces of `map`: the plain's fog draws a chunk's
    /// picture less the room's box and never its apron.
    pub fn paint_pieces(
        &mut self,
        canvas: &mut WorldCanvas,
        ctx: &egui::Context,
        rect: Rect,
        map: &LightMap,
        pieces: &[Piece],
    ) {
        if map.width == 0 || map.height == 0 {
            return;
        }
        let _timed = crate::perf::scope(crate::perf::Phase::Fog);
        // Drawn on the GPU either way: a blank picture of the map's size,
        // which `lightmap.rs` fills this frame before it is drawn — from
        // the room's inputs (task 121), or from the two channels the CPU
        // worked out, blurred and coloured there (task 140).
        self.blank(canvas.images(), map);
        if let Some(id) = self.handle.as_ref().map(|h| h.id()) {
            match &map.inputs {
                Some(inputs) => canvas.light_job(inputs.clone(), id),
                None => {
                    let job = self.pack(map, id);
                    canvas.raw_light_job(job);
                }
            }
        }
        let Some(handle) = &self.handle else {
            return;
        };
        let quads: Vec<([egui::Pos2; 4], [egui::Pos2; 4])> = pieces
            .iter()
            .map(|piece| {
                let (u0, v0) = piece.uv0;
                let (u1, v1) = piece.uv1;
                let uvs = [
                    egui::pos2(u0, v0),
                    egui::pos2(u1, v0),
                    egui::pos2(u1, v1),
                    egui::pos2(u0, v1),
                ];
                (piece.corners, uvs)
            })
            .collect();
        canvas.picture(ctx, rect, handle.clone(), &quads);
    }

    /// A picture of the map's size and nothing in it, for the GPU to
    /// write into — the one this holds, while it is the right size.
    fn blank(&mut self, images: &mut Assets<Image>, map: &LightMap) {
        if self.handle.is_some() && self.size == (map.width, map.height) {
            return;
        }
        let image = Image {
            sampler: ImageSampler::linear(),
            ..Image::new(
                Extent3d {
                    width: map.width as u32,
                    height: map.height as u32,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                vec![0; map.width * map.height * 4],
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            )
        };
        self.keep(images, image, (map.width, map.height));
    }

    /// The map's two channels as the GPU takes them, packed again only
    /// where this version changed them — the box the map says, when this
    /// holds the version before, else the lot — and handed over whole: a
    /// job the GPU uploads the changed rows of and blurs.
    fn pack(&mut self, map: &LightMap, picture: AssetId<Image>) -> RawJob {
        let (w, h) = (map.width, map.height);
        let pixel = |i: usize| map.alpha[i] as u32 | (map.glow[i] as u32) << 8;
        let held = self.packed.as_ref().is_some_and(|p| p.len() == w * h);
        if !held || self.version != map.version {
            let follows = held && self.version.wrapping_add(1) == map.version;
            match (follows, map.changed, &mut self.packed) {
                (true, Some((x, y, bw, bh)), Some(packed)) => {
                    // Copied first only if the GPU's side still holds
                    // this frame's; one row of the box at a time.
                    let data = Arc::make_mut(packed);
                    for row in y..y + bh {
                        let from = row * w + x;
                        for (k, out) in data[from..from + bw].iter_mut().enumerate() {
                            *out = pixel(from + k);
                        }
                    }
                    self.changed = Some((x, y, bw, bh));
                }
                _ => {
                    self.packed = Some(Arc::new((0..w * h).map(pixel).collect()));
                    self.changed = None;
                }
            }
            self.version = map.version;
        }
        RawJob {
            picture,
            width: w,
            height: h,
            version: self.version,
            changed: self.changed,
            pixels: self.packed.clone().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(width: usize, height: usize, alpha: Vec<u8>) -> LightMap {
        LightMap {
            width,
            height,
            glow: vec![0; alpha.len()],
            alpha,
            ..Default::default()
        }
    }

    /// A flat map blurs to itself: the taps sum to one.
    /// A step along a row comes out a ramp the blur's reach either side
    /// of it and flat beyond — and the same whether the box asked for is
    /// the whole row or a part of it.
    #[test]
    fn a_flat_map_is_unchanged_and_a_step_becomes_a_ramp() {
        // --- a_flat_map_is_unchanged ---
        {
            let m = map(9, 9, vec![200; 81]);
            assert!(blurred(&m, (0, 0, 9, 9)).iter().all(|&(a, _)| a == 200));
            assert!(blurred(&m, (3, 3, 2, 2)).iter().all(|&(a, _)| a == 200));
        }

        // --- a_step_becomes_a_ramp ---
        {
            let mut alpha = vec![0u8; 12];
            alpha[6..].fill(255);
            let m = map(12, 1, alpha);
            let whole: Vec<u8> = blurred(&m, (0, 0, 12, 1)).iter().map(|p| p.0).collect();
            assert_eq!(&whole[..4], &[0, 0, 0, 0]);
            assert_eq!(&whole[8..], &[255, 255, 255, 255]);
            assert!(whole[4] < whole[5] && whole[5] < whole[6] && whole[6] < whole[7]);
            let part: Vec<u8> = blurred(&m, (5, 0, 3, 1)).iter().map(|p| p.0).collect();
            assert_eq!(part, whole[5..8]);
        }
    }
}

#[cfg(test)]
mod packing {
    use super::*;

    /// A map the CPU worked out goes to the GPU a word a pixel — the
    /// darkness low, the lamplight above (task 140). The version after the
    /// one held is packed again over its changed box alone and comes out
    /// what packing it whole would; a version skipped is packed whole; and
    /// the same version asked for twice is the same job.
    #[test]
    fn a_map_is_packed_again_only_where_it_changed() {
        let picture = AssetId::<Image>::default();
        let mut map = LightMap {
            width: 4,
            height: 3,
            alpha: (0..12).collect(),
            glow: (100..112).collect(),
            version: 1,
            ..Default::default()
        };
        let mut fog = FogTexture::default();
        let first = fog.pack(&map, picture);
        assert_eq!(first.changed, None);
        assert_eq!(first.pixels[5], 5 | 105 << 8);
        let again = fog.pack(&map, picture);
        assert!(Arc::ptr_eq(&first.pixels, &again.pixels));

        // The next version, changed in a box: that box packed again.
        map.alpha[6] = 200;
        map.glow[6] = 7;
        map.alpha[0] = 99; // outside the box: not taken, as the room says
        map.version = 2;
        map.changed = Some((1, 1, 2, 1));
        let next = fog.pack(&map, picture);
        assert_eq!(next.changed, Some((1, 1, 2, 1)));
        assert_eq!(next.pixels[6], 200 | 7 << 8);
        assert_eq!(next.pixels[0], 100 << 8);
        // The job the GPU still holds is untouched.
        assert_eq!(first.pixels[6], 6 | 106 << 8);

        // A version skipped: the whole of it.
        map.version = 4;
        let whole = fog.pack(&map, picture);
        assert_eq!(whole.changed, None);
        assert_eq!(whole.pixels[0], 99 | 100 << 8);
    }
}
