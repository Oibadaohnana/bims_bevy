//! The crew's light map, drawn on the GPU (task 121).
//!
//! `bims::sight` works the light map out on the CPU — every body's eyes
//! marched four thousand rays at a time, the views put together with the
//! lamps under the one fog — and `fogmap.rs` blurs it into the fog's
//! texture. With the host drawing it (`sight::set_host_draws`, every run
//! unless `BIMS_LIGHTMAP=cpu`) the room hands over what the march starts
//! from instead (`sight::LightInputs`, on `LightMap::inputs`), and this
//! does the march, the composing, the blur and the colouring in three
//! compute passes (`lightmap.wgsl`) straight into the fog's texture, in the
//! render world, before the main pass of the same frame. The picture is
//! the CPU's byte for byte — the note at the top of the shader says why —
//! and **`BIMS_LIGHTMAP=check`** works both out every frame and counts the
//! bytes that differ, printed when a smoke run exits.
//!
//! What stays on the CPU: which bodies moved (a body that did not is not
//! marched again, the CPU's own rule), each ray's first two crossings (the
//! one division a ray makes), the lamps' light (rebuilt when a lamp
//! changes), and the tables every float the composing reads comes out of.
//!
//! **Nothing is kept here that the room needs back.** The fog is one fog
//! over everything nobody sees (task 128) — there is no memory of what
//! has been looked at — so the GPU's map is a picture of this frame's
//! inputs and nothing else, and nothing is read back before a save.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::diagnostic::{DiagnosticsRecorder, RecordDiagnostics};
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingType, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, CommandEncoder, CommandEncoderDescriptor,
    ComputePassDescriptor, ComputePipeline, Extent3d, MapMode, Origin3d,
    PipelineCompilationOptions, PipelineLayoutDescriptor, PollType, RawComputePipelineDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, TexelCopyBufferInfo, TexelCopyBufferLayout,
    TexelCopyTextureInfo, TextureAspect, TextureId,
};
use bevy::render::renderer::{RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue};
use bevy::render::texture::GpuImage;
use bims::sight::{LightInputs, ray_table};

/// Who draws the crew's light map: `BIMS_LIGHTMAP=cpu` the room, as it
/// always did; `=check` both, compared; anything else, or nothing, the GPU.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Cpu,
    Gpu,
    Check,
}

pub fn mode() -> Mode {
    static MODE: std::sync::OnceLock<Mode> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| match std::env::var("BIMS_LIGHTMAP").as_deref() {
        Ok("cpu") => Mode::Cpu,
        Ok("check") => Mode::Check,
        _ => Mode::Gpu,
    })
}

/// The job this frame: what the room handed over, and the fog's picture
/// to draw it into. Emptied at the start of every frame, so a frame whose
/// screen draws no fog draws nothing here.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct LightJob {
    pub inputs: Option<Arc<LightInputs>>,
    pub picture: Option<AssetId<Image>>,
}

pub struct LightMapPlugin;

impl Plugin for LightMapPlugin {
    fn build(&self, app: &mut App) {
        let mode = mode();
        bims::sight::set_host_draws(mode != Mode::Cpu, mode == Mode::Check);
        app.init_resource::<LightJob>()
            .add_plugins(ExtractResourcePlugin::<LightJob>::default())
            .add_systems(First, empty_the_job);
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<GpuLight>()
                // In the render graph's own frame, before anything is drawn:
                // the fog's texture is filled before the main pass reads it,
                // and Bevy's GPU timings (`BIMS_PERF`) can time the passes.
                .add_systems(
                    RenderGraph,
                    draw_light_map
                        .after(RenderGraphSystems::Begin)
                        .before(RenderGraphSystems::Render),
                );
        }
    }
}

fn empty_the_job(mut job: ResMut<LightJob>) {
    *job = LightJob::default();
}

/// A buffer's first `words` words, read back to the CPU, waiting for it.
fn read_back(
    device: &RenderDevice,
    queue: &RenderQueue,
    buffer: &Buffer,
    words: usize,
) -> Vec<u32> {
    let size = (words.max(1) * 4) as u64;
    let read = device.create_buffer(&BufferDescriptor {
        label: Some("light map: read back"),
        size,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("light map: read back"),
    });
    encoder.copy_buffer_to_buffer(buffer, 0, &read, 0, size);
    queue.submit([encoder.finish()]);
    let slice = read.slice(..);
    slice.map_async(MapMode::Read, |_| {});
    let _ = device.poll(PollType::wait_indefinitely());
    let data = slice.get_mapped_range();
    let words = data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    drop(data);
    read.unmap();
    words
}

