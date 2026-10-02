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
//! **Softening** (task 140, `lightsoften.wgsl`): the lamps' shadows and
//! the edge of what is seen given a penumbra that widens away from what
//! casts it, and the deck's corners shaded — passes of their own over the
//! same map, in a module and a bind group of their own so the three above
//! are what they were. `BIMS_SHADOWS=0` and `BIMS_AO=0` turn each off, and
//! `BIMS_SOFTEN_CHECK=1` counts the pixels the softening moved across the
//! room's line between lit and dark, or seen and not. A map the CPU worked
//! out (`RawJob`: a planet's plain, the deck under `BIMS_LIGHTMAP=cpu`)
//! comes up as its two raw channels and goes through the same `blur`.
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

/// `BIMS_SHADOWS=0`: the lamps' shadows as the room marched them, their
/// edges no softer than the fog's blur makes every edge. Anything else, or
/// nothing, softens them (task 140). Off under `BIMS_LIGHTMAP=check`,
/// which compares the picture with the CPU's byte for byte.
pub fn shadows_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_SHADOWS").as_deref() != Ok("0")) && mode() != Mode::Check
}

/// `BIMS_AO=0`: no shade in the deck's corners and along its walls.
/// Anything else, or nothing, shades them (task 140). Off under
/// `BIMS_LIGHTMAP=check`, as the shadows are.
pub fn ao_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_AO").as_deref() != Ok("0")) && mode() != Mode::Check
}

/// The two switches as the shader reads them (`Params::flags`).
fn flags() -> u32 {
    shadows_on() as u32 | (ao_on() as u32) << 1
}

/// The job this frame: what the room handed over, and the fog's picture
/// to draw it into. Emptied at the start of every frame, so a frame whose
/// screen draws no fog draws nothing here.
#[derive(Resource, Clone, Default, ExtractResource)]
pub struct LightJob {
    pub inputs: Option<Arc<LightInputs>>,
    pub picture: Option<AssetId<Image>>,
    /// The pictures whose two channels the CPU worked out (task 140): a
    /// planet's plain a chunk at a time, and the deck's own under
    /// `BIMS_LIGHTMAP=cpu`, blurred and coloured here as the deck's is.
    pub raw: Vec<RawJob>,
}

/// One picture's two channels as the CPU worked them out, a word a pixel
/// — the darkness in the low byte, the lamplight above, as the `map`
/// buffer holds them — for the GPU to blur and colour into `picture`.
/// `pixels` is the whole of it, shared, and made again only where it
/// changed; `changed` is the box this version differs from the one before
/// in, or `None` for all of it.
#[derive(Clone, Debug)]
pub struct RawJob {
    pub picture: AssetId<Image>,
    pub width: usize,
    pub height: usize,
    pub version: u64,
    pub changed: Option<(usize, usize, usize, usize)>,
    pub pixels: Arc<Vec<u32>>,
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
static RAW_CHECKED: AtomicU64 = AtomicU64::new(0);
static RAW_DIFF: AtomicU64 = AtomicU64::new(0);

/// What `BIMS_LIGHTMAP=check` found, as the line a smoke run prints: how
/// many frames were compared, and how many bytes of the map and of the
/// texture differed in all.
pub fn report() -> Vec<String> {
    let mut lines = Vec::new();
    if soften_check() {
        lines.push(format!(
            "soften check: {} frames, {} pixels lit or dark the other way, \
             {} seen or unseen the other way",
            SOFT_CHECKED.load(Ordering::Relaxed),
            SOFT_LIGHT_FLIPS.load(Ordering::Relaxed),
            SOFT_SIGHT_FLIPS.load(Ordering::Relaxed),
        ));
    }
    if mode() != Mode::Check {
        return lines;
    }
    lines.push(format!(
        "lightmap check: {} frames, {} map bytes, {} texel bytes differing; \
         {} pictures from the CPU's channels, {} texel bytes differing",
        CHECKED.load(Ordering::Relaxed),
        MAP_DIFF.load(Ordering::Relaxed),
        TEXEL_DIFF.load(Ordering::Relaxed),
        RAW_CHECKED.load(Ordering::Relaxed),
        RAW_DIFF.load(Ordering::Relaxed),
    ));
    lines
}

/// `BIMS_SOFTEN_CHECK=1` (task 140): every frame the softened picture is
/// composed, it is read back and held to the promise the softening makes —
/// that no pixel the room's light map calls lit is softened dark or the
/// other way round, and no pixel it calls seen is drawn under the fog's
/// half or the other way round — and the pixels that break it counted,
/// printed when a smoke run exits.
pub fn soften_check() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_SOFTEN_CHECK").as_deref() == Ok("1"))
}

