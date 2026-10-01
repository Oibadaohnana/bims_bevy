//! The surfaces: the textures the world's floors, walls and ground are
//! filled with (`ship::draw::KIND_SURFACE`).
//!
//! Each is a seamless square picture, `textures/<name>.png`, made by
//! `textures/make.py` (numpy; run it in `nix-shell -p
//! "python3.withPackages(ps: [ps.numpy ps.scipy ps.pillow])"`). A texel is
//! not a colour but **the surface over its own average, a quarter scale**:
//! every channel averages 64 of 255, and `shape.wgsl` multiplies it by
//! four and by the shape's colour. So the painters' colours stay what
//! they were — the deck is still `DECK` on average — and a canvas drawn
//! without the textures (`BIMS_SHAPES=cpu`, a canvas inside a panel) is the
//! picture it always was.
//!
//! The pictures go to the GPU once, at start, as one texture array — a
//! layer a surface in [`SURFACES`]' order, which is `ship::draw::Surface`'s
//! — with every mipmap worked out here, so a far zoom is the average and
//! not a shimmer. **`BIMS_SURFACES=0`** leaves them flat (every texel the
//! average), for looking at the difference.

use bevy::asset::RenderAssetUsages;
use bevy::image::{
    CompressedImageFormats, ImageAddressMode, ImageFilterMode, ImageSampler,
    ImageSamplerDescriptor, ImageType,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension,
};

/// One surface: its picture, how far one repeat of it reaches in world
/// units, and whether it is tied to the **world** rather than to the
/// painter's anchor — the open ground, which several painters lay side by
/// side in different frames (the planet's backdrop, the town's yards),
/// and which has no tiles to line up with.
struct Def {
    png: &'static [u8],
    repeat: f32,
    world: bool,
}

/// A tile, in world units (`shipdesign::TILE`).
const TILE: f32 = 52.0;

/// Every surface, in `ship::draw::Surface`'s order.
const SURFACES: [Def; 13] = [
    // Deck: a steel plate a tile, sixteen to a repeat.
    Def {
        png: include_bytes!("../textures/deck.png"),
        repeat: 4.0 * TILE,
        world: false,
    },
    // Bulkhead: a panel a tile.
    Def {
        png: include_bytes!("../textures/bulkhead.png"),
        repeat: 4.0 * TILE,
        world: false,
    },
    Def {
        png: include_bytes!("../textures/grass.png"),
        repeat: 8.0 * TILE,
        world: true,
    },
    Def {
        png: include_bytes!("../textures/sand.png"),
        repeat: 8.0 * TILE,
        world: true,
    },
    Def {
        png: include_bytes!("../textures/snow.png"),
        repeat: 8.0 * TILE,
        world: true,
    },
    // Stone: two courses a tile.
    Def {
        png: include_bytes!("../textures/stone.png"),
        repeat: 4.0 * TILE,
        world: false,
    },
    Def {
        png: include_bytes!("../textures/adobe.png"),
        repeat: 4.0 * TILE,
        world: false,
    },
    Def {
        png: include_bytes!("../textures/timber.png"),
        repeat: 4.0 * TILE,
        world: false,
    },
    Def {
        png: include_bytes!("../textures/floorboard.png"),
        repeat: 4.0 * TILE,
        world: false,
    },
    // Concrete: a slab two tiles square.
    Def {
        png: include_bytes!("../textures/concrete.png"),
        repeat: 4.0 * TILE,
        world: false,
    },
    Def {
        png: include_bytes!("../textures/rock.png"),
        repeat: 8.0 * TILE,
        world: true,
    },
    Def {
        png: include_bytes!("../textures/water.png"),
        repeat: 8.0 * TILE,
        world: true,
    },
    Def {
        png: include_bytes!("../textures/ice.png"),
        repeat: 8.0 * TILE,
        world: true,
    },
];

/// The side of every picture, in texels.
const SIDE: u32 = 1024;

/// How far one repeat of `surface` reaches, in world units — a tile for
/// one that is not a surface, which nothing paints.
pub fn repeat(surface: u32) -> f32 {
    SURFACES.get(surface as usize).map_or(TILE, |d| d.repeat)
}

/// Whether `surface` is anchored where it is in the world rather than
/// where its painter says (see [`Def`]).
pub fn tied_to_the_world(surface: u32) -> bool {
    SURFACES.get(surface as usize).is_some_and(|d| d.world)
}

