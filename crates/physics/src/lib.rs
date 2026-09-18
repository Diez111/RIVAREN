//! `rivaren-physics`: AABB vs SVDAG + tick queue + fluidos + circuito Pulso + fuego.

use glam::Vec3;
use rivaren_core::Aabb;
use std::collections::BinaryHeap;

/// Tick programado con prioridad temporal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScheduledTick {
    pub time: u64,
    pub pos: [i32; 3],
    pub kind: u8,
}
impl Eq for ScheduledTick {}
impl PartialOrd for ScheduledTick {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for ScheduledTick {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.time.cmp(&self.time)
    }
}

pub struct TickQueue {
    queue: BinaryHeap<ScheduledTick>,
    pub max_per_frame: usize,
}

impl TickQueue {
    pub fn new(max_per_frame: usize) -> Self {
        Self { queue: BinaryHeap::new(), max_per_frame }
    }
    pub fn schedule(&mut self, t: ScheduledTick) {
        self.queue.push(t);
    }
    pub fn tick(&mut self, now: u64, mut f: impl FnMut(ScheduledTick)) -> usize {
        let mut n = 0;
        while n < self.max_per_frame {
            match self.queue.peek() {
                Some(t) if t.time <= now => {
                    let t = self.queue.pop().unwrap();
                    f(t);
                    n += 1;
                }
                _ => break,
            }
        }
        n
    }
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Colisión AABB vs mundo SDF (Fase 1: SDF analítico; F6: SVDAG raymarch).
pub fn collide_aabb(
    aabb: &Aabb,
    vel: Vec3,
    seed: u64,
    sdf: impl Fn(u64, f32, f32, f32) -> f32,
) -> (Vec3, bool) {
    let mut pos = (aabb.min + aabb.max) * 0.5;
    let half = (aabb.max - aabb.min) * 0.5;
    let mut hit = false;
    for axis in 0..3 {
        let mut trial = pos;
        trial[axis] += vel[axis] * rivaren_core::FIXED_DT;
        let mut penetrating = false;
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    let p = trial + half * Vec3::new(sx, sy, sz);
                    if sdf(seed, p.x, p.y, p.z) < 0.0 {
                        penetrating = true;
                    }
                }
            }
        }
        if penetrating {
            hit = true;
        } else {
            pos = trial;
        }
    }
    (pos, hit)
}

/// Circuito «Pulso» (redstone original RIVAREN): grafo de componentes con
/// orden topológico + batch por tipo, máx 1000 updates/frame, resto difiere.
/// Niveles 0..15 como la luz, pero con retardo programable vía TickQueue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulsoKind {
    Hilo,
    Antorcha,
    Bloque,
    Repetidor(u8), // retardo 1..4 ticks
    Comparador,
    Piston,
    Lampara,
}

#[derive(Debug, Clone, Copy)]
pub struct PulsoNode {
    pub pos: [i32; 3],
    pub kind: PulsoKind,
    pub power: u8,
}

/// Un paso de propagación Pulso sobre el vecindario de 6.
/// Retorna la potencia de salida (0..15). Puro y testeable.
pub fn pulso_step(node: PulsoNode, neighbour_power: [u8; 6]) -> u8 {
    let max_n = *neighbour_power.iter().max().unwrap_or(&0);
    match node.kind {
        PulsoKind::Antorcha => {
            if max_n > 0 {
                0
            } else {
                15
            }
        }
        PulsoKind::Hilo => max_n.saturating_sub(1),
        PulsoKind::Bloque => max_n,
        PulsoKind::Repetidor(_) => {
            if max_n > 0 {
                15
            } else {
                0
            }
        }
        PulsoKind::Comparador => max_n, // modo resta en Fase 7
        PulsoKind::Piston => {
            if max_n > 0 {
                15
            } else {
                0
            }
        }
        PulsoKind::Lampara => max_n,
    }
}

/// Fluido «agua viva»: autómata celular (nivel 0..7 + bit cayendo).
/// Reglas: cae si aire debajo; si no, se expande a 4 vecinos con nivel-1.
/// Solo chunks activos (con fluido en movimiento) — ver fluid.comp.wgsl.
pub fn fluid_step(level_here: u8, below_solid: bool, neighbour_levels: [u8; 4]) -> [u8; 5] {
    // Retorna [nuevo_aquí, norte, sur, este, oeste] (0 = sin cambio).
    if level_here == 0 {
        return [0; 5];
    }
    if !below_solid {
        return [level_here, 0, 0, 0, 0]; // cae (el llamador mueve el bloque)
    }
    let spread = level_here.saturating_sub(1);
    if spread == 0 {
        return [level_here, 0, 0, 0, 0];
    }
    let mut out = [level_here, 0, 0, 0, 0];
    for (i, nl) in neighbour_levels.iter().enumerate() {
        if *nl < spread.saturating_sub(1) {
            out[i + 1] = spread;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pulso_torch_inverts() {
        let t = PulsoNode { pos: [0, 0, 0], kind: PulsoKind::Antorcha, power: 0 };
        assert_eq!(pulso_step(t, [0; 6]), 15);
        assert_eq!(pulso_step(t, [15, 0, 0, 0, 0, 0]), 0);
    }
    #[test]
    fn fluid_falls() {
        assert_eq!(fluid_step(7, false, [0; 4])[0], 7);
        assert_eq!(fluid_step(1, true, [0; 4]), [1, 0, 0, 0, 0]);
    }
}
