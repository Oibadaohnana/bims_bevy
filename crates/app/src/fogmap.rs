//! The smooth fog: the room's light map, drawn as a picture over the deck.
//!
//! `bims::sight::LightMap` is one byte a pixel of darkness — the fog over
//! what the crew do not see of their own deck, the shade over what they
//! see of it that no light reaches — worked out by the room when the mask
//! moves. The shape buffer cannot carry it (it holds rectangles and
//! ellipses), so it comes over as a texture: uploaded when its version
//! changes, drawn as one textured quad on the canvas layer over the shapes
//! and under the words, filtered so a pixel eight to a tile reads as a
//! soft edge rather than as a step. The quad's corners are the map's four
//! corners through whatever the screen did to the room — the room's own
//! scale and offset on the room screen, the ship's camera and heading on
//! the game's — so the fog lands on the deck it was traced over at any
//! zoom and heading.

use bevy_egui::egui;
use bims::sight::LightMap;

/// One room's fog texture, kept between frames.
#[derive(Default)]
pub struct FogTexture {
    handle: Option<egui::TextureHandle>,
    version: u64,
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
        if map.width == 0 || map.height == 0 {
            return;
        }
        if self.handle.is_none() || self.version != map.version {
            let pixels: Vec<egui::Color32> = map
                .alpha
                .iter()
                .map(|&a| egui::Color32::from_black_alpha(a))
                .collect();
            let image = egui::ColorImage {
                size: [map.width, map.height],
                source_size: egui::vec2(map.width as f32, map.height as f32),
                pixels,
            };
            match &mut self.handle {
                Some(handle) => handle.set(image, egui::TextureOptions::LINEAR),
                None => {
                    self.handle =
                        Some(ctx.load_texture("fog", image, egui::TextureOptions::LINEAR));
                }
            }
            self.version = map.version;
        }
        let Some(handle) = &self.handle else {
            return;
        };
        let mut mesh = egui::Mesh::with_texture(handle.id());
        let uvs = [
            egui::pos2(0.0, 0.0),
            egui::pos2(1.0, 0.0),
            egui::pos2(1.0, 1.0),
            egui::pos2(0.0, 1.0),
        ];
        for (at, uv) in corners.iter().zip(uvs) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: *at,
                uv,
                color: egui::Color32::WHITE,
            });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(egui::Shape::mesh(mesh));
    }
}
