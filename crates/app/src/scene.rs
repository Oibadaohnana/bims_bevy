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
//!
//! **A layer of shapes is not tessellated** (task 121): each shape goes to
//! the GPU as one record (`shapes::pack`), sixteen floats in a storage
//! buffer the layer keeps, and `shape.wgsl` draws a quad a record and
//! works out in each pixel how much of it the feathered triangles would
//! have covered ([`ShapeMaterial`]). The quads are a mesh made once for a
//! power of two of them and shared by every layer that size
//! ([`QuadMeshes`]), and the buffer keeps its size while the count stays
//! under it, so a frame hands the GPU the records and nothing else — where
//! it used to hand Bevy a new mesh of every triangle, which the render
//! thread then allocated room for and copied. **`BIMS_SHAPES=cpu`** draws
//! them the old way, through `shapes::ShapeBuf` and a mesh a frame, for
//! comparing the two.

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
use bevy::render::storage::ShaderBuffer;
use bevy::render::view::Msaa;
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin};
use bevy::transform::TransformSystems;
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiPostUpdateSet, egui};

use crate::shapes::{RECORD, Record, Rect, ShapeBuf, View};

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
const SHAPE_SHADER: Handle<Shader> = uuid_handle!("2c7d4f1e-9a3b-4e58-b6c2-1d8e5f7a0b93");

/// `BIMS_SHAPES=cpu`: the world canvas's shapes tessellated on the CPU
/// into a mesh a frame, as before task 121 — for comparing the two
/// pictures and the two costs. Anything else, or nothing, is the GPU's
/// records. Read once.
pub fn shapes_on_gpu() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_SHAPES").as_deref() != Ok("cpu"))
}

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        load_internal_asset!(app, CANVAS_SHADER, "canvas.wgsl", Shader::from_wgsl);
        load_internal_asset!(app, SHAPE_SHADER, "shape.wgsl", Shader::from_wgsl);
        crate::particles::build(app);
        app.add_plugins(Material2dPlugin::<CanvasMaterial>::default())
            .add_plugins(Material2dPlugin::<ShapeMaterial>::default())
            .init_resource::<Frame>()
            .init_resource::<Pool>()
            .init_resource::<QuadMeshes>()
            .add_systems(Startup, camera)
            .add_systems(Startup, crate::surfaces::load)
            .add_systems(First, crate::surfaces::wind)
            .add_systems(
                PostUpdate,
                // After the screens have painted — they run in egui's
                // pass — and before anything reads where a layer is or
                // whether it shows, so a layer is drawn the frame it was
                // painted in. The particles' layer after the rest, once
                // `sync` has said where it goes.
                (sync, crate::particles::sync)
                    .chain()
                    .after(EguiPostUpdateSet::EndPass)
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

/// A layer of shapes, drawn on the GPU (`shape.wgsl`): its clip, and the
/// storage buffer its records are in — a count, then the records.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ShapeMaterial {
    /// The canvas in physical pixels: min x, min y, max x, max y.
    #[uniform(0)]
    clip: Vec4,
    #[storage(1, read_only)]
    shapes: Handle<ShaderBuffer>,
    /// The surfaces' texture array (`surfaces.rs`), the same for every layer.
    #[texture(2, dimension = "2d_array")]
    #[sampler(3)]
    surfaces: Handle<Image>,
}

impl Material2d for ShapeMaterial {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(SHAPE_SHADER)
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(SHAPE_SHADER)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    /// Premultiplied, as egui blends — the same blend as a mesh layer's.
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

/// The fewest records a layer's buffer and quads are made for: a power of
/// two, as every size after it is.
const MIN_QUADS: usize = 64;

/// How many records a layer of `n` is given room for: the power of two at
/// or over it, so a count that wanders keeps its buffer and its quads.
fn room_for(n: usize) -> usize {
    n.max(MIN_QUADS).next_power_of_two()
}

/// A quad a record, for every size of layer made so far: `n` quads whose
/// corners are -1 or 1 each way and whose third coordinate is the record
/// the quad draws. Made once for a size and shared by every layer of it.
#[derive(Resource, Default)]
struct QuadMeshes {
    by_size: Vec<(usize, Handle<Mesh>)>,
}

impl QuadMeshes {
    fn of(&mut self, meshes: &mut Assets<Mesh>, n: usize) -> Handle<Mesh> {
        if let Some((_, handle)) = self.by_size.iter().find(|(size, _)| *size == n) {
            return handle.clone();
        }
        let mut positions = Vec::with_capacity(n * 4);
        let mut indices = Vec::with_capacity(n * 6);
        for q in 0..n {
            let z = q as f32;
            positions.extend_from_slice(&[
                [-1.0, -1.0, z],
                [1.0, -1.0, z],
                [1.0, 1.0, z],
                [-1.0, 1.0, z],
            ]);
            let first = (q * 4) as u32;
            indices.extend_from_slice(&[first, first + 1, first + 2, first, first + 2, first + 3]);
        }
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices));
        let handle = meshes.add(mesh);
        self.by_size.push((n, handle.clone()));
        handle
    }
}