static SOFT_CHECKED: AtomicU64 = AtomicU64::new(0);
static SOFT_LIGHT_FLIPS: AtomicU64 = AtomicU64::new(0);
static SOFT_SIGHT_FLIPS: AtomicU64 = AtomicU64::new(0);

/// The shown light at which a pixel counts as lit (`LIT` in
/// `lightsoften.wgsl`).
const LIT: usize = 32;
/// The most of its neighbourhood a pixel nobody sees is drawn as seen
/// (`SIGHT_BELOW_HALF`).
const SIGHT_BELOW_HALF: f32 = 0.49;

/// The check itself: the room's shown light against the softened, a pixel
/// at a time; and each fogged pixel's darkness against the bound its side
/// of the sight's edge puts on it — a seen one no darker than halfway from
/// the darkest it could be drawn seen (its corner's shade at the most) to
/// the fog, an unseen one no lighter than `SIGHT_BELOW_HALF` of the way
/// from the fog to the lightest (no shade at all), a level's rounding
/// allowed either way.
fn check_soft(device: &RenderDevice, queue: &RenderQueue, m: &MapState, inputs: &LightInputs) {
    let n = m.width * m.height;
    let fields = read_back(device, queue, &m.fields, n);
    let soft = read_back(device, queue, &m.soft, n);
    let vis = read_back(device, queue, &m.vis, n);
    let map = read_back(device, queue, &m.map, n);
    let fog = inputs.fog as f32;
    let (mut light_flips, mut sight_flips) = (0, 0);
    for i in 0..n {
        let shown = (fields[i] >> 8 & 255) as usize;
        let softened = soft[i] as usize;
        if (shown >= LIT) != (softened >= LIT) {
            light_flips += 1;
        }
        let (x, y) = (i % m.width, i / m.width);
        let cell = inputs.cells[(y / 8) * inputs.columns + x / 8];
        if cell & bims::sight::CELL_FOGGED == 0 {
            continue;
        }
        let alpha = (map[i] & 255) as f32;
        let lightest = inputs.dark[softened] as f32;
        let darkest = if softened >= LIT {
            lightest.max(inputs.dark[LIT] as f32)
        } else {
            fog - 1.0
        };
        let seen = f32::from_bits(vis[i]) > 0.5;
        let wrong = if seen {
            alpha > (fog + darkest) / 2.0 + 1.0
        } else {
            alpha < fog - SIGHT_BELOW_HALF * (fog - lightest) - 1.0
        };
        sight_flips += wrong as u64;
    }
    SOFT_CHECKED.fetch_add(1, Ordering::Relaxed);
    SOFT_LIGHT_FLIPS.fetch_add(light_flips, Ordering::Relaxed);
    SOFT_SIGHT_FLIPS.fetch_add(sight_flips, Ordering::Relaxed);
}

// --- the render world ---------------------------------------------------------

/// The pipelines and the two tables that never change, made the first
/// time a map is drawn; the one map being drawn; and the pictures blurred
/// from the CPU's channels, by the picture they go into.
#[derive(Resource, Default)]
struct GpuLight {
    kit: Option<Kit>,
    map: Option<MapState>,
    raw: std::collections::HashMap<AssetId<Image>, RawState>,
}

