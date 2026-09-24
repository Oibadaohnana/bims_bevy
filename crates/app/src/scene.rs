//! The world's canvas, drawn by Bevy — and the bloom over it (feature 97).
//!
//! egui paints every panel, every word and every canvas *inside* a panel;
//! the canvas *between* the panels — the deck, the map, the chart, the
//! yard — is drawn here instead, as Bevy meshes on the one
//! camera, because that is the only place a post-process can reach it.
//! bevy_egui draws into the camera's target with a pass of its own, after
//! the main pass and before the picture goes to the window, so what egui
//! paints is never bloomed and a mesh egui paints over is under it.
//!
//! **One frame, in the order it reaches the window:**
//!
//! 1. The main pass: the canvas's layers, in the order the screen painted
//!    them ([`WorldCanvas`]) — the world's shapes, the fog pictures over
//!    them, then the shapes that go over the fog (the shots, the rings,
//!    the marquee), each a mesh a z apart.
//! 2. The **bloom** ([`BLOOM_INTENSITY`], [`BLOOM_THRESHOLD`]), on an HDR
//!    target: only what is brighter than white is picked up — a colour
//!    with a channel past one, which nothing but an emissive shape has
//!    ([`crate::shapes::Paint`]) — and it is **added** to the picture, so
//!    a pixel no glow reaches is the pixel it was.
//! 3. No tonemapping: [`Tonemapping::None`] skips the pass outright, so
//!    the palette is not moved and no lookup table is wanted. What is past
//!    one is clipped to white on its way to the window.
//! 4. egui — the words over the deck, the panels and the windows — pinned
//!    after the whole post-process by [`egui_after_bloom`]. That order is
//!    not bevy_egui's by default here: it puts its 2D pass after the main
//!    pass and after `bevy_ui`'s, and with no `bevy_ui` in this build the
//!    second says nothing, so the pass could run before the bloom.
//!
//! **`BIMS_BLOOM=0`** turns the bloom off, for looking at the difference
//! and for a GPU that would rather not: the camera is then not HDR either,
//! so the target is the window's own eight bits and an emissive colour is
//! white.
//!
//! The canvas's material is egui's own shader over again (`canvas.wgsl`),
//! blended premultiplied as egui blends, with the canvas's rectangle as
//! egui's scissor, so a layer drawn here is the picture egui drew there.

use bevy::asset::{RenderAssetUsages, load_internal_asset, uuid_handle};
use bevy::camera::visibility::{NoFrustumCulling, VisibilitySystems};
use bevy::camera::{CameraUpdateSystems, Hdr};
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::core_pipeline::{Core2d, Core2dSystems};
use bevy::ecs::system::SystemParam;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::post_process::bloom::{Bloom, BloomCompositeMode, BloomPrefilter};
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::render_resource::{
    AsBindGroup, BlendState, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::render::view::Msaa;
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin};
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiPostUpdateSet, egui};

use crate::shapes::{Rect, ShapeBuf, View};

// --- the bloom ----------------------------------------------------------------

/// How much of the glow is added over the picture. Bevy's bloom composites
/// **additively** here ([`BloomCompositeMode::Additive`]): the energy-
/// conserving mode mixes the whole picture towards the blurred one, which
/// would dim every wall by this much whether anything glowed or not.
pub const BLOOM_INTENSITY: f32 = 0.35;

/// What is picked up: a pixel brighter than this, in linear light. White
/// is 1.0 exactly and no ordinary colour is brighter, so the walls, the
/// floors, the Bims, the fog and the words are all under it; only a shape
/// painted with a channel past one is over.
pub const BLOOM_THRESHOLD: f32 = 1.0;

/// No knee under the threshold: a colour at white contributes nothing at
/// all, rather than a little.
pub const BLOOM_SOFTNESS: f32 = 0.0;

/// `BIMS_BLOOM=0`: no bloom and no HDR target. Anything else, or nothing,
/// is the bloom. Read once.
pub fn bloom_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_BLOOM").as_deref() != Ok("0"))
}

fn bloom() -> Bloom {
    Bloom {
        intensity: BLOOM_INTENSITY,
        prefilter: BloomPrefilter {
            threshold: BLOOM_THRESHOLD,
            threshold_softness: BLOOM_SOFTNESS,
        },
        composite_mode: BloomCompositeMode::Additive,
        ..Bloom::NATURAL
    }
}

// --- the plugin ---------------------------------------------------------------

const CANVAS_SHADER: Handle<Shader> = uuid_handle!("6f0b6b8e-3c1d-4b8a-9d4e-97b10a3c9e51");

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, CANVAS_SHADER, "canvas.wgsl", Shader::from_wgsl);
        app.add_plugins(Material2dPlugin::<CanvasMaterial>::default())
            .init_resource::<Frame>()
            .init_resource::<Pool>()
            .add_systems(Startup, camera)
            .add_systems(
                PostUpdate,
                // After the screens have painted — they run in egui's
                // pass — and before anything reads where a layer is or
                // whether it shows, so a layer is drawn the frame it was
                // painted in.
                sync.after(EguiPostUpdateSet::EndPass)
                    .before(CameraUpdateSystems)
                    .before(TransformSystems::Propagate)
                    .before(VisibilitySystems::CheckVisibility),
            );
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Core2d,
                egui_after_bloom
                    .after(Core2dSystems::PostProcess)
                    .before(bevy_egui::render::egui_pass),
            );
        }
    }
}

