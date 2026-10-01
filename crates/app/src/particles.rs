//! The fight's particles, simulated on the GPU.
//!
//! The room says where a spray of particles starts (`bims::fx::Spray`:
//! the sparks off a bolt landing, a grenade's fire and smoke, an EMP's
//! lightning, the classes' auras — `ship::sprays`), and that one record
//! is all the CPU ever does for it. The record goes into a ring of
//! [`SLOTS`] in a storage buffer, stamped with the particle clock, and
//! `particles.wgsl` works out **every particle of every live spray in
//! the vertex stage, every frame**, from the record and the clock alone:
//! where it has flown to, how big it is, how it has cooled and faded —
//! nothing is stepped, nothing is read back, and a spray costs the CPU
//! nothing after the frame it was spawned in.
//!
//! - **The clock** ([`Particles::clock`]) is real seconds that stand still
//!   while the game is paused — the same clock the room ages its passing
//!   lights by (`Session::age_effects`) — so a paused frame is a still
//!   picture of the particles too.
//! - **The mesh** is [`SLOTS`] × [`SPRAY_MOST`] quads, made once, whose
//!   third coordinate is the quad's number: the spray is the number over
//!   [`SPRAY_MOST`], the particle the rest. A slot with no live spray, and
//!   a particle past a spray's count or its life, folds to nothing.
//! - **Units**: a spray arrives in the camera's units (the ship's turn and
//!   the stations' frames are `ship::sprays::take`'s), and the buffer's
//!   header carries the view — the scale and where the camera's origin is
//!   on the window — so the camera moving moves every particle with the
//!   deck without touching a record.
//! - **The layer** goes where the screen paints it (`WorldCanvas::particles`),
//!   over the fog with the shots; a particle is added to the picture
//!   (premultiplied, alpha nought) unless it is smoke, which is laid over
//!   it. A colour past white is past white for the bloom.
//!
//! `BIMS_PARTICLES=0` draws none, for comparing.

use bevy::asset::{RenderAssetUsages, load_internal_asset, uuid_handle};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::MeshVertexBufferLayoutRef;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendState, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::render::storage::ShaderBuffer;
use bevy::shader::{Shader, ShaderRef};
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPlugin};
use bims::fx::{SPRAY_MOST, Spray};

const PARTICLE_SHADER: Handle<Shader> = uuid_handle!("8a3e51c2-7d64-4f09-b1a8-5c2e9d7f4b60");

/// How many sprays can be alive at once: the ring the records go into,
/// the oldest overwritten first. The longest spray lives under three
/// seconds, and a hard fight spawns a few hundred a second.
pub const SLOTS: usize = 2048;

/// Floats a spray record: kind, where, born; where to, reach, life;
/// colour; count, seed, spread, dot.
pub const RECORD: usize = 16;

/// Floats before the records: the clock, the view's scale and origin,
/// then four of nothing.
const HEADER: usize = 8;

/// `BIMS_PARTICLES=0`: no particles drawn. Read once.
pub fn particles_on() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("BIMS_PARTICLES").as_deref() != Ok("0"))
}

pub fn build(app: &mut App) {
    load_internal_asset!(app, PARTICLE_SHADER, "particles.wgsl", Shader::from_wgsl);
    app.add_plugins(Material2dPlugin::<ParticleMaterial>::default())
        .init_resource::<Particles>();
}

/// The particles' layer: its clip and the ring of sprays.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct ParticleMaterial {
    /// The canvas in physical pixels: min x, min y, max x, max y.
    #[uniform(0)]
    clip: Vec4,
    #[storage(1, read_only)]
    sprays: Handle<ShaderBuffer>,
}