/// The surfaces' texture array, for every shape layer's material.
#[derive(Resource, Clone)]
pub struct SurfaceTextures(pub Handle<Image>);

/// Whether the surfaces are textured (`BIMS_SURFACES=0` is flat). Read once.
fn textured() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_SURFACES").as_deref() != Ok("0"))
}

/// The array made and put in the world, at start: before the first frame
/// any shape layer is made in.
pub fn load(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(SurfaceTextures(images.add(array())));
}

/// Every surface decoded, its mipmaps made, and all of it in one image —
/// layer by layer, each layer's levels largest first, as the GPU takes an
/// array's data. A picture that will not decode, or is the wrong size, is
/// flat: the average, so its shapes are their colour.
fn array() -> Image {
    let levels = SIDE.ilog2() + 1;
    let mut data = Vec::new();
    for def in &SURFACES {
        let mut level = textured()
            .then(|| rgba(def.png))
            .flatten()
            .unwrap_or_else(|| flat(SIDE));
        let mut side = SIDE;
        data.extend_from_slice(&level);
        while side > 1 {
            level = halved(&level, side);
            side /= 2;
            data.extend_from_slice(&level);
        }
    }
    let mut image = Image::new_uninit(
        Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: SURFACES.len() as u32,
        },
        TextureDimension::D2,
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..default()
    });
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 4,
        ..default()
    });
    image
}

/// A picture's texels, RGBA, if it is a [`SIDE`]-square PNG.
fn rgba(png: &[u8]) -> Option<Vec<u8>> {
    let image = Image::from_buffer(
        png,
        ImageType::Extension("png"),
        CompressedImageFormats::NONE,
        false,
        ImageSampler::Default,
        RenderAssetUsages::RENDER_WORLD,
    )
    .ok()?;
    let size = image.texture_descriptor.size;
    if size.width != SIDE || size.height != SIDE {
        return None;
    }
    let data = image.data?;
    (data.len() == (SIDE * SIDE * 4) as usize).then_some(data)
}

/// A picture of nothing but the average.
fn flat(side: u32) -> Vec<u8> {
    [64, 64, 64, 255].repeat((side * side) as usize)
}

/// The next mipmap down: each texel the average of the four over it.
/// The picture repeats, so every level of it does too.
fn halved(level: &[u8], side: u32) -> Vec<u8> {
    let half = (side / 2) as usize;
    let side = side as usize;
    let mut out = vec![0u8; half * half * 4];
    for y in 0..half {
        for x in 0..half {
            for c in 0..4 {
                let at =
                    |dx: usize, dy: usize| level[((2 * y + dy) * side + 2 * x + dx) * 4 + c] as u32;
                let sum = at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1);
                out[(y * half + x) * 4 + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every picture is there, the size the array is, and averages a
    /// quarter in every channel — what `shape.wgsl` takes it to be, so a
    /// surface is its painter's colour on average.
    #[test]
    fn every_surface_decodes_square_and_averages_a_quarter() {
        for (i, def) in SURFACES.iter().enumerate() {
            let texels = rgba(def.png).unwrap_or_else(|| panic!("surface {i} will not decode"));
            for c in 0..3 {
                let sum: u64 = texels.iter().skip(c).step_by(4).map(|&v| v as u64).sum();
                let mean = sum as f64 / (SIDE * SIDE) as f64;
                assert!(
                    (mean - 64.0).abs() < 1.5,
                    "surface {i} channel {c} averages {mean}"
                );
            }
        }
    }

    /// The array's data is every layer's every level, and a level is the
    /// average of the one above.
    #[test]
    fn the_mipmaps_halve_down_to_a_texel() {
        let level = [
            10, 20, 30, 255, 30, 40, 50, 255, 50, 60, 70, 255, 70, 80, 90, 255,
        ];
        assert_eq!(halved(&level, 2), vec![40, 50, 60, 255]);
        let image = array();
        let per_layer: usize = (0..=SIDE.ilog2())
            .map(|l| ((SIDE >> l) * (SIDE >> l) * 4) as usize)
            .sum();
        assert_eq!(
            image.data.as_ref().map(Vec::len),
            Some(per_layer * SURFACES.len())
        );
    }
}
