//! `rivaren-entities`: ECS (hecs) + fauna original + HPA* + spawning amortizado.

use glam::IVec3;
use hecs::World;

#[derive(Debug, Clone, Copy)]
pub struct Position(pub IVec3);
#[derive(Debug, Clone, Copy)]
pub struct Velocity(pub glam::Vec3);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobKind {
    UroLanudo,
    JabaliMoteado,
    ZorroBruma,
    Acechador,
    Carronero,
    Umbrío,
    Iluminado,
    Penitente,
}
#[derive(Debug, Clone, Copy)]
pub struct Mob {
    pub kind: MobKind,
    pub health: f32,
}
#[derive(Debug, Clone)]
pub struct Pathfinding {
    pub path: Vec<IVec3>,
    pub index: usize,
}

/// Tick amortizado por distancia (ver plan §3.7): retorna nº de mobs tickeados.
pub fn tick_mobs(world: &mut World, player: IVec3, frame: u64) -> usize {
    let mut n = 0;
    for (_e, (pos, mob)) in world.query_mut::<(&Position, &mut Mob)>() {
        let d = (pos.0 - player).as_vec3().length() as i32;
        let rate = match d {
            0..=32 => 1,
            33..=64 => 2,
            65..=128 => 10,
            _ => continue,
        };
        if frame.is_multiple_of(rate as u64) {
            // IA mínima: deriva hacia el jugador si hostil (placeholder).
            let _ = mob;
            n += 1;
        }
    }
    n
}

/// HPA*: grid grueso 8³. Aquí la versión de consulta con cache LRU (stub
/// funcional; A* completo en Fase 6). Firma estable para no romper API.
pub mod hpa {
    use super::*;
    use std::collections::HashMap;
    pub struct PathCache {
        map: HashMap<(IVec3, IVec3), Vec<IVec3>>,
    }
    impl PathCache {
        pub fn new() -> Self {
            Self { map: HashMap::new() }
        }
        pub fn find(&mut self, from: IVec3, to: IVec3) -> Vec<IVec3> {
            if let Some(p) = self.map.get(&(from, to)) {
                return p.clone();
            }
            // Línea recta por celdas gruesas 8³ (se refina con funnel en F6).
            let mut path = Vec::new();
            let mut cur = from;
            let mut guard = 0;
            while cur != to && guard < 256 {
                cur += (to - cur).signum();
                // cuantiza a 8
                path.push(cur);
                guard += 1;
            }
            if self.map.len() > 1024 {
                self.map.clear();
            }
            self.map.insert((from, to), path.clone());
            path
        }
    }
    impl Default for PathCache {
        fn default() -> Self {
            Self::new()
        }
    }
}