struct Kit {
    layout: BindGroupLayout,
    soft_layout: BindGroupLayout,
    march: ComputePipeline,
    compose: ComputePipeline,
    compose_soft: ComputePipeline,
    blur: ComputePipeline,
    distance: ComputePipeline,
    soften_across: ComputePipeline,
    soften_down: ComputePipeline,
    sight_across: ComputePipeline,
    sight_down: ComputePipeline,
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
        // The softening (task 140) is `lightmap.wgsl` with `lightsoften.wgsl`
        // after it, a module of its own: the passes of task 121 are made
        // from their own shader as it was, and cost what they cost.
        let soft_module = device.create_and_validate_shader_module(ShaderModuleDescriptor {
            label: Some("light soften"),
            source: ShaderSource::Wgsl(
                concat!(
                    include_str!("lightmap.wgsl"),
                    include_str!("lightsoften.wgsl")
                )
                .into(),
            ),
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
        // The softening's buffers are a group of their own, which only its
        // passes are made with: the passes of task 121 bind what they bound.
        let soft_entries = [
            entry(0, write),
            entry(1, write),
            entry(2, write),
            entry(3, write),
        ];
        let soft_layout = device.create_bind_group_layout("light soften", &soft_entries);
        let plain = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("light map"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let softened = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("light soften"),
            bind_group_layouts: &[Some(&layout), Some(&soft_layout)],
            immediate_size: 0,
        });
        let with = |entry: &str, pipeline_layout, module| {
            device.create_compute_pipeline(&RawComputePipelineDescriptor {
                label: Some("light map"),
                layout: Some(pipeline_layout),
                module,
                entry_point: Some(entry),
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let pipeline = |entry: &str| with(entry, &plain, &module);
        let soft = |entry: &str| with(entry, &softened, &soft_module);
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
            compose_soft: soft("compose_soft"),
            blur: pipeline("blur"),
            distance: soft("distance"),
            soften_across: soft("soften_across"),
            soften_down: soft("soften_down"),
            sight_across: soft("sight_across"),
            sight_down: soft("sight_down"),
            soft_layout,
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
    tables: Buffer,
    /// The softening's (task 140): the distance field, the softened light
    /// and the pass between its two halves.
    dist: Buffer,
    soft: Buffer,
    across: Buffer,
    vis: Buffer,
    /// How many bodies' views the seen buffer holds, and how
    /// many eyes the eyes and starts buffers.
    slots: usize,
    eye_room: usize,
    bind: Option<BindGroup>,
    soft_bind: Option<BindGroup>,
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
        // The softening's buffers, a word a pixel — nothing with it off.
        let softening = if flags() != 0 { w * h * 4 } else { 0 };
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
            tables: storage(device, "light map: tables", 1024 * 4),
            dist: storage(device, "light map: distance", softening),
            soft: storage(device, "light map: soft", softening),
            across: storage(device, "light map: across", softening),
            vis: storage(device, "light map: seen or not", softening),
            slots,
            eye_room: 8,
            bind: None,
            soft_bind: None,
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
        self.seen = seen;
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

    /// The softening's group: its four buffers, made once a map.
    fn soft_bind_group(&mut self, device: &RenderDevice, kit: &Kit) -> &BindGroup {
        self.soft_bind.get_or_insert_with(|| {
            let buffers = [&self.dist, &self.soft, &self.across, &self.vis];
            let entries: Vec<BindGroupEntry> = buffers
                .iter()
                .enumerate()
                .map(|(binding, buffer)| BindGroupEntry {
                    binding: binding as u32,
                    resource: buffer.as_entire_binding(),
                })
                .collect();
            device.create_bind_group("light soften", &kit.soft_layout, &entries)
        })
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
                // Unread since task 152 put the march's dark rule in the
                // tables (`sight::dark_reach`) for the near tiles it was;
                // the layout keeps its place.
                &self.tables,
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
    let GpuLight { kit, map, raw } = &mut *gpu;
    // A picture of the CPU's not asked for this frame is let go of: a
    // chunk the plain gave up, a deck left.
    raw.retain(|id, _| job.raw.iter().any(|r| r.picture == *id));
    if job.inputs.is_none() && job.raw.is_empty() {
        return;
    }
    let kit = kit.get_or_insert_with(|| Kit::new(&device, &queue));
    let recorder = recorder.as_deref();
    if let (Some(inputs), Some(picture)) = (&job.inputs, job.picture) {
        draw_deck(
            inputs,
            picture,
            kit,
            map,
            (&*device, &*queue),
            &images,
            recorder,
        );
    }
    for r in &job.raw {
        draw_raw(r, kit, raw, (&*device, &*queue), &images, recorder);
    }
}

/// The room's inputs marched, composed, softened, blurred and coloured
/// into the deck's picture.
fn draw_deck(
    inputs: &LightInputs,
    picture: AssetId<Image>,
    kit: &Kit,
    map: &mut Option<MapState>,
    (device, queue): (&RenderDevice, &RenderQueue),
    images: &RenderAssets<GpuImage>,
    recorder: Option<&DiagnosticsRecorder>,
) {
    let Some(image) = images.get(picture) else {
        return;
    };
    if image.texture_descriptor.size.width as usize != inputs.width
        || image.texture_descriptor.size.height as usize != inputs.height
    {
        return;
    }
    let fresh = !map.as_ref().is_some_and(|m| {
        m.sight == inputs.sight && m.width == inputs.width && m.height == inputs.height
    });
    if fresh {
        *map = Some(MapState::new(device, inputs));
    }
    let Some(m) = map.as_mut() else {
        return;
    };
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("light map"),
    });
    m.room_for_views(device, &mut encoder, inputs.views.len());
    let mut compose = fresh || m.views != inputs.views.len();
    m.views = inputs.views.len();
    // What the softening is worked out again for (task 140): the distance
    // field when the cells change, the softened light when that or the
    // lamps' light does.
    let flags = flags();
    let cells_changed = m.cells_rev != Some(inputs.cells_rev);
    let fields_changed = m.fields_rev != Some(inputs.fields_rev);
    if cells_changed {
        let cells: Vec<u32> = inputs.cells.iter().map(|&c| c as u32).collect();
        queue.write_buffer(&m.cells, 0, &words_of(&cells));
        m.cells_rev = Some(inputs.cells_rev);
        compose = true;
    }
    if fields_changed {
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
            // The body's pixel, two signed sixteen-bit halves.
            let body = (view.body.0 as u16 as u32) | (view.body.1 as u16 as u32) << 16;
            eyes.extend_from_slice(&body.to_le_bytes());
            starts.extend(floats(
                eye.t0.iter().flat_map(|t| [finite(t[0]), finite(t[1])]),
            ));
        }
        compose = true;
    }
    let marched = eyes.len() / EYE_BYTES;
    m.room_for_eyes(device, marched);
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
        flags,
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
        .chain(inputs.reach)
        .collect();
    queue.write_buffer(&m.tables, 0, &words_of(&tables));
    let recolour = compose || m.texture != Some(image.texture.id());
    let checking = inputs.cpu.is_some();
    if !recolour && !checking {
        return;
    }
    let (gx, gy) = (m.width.div_ceil(8) as u32, m.height.div_ceil(8) as u32);
    let bind = m.bind_group(device, kit).clone();
    let soft_bind = m.soft_bind_group(device, kit).clone();
    let distance = cells_changed && flags != 0;
    let soften = (cells_changed || fields_changed) && flags & 1 != 0;
    if distance || soften {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("light soften"),
            timestamp_writes: None,
        });
        let span = recorder.map(|r| r.pass_span(&mut pass, "light soften"));
        pass.set_bind_group(0, &bind, &[]);
        pass.set_bind_group(1, &soft_bind, &[]);
        if distance {
            pass.set_pipeline(&kit.distance);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        if soften {
            pass.set_pipeline(&kit.soften_across);
            pass.dispatch_workgroups(gx, gy, 1);
            pass.set_pipeline(&kit.soften_down);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        if let Some(span) = span {
            span.end(&mut pass);
        }
    }
    {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("light map"),
            timestamp_writes: None,
        });
        // Timed on the GPU under `BIMS_PERF`, as Bevy times its own passes.
        let span = recorder.map(|r| r.pass_span(&mut pass, "light map"));
        pass.set_bind_group(0, &bind, &[]);
        if flags != 0 {
            pass.set_bind_group(1, &soft_bind, &[]);
        }
        if marched > 0 {
            pass.set_pipeline(&kit.march);
            pass.dispatch_workgroups((RAYS / 64) as u32, marched as u32, 1);
        }
        if compose {
            // Composed as the CPU composes, with the softening off.
            pass.set_pipeline(if flags == 0 {
                &kit.compose
            } else {
                &kit.compose_soft
            });
            pass.dispatch_workgroups(gx, gy, 1);
            // The edge of what is seen softened over what was composed.
            if flags & 1 != 0 {
                pass.set_pipeline(&kit.sight_across);
                pass.dispatch_workgroups(gx, gy, 1);
                pass.set_pipeline(&kit.sight_down);
                pass.dispatch_workgroups(gx, gy, 1);
            }
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
    if compose && flags & 1 != 0 && soften_check() {
        check_soft(device, queue, m, inputs);
    }
    if let Some(cpu) = &inputs.cpu {
        check(device, queue, m, inputs, cpu);
    }
}

// --- a picture from the CPU's channels (task 140) ----------------------------

/// A picture blurred from the CPU's two channels: the channels as last
/// uploaded and which version they are, the texels the blur writes, the
/// bind group over them, and the texture they were last copied into. The
/// bindings the blur never reads are a little buffer each.
struct RawState {
    width: usize,
    height: usize,
    stride: usize,
    version: u64,
    map: Buffer,
    texels: Buffer,
    bind: BindGroup,
    texture: Option<TextureId>,
    _held: Vec<Buffer>,
}

impl RawState {
    fn new(
        (device, queue): (&RenderDevice, &RenderQueue),
        kit: &Kit,
        (width, height): (usize, usize),
    ) -> RawState {
        let stride = width.next_multiple_of(64);
        let params = device.create_buffer(&BufferDescriptor {
            label: Some("light map: channels' params"),
            size: 48,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut words = [0u32; 12];
        words[0] = width as u32;
        words[1] = height as u32;
        words[7] = stride as u32;
        queue.write_buffer(&params, 0, &words_of(&words));
        let map = storage(device, "light map: channels", width * height * 4);
        let texels = storage(device, "light map: channels' texels", stride * height * 4);
        let spare: Vec<Buffer> = (0..12)
            .map(|_| storage(device, "light map: unread", 64))
            .collect();
        let entries: Vec<BindGroupEntry> = (0..12)
            .map(|binding| BindGroupEntry {
                binding: binding as u32,
                resource: match binding {
                    0 => &params,
                    4 => &map,
                    5 => &texels,
                    10 => &kit.colours,
                    _ => &spare[binding],
                }
                .as_entire_binding(),
            })
            .collect();
        let bind = device.create_bind_group("light map: channels", &kit.layout, &entries);
        let mut held = spare;
        held.push(params);
        RawState {
            width,
            height,
            stride,
            version: u64::MAX,
            map,
            texels,
            bind,
            texture: None,
            _held: held,
        }
    }
}

/// One picture of the CPU's channels: the rows that changed uploaded —
/// the box's when this holds the version before, else the lot — then
/// blurred and coloured by the deck's own pass into its texture.
fn draw_raw(
    job: &RawJob,
    kit: &Kit,
    states: &mut std::collections::HashMap<AssetId<Image>, RawState>,
    (device, queue): (&RenderDevice, &RenderQueue),
    images: &RenderAssets<GpuImage>,
    recorder: Option<&DiagnosticsRecorder>,
) {
    let (w, h) = (job.width, job.height);
    let Some(image) = images.get(job.picture) else {
        return;
    };
    if image.texture_descriptor.size.width as usize != w
        || image.texture_descriptor.size.height as usize != h
        || job.pixels.len() != w * h
    {
        return;
    }
    let fresh = !states
        .get(&job.picture)
        .is_some_and(|s| (s.width, s.height) == (w, h));
    if fresh {
        states.insert(job.picture, RawState::new((device, queue), kit, (w, h)));
    }
    let Some(s) = states.get_mut(&job.picture) else {
        return;
    };
    let upload = s.version != job.version;
    if upload {
        let follows = s.version.wrapping_add(1) == job.version;
        let (y0, rows) = match (follows, job.changed) {
            (true, Some((_, y, _, rows))) => (y, rows),
            _ => (0, h),
        };
        let (from, to) = (y0 * w, ((y0 + rows) * w).min(w * h));
        queue.write_buffer(&s.map, (from * 4) as u64, &words_of(&job.pixels[from..to]));
        s.version = job.version;
    }
    if !upload && s.texture == Some(image.texture.id()) {
        return;
    }
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("fog blur"),
    });
    if upload {
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("fog blur"),
            timestamp_writes: None,
        });
        let span = recorder.map(|r| r.pass_span(&mut pass, "fog blur"));
        pass.set_bind_group(0, &s.bind, &[]);
        pass.set_pipeline(&kit.blur);
        pass.dispatch_workgroups(w.div_ceil(8) as u32, h.div_ceil(8) as u32, 1);
        if let Some(span) = span {
            span.end(&mut pass);
        }
    }
    encoder.copy_buffer_to_texture(
        TexelCopyBufferInfo {
            buffer: &s.texels,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((s.stride * 4) as u32),
                rows_per_image: Some(h as u32),
            },
        },
        TexelCopyTextureInfo {
            texture: &image.texture,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        Extent3d {
            width: w as u32,
            height: h as u32,
            depth_or_array_layers: 1,
        },
    );
    s.texture = Some(image.texture.id());
    queue.submit([encoder.finish()]);
    if upload && mode() == Mode::Check {
        check_raw(device, queue, s, job);
    }
}

/// `BIMS_LIGHTMAP=check`, for a picture of the CPU's channels: the GPU's
/// texels read back and compared with `fogmap::texels`, which is what the
/// CPU drew the same channels as before task 140.
fn check_raw(device: &RenderDevice, queue: &RenderQueue, s: &RawState, job: &RawJob) {
    let texels = read_back(device, queue, &s.texels, s.stride * s.height);
    let reference = crate::fogmap::texels(&bims::sight::LightMap {
        width: s.width,
        height: s.height,
        alpha: job.pixels.iter().map(|&p| p as u8).collect(),
        glow: job.pixels.iter().map(|&p| (p >> 8) as u8).collect(),
        ..Default::default()
    });
    let mut diff = 0;
    for y in 0..s.height {
        for x in 0..s.width {
            let gpu = texels[y * s.stride + x].to_le_bytes();
            let want = reference[y * s.width + x];
            diff += gpu.iter().zip(want).filter(|(a, b)| **a != *b).count() as u64;
        }
    }
    RAW_CHECKED.fetch_add(1, Ordering::Relaxed);
    RAW_DIFF.fetch_add(diff, Ordering::Relaxed);
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
