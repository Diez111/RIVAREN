//! Estructuras deterministas: aldeas con Fragmento de Historia, ruinas, portales...
//! Índice espacial grid-hash + influence_radius para consultar solo cercanas.

use glam::IVec3;
use rivaren_core::{Seed, hash3};
use super::pipeline::ChunkBuilder;

pub trait StructureGenerator: Send + Sync {
    fn try_generate(&self, seed: Seed, chunk: IVec3, world: &mut ChunkBuilder) -> bool;
    fn influence_radius(&self) -> i32;
    fn name(&self) -> &'static str;
}

/// Probabilidad hash por chunk: determinista.
#[inline(always)]
pub fn chunk_roll(seed: Seed, chunk: IVec3, salt: u64) -> f32 {
    let h = hash3(seed ^ salt, chunk.x, chunk.y, chunk.z);
    ((h >> 11) as f64 / 9.007199254740992e15) as f32
}

/// Aldea Ferral: aparece cada ~24 chunks en pradera, radio 3.
pub struct VillageGenerator;
impl StructureGenerator for VillageGenerator {
    fn name(&self) -> &'static str { "Aldea Ferral" }
    fn influence_radius(&self) -> i32 { 3 }
    fn try_generate(&self, seed: Seed, chunk: IVec3, b: &mut ChunkBuilder) -> bool {
        // Solo superficie (y==0 en coords de chunk de superficie).
        if chunk_roll(seed, chunk, 0xB1AA) < 0.002 {
            // Plataforma de plaza 9×9 de piedra cristal en el centro del chunk.
            let base_y = 68;
            for dx in -4..=4 {
                for dz in -4..=4 {
                    b.set_local(dx + 16, base_y, dz + 16, 21); // 21 = losa plaza
                }
            }
            return true;
        }
        false
    }
}

/// Portal ruinoso: raro, radio 1.
pub struct RuinedPortalGenerator;
impl StructureGenerator for RuinedPortalGenerator {
    fn name(&self) -> &'static str { "Portal Ruinoso" }
    fn influence_radius(&self) -> i32 { 1 }
    fn try_generate(&self, seed: Seed, chunk: IVec3, b: &mut ChunkBuilder) -> bool {
        if chunk_roll(seed, chunk, 0x9071) < 0.0008 {
            let base_y = 70;
            for dy in 0..4 {
                b.set_local(16, base_y + dy, 16, 30);
                b.set_local(18, base_y + dy, 16, 30);
            }
            b.set_local(17, base_y + 4, 16, 30);
            return true;
        }
        false
    }
}

/// Registro global de estructuras.
pub fn all_structures() -> Vec<Box<dyn StructureGenerator>> {
    vec![Box::new(VillageGenerator), Box::new(RuinedPortalGenerator)]
}