/// A layer's records as the storage buffer holds them: the count, three
/// words of nothing, then `room` records — the ones past the count left
/// nought, which the vertex stage folds away.
fn record_bytes(records: &[Record], room: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; 16 + room * RECORD * 4];
    bytes[..4].copy_from_slice(&(records.len() as u32).to_le_bytes());
    for (out, v) in bytes[16..]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(records.iter().flatten())
    {
        *out = v.to_le_bytes();
    }
    bytes
}

// --- a frame's layers ---------------------------------------------------------

/// One thing painted on the canvas, in points, over whatever was painted
/// before it this frame.
enum Layer {
    /// Triangles: the fog's pictures, and the shapes under
    /// `BIMS_SHAPES=cpu`.
    Mesh(MeshLayer),
    /// Shapes, a record each, for `shape.wgsl`.
    Shapes { clip: Vec4, records: Vec<Record> },
    /// The fight's particles (`particles.rs`): one layer at most, its
    /// sprays kept in `particles::Particles`.
    Particles { clip: Vec4 },
}

struct MeshLayer {
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
    /// The crew's light map for the GPU to draw this frame, if the room
    /// handed one over (`lightmap.rs`, task 121).
    light: ResMut<'w, crate::lightmap::LightJob>,
    /// The fight's particles, simulated on the GPU (`particles.rs`).
    particles: ResMut<'w, crate::particles::Particles>,
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
        if shapes_on_gpu() {
            let mut records = Vec::new();
            crate::shapes::pack(shapes, view, rect, ppp, &mut records);
            if records.is_empty() {
                return;
            }
            let clip = clip_of(rect, ppp);
            self.push(ppp, Layer::Shapes { clip, records });
            return;
        }
        let mut buf = ShapeBuf::new(rect, ppp);
        buf.replay(shapes, view);
        if buf.is_empty() {
            return;
        }
        let parts = buf.into_parts();
        self.push(
            ppp,
            Layer::Mesh(MeshLayer {
                clip: clip_of(rect, ppp),
                positions: parts.positions,
                colors: parts.colors,
                uvs: None,
                indices: parts.indices,
                picture: None,
            }),
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
        let mut layer = MeshLayer {
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
        self.push(ppp, Layer::Mesh(layer));
    }

    /// Paint the fight's particles into `rect`, over whatever was painted
    /// before them: the clock on by `dt` real seconds (nought while
    /// paused), `sprays` — spawned since the last frame, in world units —
    /// added to the ring, and every live one drawn under `view` by the
    /// GPU (`particles.rs`).
    pub fn particles(
        &mut self,
        ctx: &egui::Context,
        rect: Rect,
        view: View,
        dt: f32,
        sprays: &[bims::fx::Spray],
    ) {
        self.particles.feed(dt, sprays);
        self.particles.set_view(view.scale, view.offset + rect.min);
        let ppp = ctx.pixels_per_point();
        let clip = clip_of(rect, ppp);
        self.push(ppp, Layer::Particles { clip });
    }

    /// The images a picture is kept in between frames.
    pub fn images(&mut self) -> &mut Assets<Image> {
        &mut self.images
    }

    /// Hand the GPU the crew's light map to draw this frame into
    /// `picture` (`lightmap.rs`, task 121).
    pub fn light_job(
        &mut self,
        inputs: std::sync::Arc<bims::sight::LightInputs>,
        picture: AssetId<Image>,
    ) {
        self.light.inputs = Some(inputs);
        self.light.picture = Some(picture);
    }

    /// Hand the GPU a map the CPU worked out, to blur and colour into its
    /// picture this frame (`lightmap.rs`, task 140).
    pub fn raw_light_job(&mut self, job: crate::lightmap::RawJob) {
        self.light.raw.push(job);
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

/// A mesh layer's entity, kept from frame to frame: its mesh is replaced
/// every frame and its material only when the clip or the picture moves.
struct Slot {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<CanvasMaterial>,
    clip: Vec4,
    picture: Option<AssetId<Image>>,
}

/// A shape layer's entity, kept from frame to frame: its records are
/// written into its buffer every frame, and its quads, its buffer's size
/// and its material change only when the count outgrows the room or the
/// clip moves.
struct ShapeSlot {
    entity: Entity,
    buffer: Handle<ShaderBuffer>,
    material: Handle<ShapeMaterial>,
    clip: Vec4,
    room: usize,
    /// Frames the material is still to be made again for: two after the
    /// buffer changed size. Bevy puts a material's bind group together
    /// with whatever buffer the render world holds for it at the time,
    /// and nothing orders that after the buffer's own upload, so the
    /// frame the buffer grows the bind group may be made over the old
    /// one; made again the frame after, it is over the new one for good.
    rebind: u8,
}

/// The layers' entities, one pool a kind: the first so many of each are
/// this frame's layers of that kind, and the rest are hidden until a frame
/// wants that many again. Which is drawn over which is their z, the order
/// they were painted in, whichever pool they are in.
#[derive(Resource, Default)]
struct Pool {
    slots: Vec<Slot>,
    shapes: Vec<ShapeSlot>,
}

/// This frame's layers onto the entities that draw them, a z apart in the
/// order they were painted — and the camera's scale kept at a point.
#[allow(clippy::too_many_arguments)]
fn sync(
    mut commands: Commands,
    mut frame: ResMut<Frame>,
    mut pool: ResMut<Pool>,
    mut quads: ResMut<QuadMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<CanvasMaterial>>,
    mut shape_materials: ResMut<Assets<ShapeMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    surfaces: Res<crate::surfaces::SurfaceTextures>,
    mut placed: Query<(&mut Transform, &mut Visibility, &mut Mesh2d)>,
    mut projection: Query<&mut Projection, With<Camera2d>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut particles: ResMut<crate::particles::Particles>,
) {
    particles.placed = None;
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
    let (mut used, mut used_shapes) = (0, 0);
    for (z, layer) in layers.into_iter().enumerate() {
        match layer {
            Layer::Mesh(layer) => {
                mesh_layer(
                    &mut commands,
                    &mut pool.slots,
                    used,
                    z,
                    layer,
                    &mut meshes,
                    &mut materials,
                    &mut placed,
                );
                used += 1;
            }
            Layer::Shapes { clip, records } => {
                shape_layer(
                    &mut commands,
                    &mut pool.shapes,
                    used_shapes,
                    z,
                    clip,
                    &records,
                    (&mut quads, &mut meshes),
                    (&mut shape_materials, &surfaces.0),
                    &mut buffers,
                    &mut placed,
                );
                used_shapes += 1;
            }
            Layer::Particles { clip } => particles.placed = Some((z, clip)),
        }
    }
    // A frame with no shapes at all (the menus, the map) lets the shape
    // slots and the shared quad meshes go, and the next frame with some
    // makes them afresh. A quad mesh lives on the GPU alone
    // (`RENDER_WORLD`), and once nothing drew with it for a while — the
    // menus between two runs — its GPU copy was gone while the cache still
    // handed out its handle: the next run drew no deck at all.
    if used_shapes == 0 {
        for slot in pool.shapes.drain(..) {
            commands.entity(slot.entity).despawn();
            shape_materials.remove(&slot.material);
            buffers.remove(&slot.buffer);
        }
        for (_, mesh) in quads.by_size.drain(..) {
            meshes.remove(&mesh);
        }
    }
    let idle = pool.slots[used..].iter().map(|s| s.entity);
    let idle = idle.chain(pool.shapes[used_shapes..].iter().map(|s| s.entity));
    for entity in idle {
        if let Ok((_, mut visibility, _)) = placed.get_mut(entity) {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
}

/// A layer of triangles onto the `i`th mesh slot, at `z`.
#[allow(clippy::too_many_arguments)]
fn mesh_layer(
    commands: &mut Commands,
    slots: &mut Vec<Slot>,
    i: usize,
    z: usize,
    layer: MeshLayer,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<CanvasMaterial>,
    placed: &mut Query<(&mut Transform, &mut Visibility, &mut Mesh2d)>,
) {
    if i == slots.len() {
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
                placement(z),
                Visibility::Visible,
                NoFrustumCulling,
            ))
            .id();
        slots.push(Slot {
            entity,
            mesh,
            material,
            clip: layer.clip,
            picture: layer.picture.as_ref().map(|h| h.id()),
        });
    }
    let slot = &mut slots[i];
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
    if let Ok((mut transform, mut visibility, _)) = placed.get_mut(slot.entity) {
        transform.set_if_neq(placement(z));
        visibility.set_if_neq(Visibility::Visible);
    }
}

/// A layer of shape records onto the `i`th shape slot, at `z`.
#[allow(clippy::too_many_arguments)]
fn shape_layer(
    commands: &mut Commands,
    slots: &mut Vec<ShapeSlot>,
    i: usize,
    z: usize,
    clip: Vec4,
    records: &[Record],
    (quads, meshes): (&mut QuadMeshes, &mut Assets<Mesh>),
    (materials, surfaces): (&mut Assets<ShapeMaterial>, &Handle<Image>),
    buffers: &mut Assets<ShaderBuffer>,
    placed: &mut Query<(&mut Transform, &mut Visibility, &mut Mesh2d)>,
) {
    let room = room_for(records.len());
    let bytes = record_bytes(records, room);
    if i == slots.len() {
        let buffer = buffers.add(ShaderBuffer::new(&bytes, RenderAssetUsages::RENDER_WORLD));
        let material = materials.add(ShapeMaterial {
            clip,
            shapes: buffer.clone(),
            surfaces: surfaces.clone(),
        });
        let entity = commands
            .spawn((
                Mesh2d(quads.of(meshes, room)),
                MeshMaterial2d(material.clone()),
                placement(z),
                Visibility::Visible,
                NoFrustumCulling,
            ))
            .id();
        slots.push(ShapeSlot {
            entity,
            buffer,
            material,
            clip,
            room,
            rebind: 2,
        });
        return;
    }
    let slot = &mut slots[i];
    if let Some(mut buffer) = buffers.get_mut(&slot.buffer) {
        buffer.data = Some(bytes);
    }
    // A buffer of another size is another buffer on the GPU, and the bind
    // group has to be made again over it; so does a clip that moved.
    let regrown = slot.room != room;
    if regrown {
        slot.rebind = 2;
    }
    if slot.rebind > 0 || slot.clip != clip {
        if let Some(mut material) = materials.get_mut(&slot.material) {
            material.clip = clip;
        }
        slot.clip = clip;
        slot.rebind = slot.rebind.saturating_sub(1);
    }
    if let Ok((mut transform, mut visibility, mut mesh)) = placed.get_mut(slot.entity) {
        if regrown {
            mesh.0 = quads.of(meshes, room);
            slot.room = room;
        }
        // Bevy keeps a blended mesh's place in the draw order from the
        // frame it was queued, and queues it again only when its mesh or
        // material changes — never for a transform. A layer that moved up
        // (the station's backdrop came in under the deck a few frames into
        // a run) stayed at its old z, under the backdrop: no deck until a
        // zoom grew the buffer. So a move is told as a change of mesh.
        if transform.set_if_neq(placement(z)) {
            mesh.set_changed();
        }
        visibility.set_if_neq(Visibility::Visible);
    }
}

/// Where layer `i` sits: at the window's top-left, y turned down so its
/// points are egui's, and `i` towards the viewer, so a later layer is
/// drawn over an earlier one.
fn placement(i: usize) -> Transform {
    Transform::from_xyz(0.0, 0.0, i as f32).with_scale(Vec3::new(1.0, -1.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layer's buffer is what `shape.wgsl` reads: the count in the first
    /// word, three words of nothing, then the records, sixteen floats each,
    /// and nought for the room past them; and the room is a power of two,
    /// so a count that wanders keeps its buffer.
    #[test]
    fn a_layer_s_buffer_is_the_count_then_the_records_in_a_power_of_two() {
        assert_eq!(room_for(0), MIN_QUADS);
        assert_eq!(room_for(MIN_QUADS + 1), MIN_QUADS * 2);
        assert_eq!(room_for(15_400), 16_384);
        let mut record = [0.0; RECORD];
        record[0] = crate::shapes::REC_RECT_FILL;
        record[15] = 0.5;
        let bytes = record_bytes(&[record, record], 4);
        assert_eq!(bytes.len(), 16 + 4 * RECORD * 4);
        assert_eq!(
            &bytes[..16],
            &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );
        let float = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(float(16), crate::shapes::REC_RECT_FILL);
        assert_eq!(float(16 + 15 * 4), 0.5);
        assert_eq!(float(16 + RECORD * 4), crate::shapes::REC_RECT_FILL);
        assert!(bytes[16 + 2 * RECORD * 4..].iter().all(|&b| b == 0));
    }
}