/// Nothing, in the one place that makes egui's pass wait for the bloom and
/// the tonemapping: an edge in the render schedule, and nothing else.
fn egui_after_bloom() {}

/// The one camera: it clears the window to the void, draws the canvas's
/// layers, blooms them, and egui draws on it. Points are its units — the
/// projection's origin is the window's top-left and [`sync`] keeps its
/// scale at the UI's — and a layer's own transform turns y down.
fn camera(mut commands: Commands) {
    let mut camera = commands.spawn((
        Camera2d,
        // No multisampling: the feathering is the anti-aliasing, as it was
        // under egui, and a second pass of it would soften every edge.
        Msaa::Off,
        Projection::Orthographic(OrthographicProjection {
            viewport_origin: Vec2::new(0.0, 1.0),
            ..OrthographicProjection::default_2d()
        }),
        bevy_egui::PrimaryEguiContext,
    ));
    if bloom_on() {
        camera.insert((Hdr, Tonemapping::None, bloom()));
    }
}

// --- the material -------------------------------------------------------------

/// A layer's material: its clip, and the picture for one that has a
/// texture. A shape layer has no picture and no UVs, and the shader reads
/// no texture for it.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct CanvasMaterial {
    /// The canvas in physical pixels: min x, min y, max x, max y.
    #[uniform(0)]
    clip: Vec4,
    #[texture(1)]
    #[sampler(2)]
    picture: Option<Handle<Image>>,
}

impl Material2d for CanvasMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(CANVAS_SHADER)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    /// Premultiplied, as egui blends: every colour on the canvas is.
    fn specialize(
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(fragment) = &mut descriptor.fragment {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            }
        }
        Ok(())
    }
}

// --- a frame's layers ---------------------------------------------------------

/// One thing painted on the canvas, in points, over whatever was painted
/// before it this frame.
struct Layer {
    /// The canvas, in physical pixels — egui's scissor for it.
    clip: Vec4,
    positions: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    uvs: Option<Vec<[f32; 2]>>,
    indices: Vec<u32>,
    picture: Option<Handle<Image>>,
}

/// What the screen painted on the canvas this frame, in order, and how
/// big a point is: taken by [`sync`] every frame, so a screen that paints
/// nothing — the menu — leaves nothing standing.
#[derive(Resource, Default)]
pub struct Frame {
    layers: Vec<Layer>,
    pixels_per_point: Option<f32>,
}

/// The canvas as a screen paints on it: shapes and pictures, in the order
/// they are to be seen. The words go on afterwards, through egui, and are
/// over all of it.
#[derive(SystemParam)]
pub struct WorldCanvas<'w> {
    frame: ResMut<'w, Frame>,
    images: ResMut<'w, Assets<Image>>,
}

impl WorldCanvas<'_> {
    /// Paint `shapes`, in world units under `view`, into `rect` — clipped
    /// to it, so the canvas beside the panels stays out of them.
    pub fn shapes(&mut self, ctx: &egui::Context, rect: Rect, view: View, shapes: &[f32]) {
        let _timed = crate::perf::scope(crate::perf::Phase::Tessellate);
        crate::perf::tally(
            crate::perf::Count::Shapes,
            (shapes.len() / crate::shapes::STRIDE) as u64,
        );
        crate::perf::tally(crate::perf::Count::Floats, shapes.len() as u64);
        let ppp = ctx.pixels_per_point();
        let mut buf = ShapeBuf::new(rect, ppp);
        buf.replay(shapes, view);
        if buf.is_empty() {
            return;
        }
        let parts = buf.into_parts();
        self.push(
            ppp,
            Layer {
                clip: clip_of(rect, ppp),
                positions: parts.positions,
                colors: parts.colors,
                uvs: None,
                indices: parts.indices,
                picture: None,
            },
        );
    }

    /// Paint pieces of `picture` into `rect`: each a quad with its corners
    /// at window points — the piece's origin, then clockwise — showing the
    /// part of the picture between two UV corners. The fog.
    pub fn picture(
        &mut self,
        ctx: &egui::Context,
        rect: Rect,
        picture: Handle<Image>,
        pieces: &[([egui::Pos2; 4], [egui::Pos2; 4])],
    ) {
        if pieces.is_empty() {
            return;
        }
        let ppp = ctx.pixels_per_point();
        let mut layer = Layer {
            clip: clip_of(rect, ppp),
            positions: Vec::with_capacity(pieces.len() * 4),
            colors: vec![[1.0; 4]; pieces.len() * 4],
            uvs: Some(Vec::with_capacity(pieces.len() * 4)),
            indices: Vec::with_capacity(pieces.len() * 6),
            picture: Some(picture),
        };
        for (corners, uvs) in pieces {
            let first = layer.positions.len() as u32;
            layer
                .positions
                .extend(corners.iter().map(|p| [p.x, p.y, 0.0]));
            if let Some(out) = &mut layer.uvs {
                out.extend(uvs.iter().map(|uv| [uv.x, uv.y]));
            }
            layer.indices.extend_from_slice(&[
                first,
                first + 1,
                first + 2,
                first,
                first + 2,
                first + 3,
            ]);
        }
        self.push(ppp, layer);
    }

    /// The images a picture is kept in between frames.
    pub fn images(&mut self) -> &mut Assets<Image> {
        &mut self.images
    }

    fn push(&mut self, ppp: f32, layer: Layer) {
        self.frame.pixels_per_point = Some(ppp);
        self.frame.layers.push(layer);
    }
}