// --- the check ----------------------------------------------------------------

static CHECKED: AtomicU64 = AtomicU64::new(0);
static MAP_DIFF: AtomicU64 = AtomicU64::new(0);
static TEXEL_DIFF: AtomicU64 = AtomicU64::new(0);

/// What `BIMS_LIGHTMAP=check` found, as the line a smoke run prints: how
/// many frames were compared, and how many bytes of the map and of the
/// texture differed in all.
pub fn report() -> Vec<String> {
    if mode() != Mode::Check {
        return Vec::new();
    }
    vec![format!(
        "lightmap check: {} frames, {} map bytes, {} texel bytes differing",
        CHECKED.load(Ordering::Relaxed),
        MAP_DIFF.load(Ordering::Relaxed),
        TEXEL_DIFF.load(Ordering::Relaxed),
    )]
}

// --- the render world ---------------------------------------------------------

/// The pipelines and the two tables that never change, made the first
/// time a map is drawn; and the one map being drawn.
#[derive(Resource, Default)]
struct GpuLight {
    kit: Option<Kit>,
    map: Option<MapState>,
}

struct Kit {
    layout: BindGroupLayout,
    march: ComputePipeline,
    compose: ComputePipeline,
    blur: ComputePipeline,
    rays: Buffer,
    colours: Buffer,
}

/// An infinite crossing, as the GPU is handed it: the largest float,
/// which no ray's crossing ever reaches and is never added to (a ray
/// that never crosses a way never steps it), so the walk takes the same
/// turns an infinity took on the CPU.
fn finite(v: f32) -> f32 {
    if v.is_finite() { v } else { f32::MAX }
}

fn floats(values: impl IntoIterator<Item = f32>) -> Vec<u8> {
    values.into_iter().flat_map(f32::to_le_bytes).collect()
}

fn words(values: impl IntoIterator<Item = u32>) -> Vec<u8> {
    values.into_iter().flat_map(u32::to_le_bytes).collect()
}

