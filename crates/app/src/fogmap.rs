//! The smooth fog: the room's light map, drawn as a picture over the deck.
//!
//! `bims::sight::LightMap` is two bytes a pixel — the darkness: the fog
//! over what the crew do not see of their own deck, the grey and the black
//! over a stranger's, the shade over what they see that no light reaches;
//! and the glow: how much lamplight falls there — worked out by the room
//! as the crew move. The shape buffer cannot carry it (it holds rectangles
//! and ellipses), so it comes over as a texture: uploaded when its version
//! changes — only the box the map says changed, when this holds the
//! version before it, else the whole of it — the two bytes composed into
//! one premultiplied pixel (black under lamplight), drawn as one textured
//! quad on the canvas layer over the shapes and under the words.
//! The quad's corners are the map's four corners through whatever the
//! screen did to the room — the room's own scale and offset on the room
//! screen, the ship's camera and heading on the game's — so the fog lands
//! on the deck it was traced over at any zoom and heading. The plain
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
//! is untouched — and a partial upload is widened by the blur's reach
//! ([`REACH`]) either way, since a pixel just outside the box the room
//! says changed has neighbours inside it.

use bevy_egui::egui;
use bims::sight::LightMap;

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

/// One room's fog texture, kept between frames.
#[derive(Default)]
pub struct FogTexture {
    handle: Option<egui::TextureHandle>,
    version: u64,
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
    /// on the canvas, in window points.
    pub fn paint(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        map: &LightMap,
        corners: [egui::Pos2; 4],
    ) {
        self.paint_pieces(
            ctx,
            painter,
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
        ctx: &egui::Context,
        painter: &egui::Painter,
        map: &LightMap,
        pieces: &[Piece],
    ) {
        if map.width == 0 || map.height == 0 {
            return;
        }
        self.upload(ctx, map);
        let Some(handle) = &self.handle else {
            return;
        };
        let mut mesh = egui::Mesh::with_texture(handle.id());
        for piece in pieces {
            let (u0, v0) = piece.uv0;
            let (u1, v1) = piece.uv1;
            let uvs = [
                egui::pos2(u0, v0),
                egui::pos2(u1, v0),
                egui::pos2(u1, v1),
                egui::pos2(u0, v1),
            ];
            let first = mesh.vertices.len() as u32;
            for (at, uv) in piece.corners.iter().zip(uvs) {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: *at,
                    uv,
                    color: egui::Color32::WHITE,
                });
            }
            mesh.add_triangle(first, first + 1, first + 2);
            mesh.add_triangle(first, first + 2, first + 3);
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    /// Take the map's picture, if this holds another version of it: the
    /// box it says changed when this holds the version before, else the
    /// lot.
    fn upload(&mut self, ctx: &egui::Context, map: &LightMap) {
        if self.handle.is_some() && self.version == map.version {
            return;
        }
        // What the room composed again since the version this holds,
        // if that is the one before: the box it says, else the lot.
        let follows = self.version + 1 == map.version;
        let region = match (follows, map.changed, &self.handle) {
            (true, Some(r), Some(_)) => r,
            _ => (0, 0, map.width, map.height),
        };
        // The box, widened by the blur's reach: what changed inside it
        // shows for that far outside it.
        let (x, y, w, h) = {
            let (x, y, w, h) = region;
            let (x0, y0) = (x.saturating_sub(REACH), y.saturating_sub(REACH));
            let x1 = (x + w + REACH).min(map.width);
            let y1 = (y + h + REACH).min(map.height);
            (x0, y0, x1 - x0, y1 - y0)
        };
        // Two layers in one pixel: the darkness, black at the map's
        // alpha, and the lamplight over it at the map's glow —
        // composed premultiplied, which is what egui's textures are.
        let pixels: Vec<egui::Color32> = blurred(map, (x, y, w, h))
            .into_iter()
            .map(|(a, g)| {
                let (a, g) = (a as f32 / 255.0, g as f32 / 255.0);
                let over = g + a * (1.0 - g);
                egui::Color32::from_rgba_premultiplied(
                    (LAMPLIGHT[0] * g * 255.0) as u8,
                    (LAMPLIGHT[1] * g * 255.0) as u8,
                    (LAMPLIGHT[2] * g * 255.0) as u8,
                    (over * 255.0) as u8,
                )
            })
            .collect();
        let image = egui::ColorImage {
            size: [w, h],
            source_size: egui::vec2(w as f32, h as f32),
            pixels,
        };
        match &mut self.handle {
            Some(handle) if (w, h) == (map.width, map.height) => {
                handle.set(image, egui::TextureOptions::LINEAR)
            }
            Some(handle) => handle.set_partial([x, y], image, egui::TextureOptions::LINEAR),
            None => {
                self.handle = Some(ctx.load_texture("fog", image, egui::TextureOptions::LINEAR));
            }
        }
        self.version = map.version;
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