/// A canvas in points, as egui's scissor for it: physical pixels, rounded.
fn clip_of(rect: Rect, ppp: f32) -> Vec4 {
    Vec4::new(
        (rect.min.x * ppp).round(),
        (rect.min.y * ppp).round(),
        (rect.max.x * ppp).round(),
        (rect.max.y * ppp).round(),
    )
}

// --- onto Bevy's entities -----------------------------------------------------

/// One layer's entity, kept from frame to frame: its mesh is replaced every
/// frame and its material only when the clip or the picture moves.
struct Slot {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<CanvasMaterial>,
    clip: Vec4,
    picture: Option<AssetId<Image>>,
}

/// The layers' entities: the first `n` of them are this frame's layers, in
/// order, and the rest are hidden until a frame wants that many again.
#[derive(Resource, Default)]
struct Pool {
    slots: Vec<Slot>,
}

/// This frame's layers onto the entities that draw them, a z apart in the
/// order they were painted — and the camera's scale kept at a point.
#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    mut frame: ResMut<Frame>,
    mut pool: ResMut<Pool>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CanvasMaterial>>,
    mut placed: Query<(&mut Transform, &mut Visibility)>,
    mut projection: Query<&mut Projection, With<Camera2d>>,
    window: Query<&Window, With<PrimaryWindow>>,
) {
    let _timed = crate::perf::scope(crate::perf::Phase::Upload);
    // A point, in the projection's logical pixels: what the UI scale is.
    if let (Some(ppp), Ok(window)) = (frame.pixels_per_point, window.single())
        && ppp > 0.0
        && let Ok(mut projection) = projection.single_mut()
        && let Projection::Orthographic(o) = &mut *projection
    {
        let scale = window.scale_factor() / ppp;
        if o.scale != scale {
            o.scale = scale;
        }
    }
    let layers = std::mem::take(&mut frame.layers);
    let used = layers.len();
    for (i, layer) in layers.into_iter().enumerate() {
        if i == pool.slots.len() {
            let mesh = meshes.add(Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            ));
            let material = materials.add(CanvasMaterial {
                clip: layer.clip,
                picture: layer.picture.clone(),
            });
            let entity = commands
                .spawn((
                    Mesh2d(mesh.clone()),
                    MeshMaterial2d(material.clone()),
                    placement(i),
                    Visibility::Visible,
                    NoFrustumCulling,
                ))
                .id();
            pool.slots.push(Slot {
                entity,
                mesh,
                material,
                clip: layer.clip,
                picture: layer.picture.as_ref().map(|h| h.id()),
            });
        }
        let slot = &mut pool.slots[i];
        let picture = layer.picture.as_ref().map(|h| h.id());
        if slot.clip != layer.clip || slot.picture != picture {
            if let Some(mut material) = materials.get_mut(&slot.material) {
                material.clip = layer.clip;
                material.picture = layer.picture.clone();
            }
            slot.clip = layer.clip;
            slot.picture = picture;
        }
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, layer.positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, layer.colors);
        if let Some(uvs) = layer.uvs {
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
        }
        let mesh = mesh.with_inserted_indices(Indices::U32(layer.indices));
        let _ = meshes.insert(slot.mesh.id(), mesh);
        if let Ok((mut transform, mut visibility)) = placed.get_mut(slot.entity) {
            transform.set_if_neq(placement(i));
            visibility.set_if_neq(Visibility::Visible);
        }
    }
    for slot in &pool.slots[used..] {
        if let Ok((_, mut visibility)) = placed.get_mut(slot.entity) {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
}

/// Where layer `i` sits: at the window's top-left, y turned down so its
/// points are egui's, and `i` towards the viewer, so a later layer is
/// drawn over an earlier one.
fn placement(i: usize) -> Transform {
    Transform::from_xyz(0.0, 0.0, i as f32).with_scale(Vec3::new(1.0, -1.0, 1.0))
}