impl Kit {
    fn new(device: &RenderDevice, queue: &RenderQueue) -> Kit {
        let module = device.create_and_validate_shader_module(ShaderModuleDescriptor {
            label: Some("light map"),
            source: ShaderSource::Wgsl(include_str!("lightmap.wgsl").into()),
        });
        let entry = |binding: u32, ty: BufferBindingType| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let read = BufferBindingType::Storage { read_only: true };
        let write = BufferBindingType::Storage { read_only: false };
        let entries = [
            entry(0, BufferBindingType::Uniform),
            entry(1, read),
            entry(2, read),
            entry(3, write),
            entry(4, write),
            entry(5, write),
            entry(6, read),
            entry(7, read),
            entry(8, read),
            entry(9, read),
            entry(10, read),
            entry(11, read),
        ];
        let layout = device.create_bind_group_layout("light map", &entries);
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("light map"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry: &str| {
            device.create_compute_pipeline(&RawComputePipelineDescriptor {
                label: Some("light map"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some(entry),
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let rays = ray_table();
        let rays = constant(
            device,
            queue,
            "light map: rays",
            &floats(
                rays.iter()
                    .flat_map(|r| [r[0], r[1], finite(r[2]), finite(r[3])]),
            ),
        );
        let colours = constant(
            device,
            queue,
            "light map: colours",
            &words((0..=255u8).flat_map(|a| {
                (0..=255u8).map(move |g| u32::from_le_bytes(crate::fogmap::texel(a, g)))
            })),
        );
        Kit {
            march: pipeline("march"),
            compose: pipeline("compose"),
            blur: pipeline("blur"),
            layout,
            rays,
            colours,
        }
    }
}

/// A storage buffer holding `bytes`, written once.
fn constant(device: &RenderDevice, queue: &RenderQueue, label: &str, bytes: &[u8]) -> Buffer {
    let buffer = storage(device, label, bytes.len());
    queue.write_buffer(&buffer, 0, bytes);
    buffer
}

/// A storage buffer of at least `bytes`, never nought.
fn storage(device: &RenderDevice, label: &str, bytes: usize) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some(label),
        size: bytes.max(16).next_multiple_of(4) as u64,
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// The bytes one marched eye takes in the eyes' buffer (`Eye` in the
/// shader), and its rays' first crossings in the starts' buffer.
const EYE_BYTES: usize = 48;
const RAYS: usize = 4096;
const START_BYTES: usize = RAYS * 8;

/// One sight's map as it is drawn here: every buffer the passes read and
/// write, and what each was last made from, so a frame does only what
/// changed.
struct MapState {
    sight: u64,
    width: usize,
    height: usize,
    words: usize,
    tile_words: usize,
    stride: usize,
    params: Buffer,
    cells: Buffer,
    fields: Buffer,
    seen: Buffer,
    map: Buffer,
    texels: Buffer,
    eyes: Buffer,
    starts: Buffer,
    near: Buffer,
    tables: Buffer,
    /// How many bodies' views the seen and near buffers hold, and how
    /// many eyes the eyes and starts buffers.
    slots: usize,
    eye_room: usize,
    bind: Option<BindGroup>,
    /// The revision of each body's view marched into its bits.
    revs: Vec<u64>,
    cells_rev: Option<u64>,
    fields_rev: Option<u64>,
    views: usize,
    /// The texture the colours were last copied into: a picture made
    /// again (the same asset, a new texture) wants them again.
    texture: Option<TextureId>,
}

impl MapState {
    fn new(device: &RenderDevice, inputs: &LightInputs) -> MapState {
        let (w, h) = (inputs.width, inputs.height);
        let words = (w * h).div_ceil(32);
        let tile_words = (inputs.columns * inputs.rows).div_ceil(32);
        let stride = w.next_multiple_of(64);
        let slots = inputs.views.len().max(1);
        MapState {
            sight: inputs.sight,
            width: w,
            height: h,
            words,
            tile_words,
            stride,
            params: device.create_buffer(&BufferDescriptor {
                label: Some("light map: params"),
                size: 48,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            cells: storage(device, "light map: cells", inputs.columns * inputs.rows * 4),
            fields: storage(device, "light map: fields", w * h * 4),
            seen: storage(device, "light map: seen", slots * words * 4),
            map: storage(device, "light map: map", w * h * 4),
            texels: storage(device, "light map: texels", stride * h * 4),
            eyes: storage(device, "light map: eyes", EYE_BYTES * 8),
            starts: storage(device, "light map: starts", START_BYTES * 8),
            near: storage(device, "light map: near", slots * tile_words * 4),
            tables: storage(device, "light map: tables", 768 * 4),
            slots,
            eye_room: 8,
            bind: None,
            revs: Vec::new(),
            cells_rev: None,
            fields_rev: None,
            views: usize::MAX,
            texture: None,
        }
    }

    /// Room for `n` bodies' views, keeping the bits already marched.
    fn room_for_views(&mut self, device: &RenderDevice, encoder: &mut CommandEncoder, n: usize) {
        if n <= self.slots {
            return;
        }
        let slots = n.next_power_of_two();
        let seen = storage(device, "light map: seen", slots * self.words * 4);
        encoder.copy_buffer_to_buffer(
            &self.seen,
            0,
            &seen,
            0,
            (self.slots * self.words * 4) as u64,
        );
        let near = storage(device, "light map: near", slots * self.tile_words * 4);
        encoder.copy_buffer_to_buffer(
            &self.near,
            0,
            &near,
            0,
            (self.slots * self.tile_words * 4) as u64,
        );
        self.seen = seen;
        self.near = near;
        self.slots = slots;
        self.bind = None;
    }

    /// Room for `n` marched eyes this frame.
    fn room_for_eyes(&mut self, device: &RenderDevice, n: usize) {
        if n <= self.eye_room {
            return;
        }
        let room = n.next_power_of_two();
        self.eyes = storage(device, "light map: eyes", EYE_BYTES * room);
        self.starts = storage(device, "light map: starts", START_BYTES * room);
        self.eye_room = room;
        self.bind = None;
    }

    fn bind_group(&mut self, device: &RenderDevice, kit: &Kit) -> &BindGroup {
        self.bind.get_or_insert_with(|| {
            let buffers = [
                &self.params,
                &self.cells,
                &self.fields,
                &self.seen,
                &self.map,
                &self.texels,
                &self.eyes,
                &self.starts,
                &self.near,
                &kit.rays,
                &kit.colours,
                &self.tables,
            ];
            let entries: Vec<BindGroupEntry> = buffers
                .iter()
                .enumerate()
                .map(|(binding, buffer)| BindGroupEntry {
                    binding: binding as u32,
                    resource: buffer.as_entire_binding(),
                })
                .collect();
            device.create_bind_group("light map", &kit.layout, &entries)
        })
    }
}

fn words_of(values: &[u32]) -> Vec<u8> {
    words(values.iter().copied())
}

/// The frame's light map, if the room handed one over: what changed put
/// on the GPU, the eyes that moved marched, the map composed, blurred and
/// coloured into the fog's texture — before the main pass that draws it.
fn draw_light_map(
    job: Res<LightJob>,
    mut gpu: ResMut<GpuLight>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    images: Res<RenderAssets<GpuImage>>,
    recorder: Option<Res<DiagnosticsRecorder>>,
) {
    let (Some(inputs), Some(picture)) = (&job.inputs, job.picture) else {
        return;
    };
    let Some(image) = images.get(picture) else {
        return;
    };
    if image.texture_descriptor.size.width as usize != inputs.width
        || image.texture_descriptor.size.height as usize != inputs.height
    {
        return;
    }
    let GpuLight { kit, map } = &mut *gpu;
    let kit = kit.get_or_insert_with(|| Kit::new(&device, &queue));
    let fresh = !map.as_ref().is_some_and(|m| {
        m.sight == inputs.sight && m.width == inputs.width && m.height == inputs.height
    });
    if fresh {
        *map = Some(MapState::new(&device, inputs));
    }
    let Some(m) = map.as_mut() else {
        return;
    };
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("light map"),
    });
    m.room_for_views(&device, &mut encoder, inputs.views.len());
    let mut compose = fresh || m.views != inputs.views.len();
    m.views = inputs.views.len();
    if m.cells_rev != Some(inputs.cells_rev) {
        let cells: Vec<u32> = inputs.cells.iter().map(|&c| c as u32).collect();
        queue.write_buffer(&m.cells, 0, &words_of(&cells));
        m.cells_rev = Some(inputs.cells_rev);
        compose = true;
    }
    if m.fields_rev != Some(inputs.fields_rev) {
        let fields: Vec<u32> = inputs
            .light_field
            .iter()
            .zip(inputs.shown_field.iter())
            .map(|(&l, &s)| l as u32 | (s as u32) << 8)
            .collect();
        queue.write_buffer(&m.fields, 0, &words_of(&fields));
        m.fields_rev = Some(inputs.fields_rev);
        compose = true;
    }
    // The bodies whose eyes moved: their bits cleared and their eyes
    // marched afresh.
    m.revs.resize(inputs.views.len(), u64::MAX);
    let mut eyes = Vec::new();
    let mut starts = Vec::new();
    for (slot, view) in inputs.views.iter().enumerate() {
        if m.revs[slot] == view.rev {
            continue;
        }
        m.revs[slot] = view.rev;
        let at = (slot * m.words * 4) as u64;
        encoder.clear_buffer(&m.seen, at, Some((m.words * 4) as u64));
        queue.write_buffer(
            &m.near,
            (slot * m.tile_words * 4) as u64,
            &words_of(&view.near),
        );
        for eye in &view.eyes {
            let (beyond_tile, beyond_dir) = eye.beyond.unwrap_or(((0, 0), (0, 0)));
            let flags = eye.from_opaque as u32 | (eye.beyond.is_some() as u32) << 1;
            for v in [
                eye.start.0,
                eye.start.1,
                eye.from_tile.0,
                eye.from_tile.1,
                beyond_tile.0,
                beyond_tile.1,
                beyond_dir.0,
                beyond_dir.1,
            ] {
                eyes.extend_from_slice(&v.to_le_bytes());
            }
            eyes.extend_from_slice(&flags.to_le_bytes());
            eyes.extend_from_slice(&(slot as u32).to_le_bytes());
            eyes.extend_from_slice(&eye.far.to_le_bytes());
            eyes.extend_from_slice(&0u32.to_le_bytes());
            starts.extend(floats(
                eye.t0.iter().flat_map(|t| [finite(t[0]), finite(t[1])]),
            ));
        }
        compose = true;
    }
    let marched = eyes.len() / EYE_BYTES;
    m.room_for_eyes(&device, marched);
    if marched > 0 {
        queue.write_buffer(&m.eyes, 0, &eyes);
        queue.write_buffer(&m.starts, 0, &starts);
    }
    let params = [
        m.width as u32,
        m.height as u32,
        inputs.columns as u32,
        inputs.views.len() as u32,
        m.words as u32,
        m.tile_words as u32,
        marched as u32,
        m.stride as u32,
        inputs.fog as u32,
        0,
        0,
        0,
    ];
    queue.write_buffer(&m.params, 0, &words_of(&params));
    let tables: Vec<u32> = inputs
        .dark
        .iter()
        .chain(&inputs.glow_seen)
        .chain(&inputs.glow_fog)
        .map(|&v| v as u32)
        .collect();
    queue.write_buffer(&m.tables, 0, &words_of(&tables));
    let recolour = compose || m.texture != Some(image.texture.id());
    let checking = inputs.cpu.is_some();
    if !recolour && !checking {
        return;
    }
    let (gx, gy) = (m.width.div_ceil(8) as u32, m.height.div_ceil(8) as u32);
    let bind = m.bind_group(&device, kit).clone();
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("light map"),
            timestamp_writes: None,
        });
        // Timed on the GPU under `BIMS_PERF`, as Bevy times its own passes.
        let span = recorder
            .as_ref()
            .map(|r| r.pass_span(&mut pass, "light map"));
        pass.set_bind_group(0, &bind, &[]);
        if marched > 0 {
            pass.set_pipeline(&kit.march);
            pass.dispatch_workgroups((RAYS / 64) as u32, marched as u32, 1);
        }
        if compose {
            pass.set_pipeline(&kit.compose);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        if recolour {
            pass.set_pipeline(&kit.blur);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        if let Some(span) = span {
            span.end(&mut pass);
        }
    }
    if recolour {
        encoder.copy_buffer_to_texture(
            TexelCopyBufferInfo {
                buffer: &m.texels,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some((m.stride * 4) as u32),
                    rows_per_image: Some(m.height as u32),
                },
            },
            TexelCopyTextureInfo {
                texture: &image.texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            Extent3d {
                width: m.width as u32,
                height: m.height as u32,
                depth_or_array_layers: 1,
            },
        );
        m.texture = Some(image.texture.id());
    }
    queue.submit([encoder.finish()]);
    if let Some(cpu) = &inputs.cpu {
        check(&device, &queue, m, inputs, cpu);
    }
}

/// `BIMS_LIGHTMAP=check`: the GPU's map and texture read back and
/// compared with what the room worked out itself this frame.
fn check(
    device: &RenderDevice,
    queue: &RenderQueue,
    m: &MapState,
    inputs: &LightInputs,
    cpu: &(Vec<u8>, Vec<u8>),
) {
    let (alpha, glow) = cpu;
    let n = m.width * m.height;
    let map = read_back(device, queue, &m.map, n);
    let mut map_diff = 0;
    for i in 0..n.min(alpha.len()) {
        map_diff += (map[i] & 255 != alpha[i] as u32) as u64;
        map_diff += (map[i] >> 8 & 255 != glow[i] as u32) as u64;
    }
    let texels = read_back(device, queue, &m.texels, m.stride * m.height);
    let reference = crate::fogmap::texels(&bims::sight::LightMap {
        width: m.width,
        height: m.height,
        alpha: alpha.clone(),
        glow: glow.clone(),
        ..Default::default()
    });
    let mut texel_diff = 0;
    for y in 0..m.height {
        for x in 0..m.width {
            let gpu = texels[y * m.stride + x].to_le_bytes();
            let want = reference[y * m.width + x];
            texel_diff += gpu.iter().zip(want).filter(|(a, b)| **a != *b).count() as u64;
        }
    }
    let _ = inputs;
    CHECKED.fetch_add(1, Ordering::Relaxed);
    MAP_DIFF.fetch_add(map_diff, Ordering::Relaxed);
    TEXEL_DIFF.fetch_add(texel_diff, Ordering::Relaxed);
}