impl Material2d for ParticleMaterial {
    fn vertex_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_SHADER)
    }

    fn fragment_shader() -> ShaderRef {
        ShaderRef::Handle(PARTICLE_SHADER)
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    /// Premultiplied, as the canvas blends: a particle with an alpha of
    /// nought is added to the picture, smoke laid over it.
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

/// The layer's entity and what it draws with, made the first frame the
/// layer is painted and let go the first frame it is not.
struct Slot {
    entity: Entity,
    mesh: Handle<Mesh>,
    buffer: Handle<ShaderBuffer>,
    material: Handle<ParticleMaterial>,
    clip: Vec4,
    /// Frames the material is still to be made again for, as a shape
    /// layer's is (`scene::ShapeSlot::rebind`): its bind group may be put
    /// together before the buffer's first upload.
    rebind: u8,
}

/// The particle clock, the ring of sprays and the layer's entity.
#[derive(Resource)]
pub struct Particles {
    /// Real seconds, standing still while the game is paused.
    clock: f32,
    /// The records, [`SLOTS`] of them; a slot never written is nought,
    /// a spray of no particles.
    ring: Vec<[f32; RECORD]>,
    /// The slot the next spray goes into.
    head: usize,
    /// The view this frame: the scale, and the camera's origin on the
    /// window in points.
    view: [f32; 4],
    /// Where the screen painted the layer this frame — its z among the
    /// canvas's layers and its clip — set by `scene::sync`.
    pub(crate) placed: Option<(usize, Vec4)>,
    slot: Option<Slot>,
}

impl Default for Particles {
    fn default() -> Particles {
        Particles {
            clock: 0.0,
            ring: vec![[0.0; RECORD]; SLOTS],
            head: 0,
            view: [1.0, 0.0, 0.0, 0.0],
            placed: None,
            slot: None,
        }
    }
}

impl Particles {
    /// The clock on by `dt` real seconds (nought while paused), and the
    /// sprays the rooms spawned since the last frame put in the ring,
    /// born now.
    pub fn feed(&mut self, dt: f32, sprays: &[Spray]) {
        self.clock += dt.max(0.0);
        for s in sprays {
            self.ring[self.head] = record(s, self.clock);
            self.head = (self.head + 1) % SLOTS;
        }
    }

    /// The view the records are drawn through this frame: points a
    /// camera unit, and where the camera's origin is on the window.
    pub fn set_view(&mut self, scale: f32, origin: Vec2) {
        self.view = [scale, origin.x, origin.y, 0.0];
    }

    /// Every spray forgotten: a new run starts with none.
    fn clear(&mut self) {
        self.ring.fill([0.0; RECORD]);
        self.head = 0;
    }

    fn bytes(&self) -> Vec<u8> {
        let mut floats = Vec::with_capacity(HEADER + SLOTS * RECORD);
        floats.push(self.clock);
        floats.extend_from_slice(&self.view[..3]);
        floats.extend_from_slice(&[0.0; 4]);
        for r in &self.ring {
            floats.extend_from_slice(r);
        }
        floats.iter().flat_map(|f| f.to_le_bytes()).collect()
    }
}

/// One spray as `particles.wgsl` reads it, born at `born` on the clock.
fn record(s: &Spray, born: f32) -> [f32; RECORD] {
    [
        s.kind as u32 as f32,
        s.at.x,
        s.at.y,
        born,
        s.to.x,
        s.to.y,
        s.reach,
        s.life,
        s.colour.r,
        s.colour.g,
        s.colour.b,
        s.colour.a,
        s.count.min(SPRAY_MOST) as f32,
        (s.seed & 0x00ff_ffff) as f32,
        s.spread,
        s.dot,
    ]
}

/// The quads: [`SLOTS`] × [`SPRAY_MOST`], corners at ±1, the third
/// coordinate the quad's number.
fn quads() -> Mesh {
    let n = SLOTS * SPRAY_MOST as usize;
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
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

/// The layer onto its entity, after `scene::sync` has said where it goes:
/// the ring and the clock written into the buffer every frame it is
/// painted, everything let go the first frame it is not.
pub fn sync(
    mut commands: Commands,
    mut particles: ResMut<Particles>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ParticleMaterial>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
    mut placed: Query<(&mut Transform, &mut Visibility, &mut Mesh2d)>,
) {
    let Some((z, clip)) = particles.placed.take().filter(|_| particles_on()) else {
        if let Some(slot) = particles.slot.take() {
            commands.entity(slot.entity).despawn();
            meshes.remove(&slot.mesh);
            materials.remove(&slot.material);
            buffers.remove(&slot.buffer);
            particles.clear();
        }
        return;
    };
    let bytes = particles.bytes();
    let placement = Transform::from_xyz(0.0, 0.0, z as f32).with_scale(Vec3::new(1.0, -1.0, 1.0));
    let Some(slot) = particles.slot.as_mut() else {
        let mesh = meshes.add(quads());
        let buffer = buffers.add(ShaderBuffer::new(&bytes, RenderAssetUsages::RENDER_WORLD));
        let material = materials.add(ParticleMaterial {
            clip,
            sprays: buffer.clone(),
        });
        let entity = commands
            .spawn((
                Mesh2d(mesh.clone()),
                MeshMaterial2d(material.clone()),
                placement,
                Visibility::Visible,
                NoFrustumCulling,
            ))
            .id();
        particles.slot = Some(Slot {
            entity,
            mesh,
            buffer,
            material,
            clip,
            rebind: 2,
        });
        return;
    };
    if let Some(mut buffer) = buffers.get_mut(&slot.buffer) {
        buffer.data = Some(bytes);
    }
    if slot.rebind > 0 || slot.clip != clip {
        if let Some(mut material) = materials.get_mut(&slot.material) {
            material.clip = clip;
        }
        slot.clip = clip;
        slot.rebind = slot.rebind.saturating_sub(1);
    }
    if let Ok((mut transform, mut visibility, mut mesh)) = placed.get_mut(slot.entity) {
        // A move in the draw order has to be told as a change of mesh, or
        // Bevy keeps drawing the layer at its old z (`scene::shape_layer`).
        if transform.set_if_neq(placement) {
            mesh.set_changed();
        }
        visibility.set_if_neq(Visibility::Visible);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bims::fx::SprayKind;
    use bims::math::vec2;

    /// A record is what the shader reads, field for field, and the ring
    /// goes round: the oldest spray is the one written over.
    #[test]
    fn a_spray_is_one_record_and_the_ring_writes_over_the_oldest() {
        let s = Spray::new(SprayKind::Embers, vec2(1.0, 2.0), vec2(3.0, 4.0))
            .reach(20.0)
            .life(0.5)
            .count(40)
            .dot(2.5);
        let r = record(&s, 7.0);
        assert_eq!(r[0], SprayKind::Embers as u32 as f32);
        assert_eq!((r[1], r[2], r[3]), (1.0, 2.0, 7.0));
        assert_eq!((r[4], r[5], r[6], r[7]), (3.0, 4.0, 20.0, 0.5));
        assert_eq!(r[12], SPRAY_MOST as f32, "a record holds sixteen at most");
        assert_eq!(r[15], 2.5);

        let mut p = Particles::default();
        p.feed(0.25, &vec![s; SLOTS + 3]);
        assert_eq!(p.head, 3);
        assert_eq!(p.ring[2][3], 0.25, "born on the clock as fed");
        let bytes = p.bytes();
        assert_eq!(bytes.len(), (HEADER + SLOTS * RECORD) * 4);
        assert_eq!(f32::from_le_bytes(bytes[..4].try_into().unwrap()), 0.25);
        // A paused frame stands the clock still.
        p.feed(0.0, &[]);
        assert_eq!(p.clock, 0.25);
    }
}
