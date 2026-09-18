//! «Pulso»: sistema de señales de la civilización perdida (tipo redstone original).
//! Red de componentes con propagación por ticks programados, sin recursión
//! infinita, con límite de actualizaciones por tick.

use std::collections::{BinaryHeap, HashMap};

pub type Pos = [i32; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulsoKind {
    /// Cable: propaga señal con pérdida de 1.
    Cable,
    /// Antorcha: emite 15 si no recibe señal (inversor).
    Torch,
    /// Palanca: 15 mientras está activa.
    Lever,
    /// Botón: 15 durante 20 ticks.
    Button,
    /// Lámpara: se enciende con cualquier señal.
    Lamp,
    /// Compuerta lógica configurable.
    Gate(GateMode),
    /// Bloque macizo de Pulso (no propaga).
    Block,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateMode {
    And,
    Or,
    Not,
    Xor,
}

#[derive(Debug, Clone, Copy)]
pub struct Node {
    pub kind: PulsoKind,
    pub power: u8,
    pub powered_prev: bool,
    /// Para botones: tick en el que se apaga.
    pub off_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scheduled {
    at: u64,
    pos: Pos,
}
impl PartialOrd for Scheduled {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Scheduled {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.at.cmp(&self.at).then(o.pos.cmp(&self.pos))
    }
}

#[derive(Debug, Default)]
pub struct PulsoWorld {
    pub nodes: HashMap<Pos, Node>,
    queue: BinaryHeap<Scheduled>,
    pub tick: u64,
    /// Cambios de estado aplicados (para el juego: remesh/lamps).
    pub changes: Vec<(Pos, u8)>,
}

/// ¿El componente entrega energía a la red? (lámparas y pistones son cargas).
#[inline(always)]
fn outputs(kind: PulsoKind) -> bool {
    matches!(
        kind,
        PulsoKind::Cable
            | PulsoKind::Torch
            | PulsoKind::Lever
            | PulsoKind::Button
            | PulsoKind::Gate(_)
    )
}

const DIRS: [Pos; 6] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
];

impl PulsoWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn place(&mut self, pos: Pos, kind: PulsoKind) {
        self.nodes.insert(
            pos,
            Node {
                kind,
                // Empieza en 0: el primer tick calcula y propaga el estado real.
                power: 0,
                powered_prev: false,
                off_at: 0,
            },
        );
        self.schedule(pos, self.tick);
        for d in DIRS {
            let n = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
            self.schedule(n, self.tick);
        }
    }

    pub fn remove(&mut self, pos: Pos) {
        if self.nodes.remove(&pos).is_some() {
            for d in DIRS {
                let n = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
                self.schedule(n, self.tick);
            }
        }
    }

    pub fn toggle(&mut self, pos: Pos) {
        if let Some(node) = self.nodes.get_mut(&pos) {
            match node.kind {
                PulsoKind::Lever => {
                    node.power = if node.power > 0 { 0 } else { 15 };
                    self.schedule(pos, self.tick);
                }
                PulsoKind::Button => {
                    node.power = 15;
                    node.off_at = self.tick + 20;
                    self.schedule(pos, self.tick);
                }
                _ => {}
            }
        }
    }

    fn schedule(&mut self, pos: Pos, at: u64) {
        if self.nodes.contains_key(&pos) {
            self.queue.push(Scheduled { at: at + 1, pos });
        }
    }

    fn input_power(&self, pos: Pos, exclude: Pos) -> u8 {
        let mut max = 0u8;
        for d in DIRS {
            let n = [pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]];
            if n == exclude {
                continue;
            }
            if let Some(node) = self.nodes.get(&n) {
                if !outputs(node.kind) {
                    continue; // lámparas/pistones no alimentan la red
                }
                let v = match node.kind {
                    PulsoKind::Cable => node.power.saturating_sub(1),
                    _ => node.power,
                };
                max = max.max(v);
            }
        }
        max
    }

    /// Un tick de simulación. Retorna los cambios de bloque a aplicar.
    pub fn tick_step(&mut self, max_updates: usize) -> Vec<(Pos, u8)> {
        self.tick += 1;
        self.changes.clear();
        let mut processed = 0;
        // Apaga botones vencidos.
        let expired: Vec<Pos> = self
            .nodes
            .iter()
            .filter(|(_, n)| {
                matches!(n.kind, PulsoKind::Button) && n.power > 0 && self.tick >= n.off_at
            })
            .map(|(p, _)| *p)
            .collect();
        for p in expired {
            if let Some(n) = self.nodes.get_mut(&p) {
                n.power = 0;
            }
            self.schedule(p, self.tick);
        }
        // Procesa la cola.
        while processed < max_updates {
            let Some(item) = self.queue.pop() else {
                break;
            };
            if item.at > self.tick {
                self.queue.push(item);
                break;
            }
            let Some(node) = self.nodes.get(&item.pos).copied() else {
                continue;
            };
            let input = self.input_power(item.pos, item.pos);
            let new_power = match node.kind {
                PulsoKind::Torch => {
                    if input > 0 {
                        0
                    } else {
                        15
                    }
                }
                PulsoKind::Lever => node.power,
                PulsoKind::Button => node.power,
                PulsoKind::Block => 0,
                PulsoKind::Cable => input,
                PulsoKind::Lamp => input,
                PulsoKind::Gate(mode) => {
                    let mut inputs: Vec<u8> = Vec::with_capacity(6);
                    for d in DIRS {
                        let n = [item.pos[0] + d[0], item.pos[1] + d[1], item.pos[2] + d[2]];
                        if let Some(other) = self.nodes.get(&n) {
                            if !outputs(other.kind) {
                                continue;
                            }
                            inputs.push(match other.kind {
                                PulsoKind::Cable => other.power.saturating_sub(1),
                                _ => other.power,
                            });
                        }
                    }
                    let a = inputs.first().map(|v| *v > 0).unwrap_or(false);
                    let b = inputs.get(1).map(|v| *v > 0).unwrap_or(false);
                    let on = match mode {
                        GateMode::And => a && b,
                        GateMode::Or => a || b,
                        GateMode::Not => !a,
                        GateMode::Xor => a ^ b,
                    };
                    if on {
                        15
                    } else {
                        0
                    }
                }
            };
            processed += 1;
            if new_power != node.power || (node.power > 0) != node.powered_prev {
                if let Some(n) = self.nodes.get_mut(&item.pos) {
                    n.power = new_power;
                    n.powered_prev = new_power > 0;
                }
                self.changes.push((item.pos, new_power));
                // Propaga a vecinos.
                for d in DIRS {
                    let n = [item.pos[0] + d[0], item.pos[1] + d[1], item.pos[2] + d[2]];
                    self.schedule(n, self.tick);
                }
            }
        }
        self.changes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lever_powers_cable_and_lamp() {
        let mut w = PulsoWorld::new();
        w.place([0, 0, 0], PulsoKind::Lever);
        w.place([1, 0, 0], PulsoKind::Cable);
        w.place([2, 0, 0], PulsoKind::Cable);
        w.place([3, 0, 0], PulsoKind::Lamp);
        w.toggle([0, 0, 0]);
        for _ in 0..10 {
            w.tick_step(64);
        }
        assert!(w.nodes[&[1, 0, 0]].power >= 14);
        assert!(w.nodes[&[3, 0, 0]].power > 0, "la lámpara debe encenderse");
    }

    #[test]
    fn torch_inverter() {
        let mut w = PulsoWorld::new();
        w.place([0, 0, 0], PulsoKind::Lever);
        w.place([1, 0, 0], PulsoKind::Torch);
        w.place([2, 0, 0], PulsoKind::Lamp);
        for _ in 0..8 {
            w.tick_step(64);
        }
        // Sin energía: la antorcha emite 15 y la lámpara está encendida.
        assert_eq!(w.nodes[&[1, 0, 0]].power, 15);
        assert_eq!(w.nodes[&[2, 0, 0]].power, 15);
        // Al activar la palanca, la antorcha se apaga.
        w.toggle([0, 0, 0]);
        for _ in 0..8 {
            w.tick_step(64);
        }
        assert_eq!(w.nodes[&[1, 0, 0]].power, 0);
        assert_eq!(w.nodes[&[2, 0, 0]].power, 0);
    }

    #[test]
    fn gate_and() {
        let mut w = PulsoWorld::new();
        // Gate en [0,0,1]; palancas adyacentes en [0,0,0] (z-) y [0,1,1] (y+).
        w.place([0, 0, 0], PulsoKind::Lever);
        w.place([0, 1, 1], PulsoKind::Lever);
        w.place([0, 0, 1], PulsoKind::Gate(GateMode::And));
        w.place([1, 0, 1], PulsoKind::Lamp);
        w.toggle([0, 0, 0]);
        for _ in 0..6 {
            w.tick_step(64);
        }
        assert_eq!(w.nodes[&[0, 0, 1]].power, 0, "AND con una entrada");
        w.toggle([0, 1, 1]);
        for _ in 0..6 {
            w.tick_step(64);
        }
        assert_eq!(w.nodes[&[0, 0, 1]].power, 15, "AND con dos entradas");
    }
}
