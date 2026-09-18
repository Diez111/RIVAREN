//! Quests, objetivos y el hilo narrativo de la civilización perdida.

use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Serialize)]
pub enum Objective {
    Collect { item: u16, count: u32 },
    Kill { mob: &'static str, count: u32 },
    Reach { dim: u8 },
    Talk { npc: &'static str },
    Karma { axis: u8, min: f32 },
    Craft { item: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestState {
    Locked,
    Available,
    Active,
    Done,
}

#[derive(Debug, Clone)]
pub struct Quest {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub objectives: Vec<Objective>,
    pub state: QuestState,
    /// Requiere completar estos ids antes.
    pub requires: Vec<&'static str>,
    /// Fragmento de lore que otorga al completar.
    pub lore: &'static str,
}

#[derive(Debug, Clone, Default)]
pub struct QuestLog {
    pub quests: Vec<Quest>,
    pub completed: Vec<&'static str>,
}

impl QuestLog {
    /// Campaña principal: restaurar el equilibrio entre dimensiones.
    pub fn campaign() -> Self {
        Self {
            quests: vec![
                Quest {
                    id: "primer_aliento",
                    title: "Primer Aliento",
                    description: "Recolecta fibra de bruma para tejer tu primera prenda.",
                    objectives: vec![Objective::Collect { item: 40, count: 3 }],
                    state: QuestState::Available,
                    requires: vec![],
                    lore: "Los Iluminados dejaron hilos de bruma suspendidos entre los árboles de cristal.",
                },
                Quest {
                    id: "voz_del_vergel",
                    title: "La Voz del Vergel",
                    description: "Encuentra y habla con un Iluminado del Bosque de Cristal.",
                    objectives: vec![Objective::Talk { npc: "Iluminado" }],
                    state: QuestState::Locked,
                    requires: vec!["primer_aliento"],
                    lore: "Los sabios no hablan: cantan en frecuencias que solo el corazón entiende.",
                },
                Quest {
                    id: "pulso_perdido",
                    title: "El Pulso Perdido",
                    description: "Construye un Relé de Pulso y una Lámpara para reactivar la red antigua.",
                    objectives: vec![
                        Objective::Craft { item: 71 },
                        Objective::Craft { item: 74 },
                    ],
                    state: QuestState::Locked,
                    requires: vec!["voz_del_vergel"],
                    lore: "La civilización perdida movía montañas con señales de brasa estable.",
                },
                Quest {
                    id: "descenso_voluntario",
                    title: "Descenso Voluntario",
                    description: "Enciende un Portal de Descenso y entra al Infierno.",
                    objectives: vec![Objective::Reach { dim: 1 }],
                    state: QuestState::Locked,
                    requires: vec!["pulso_perdido"],
                    lore: "El Infierno no es un castigo: es un espejo que devuelve lo que siembras.",
                },
                Quest {
                    id: "redencion",
                    title: "Redención",
                    description: "Ayuda a un Penitente y sal del Infierno por el Portal de Ascensión.",
                    objectives: vec![Objective::Talk { npc: "Penitente" }],
                    state: QuestState::Locked,
                    requires: vec!["descenso_voluntario"],
                    lore: "Quien sale del Infierno por voluntad propia conserva su hogar en el alma.",
                },
                Quest {
                    id: "equilibrio",
                    title: "El Equilibrio",
                    description: "Alcanza karma positivo en Compasión y Justicia, y descubre la verdad de la civilización.",
                    objectives: vec![
                        Objective::Karma { axis: 0, min: 40.0 },
                        Objective::Karma { axis: 1, min: 20.0 },
                    ],
                    state: QuestState::Locked,
                    requires: vec!["redencion"],
                    lore: "No hay jefe final. Hay una balanza, y tú eres el fiel.",
                },
            ],
            completed: Vec::new(),
        }
    }

    pub fn unlock_available(&mut self) {
        let done = self.completed.clone();
        for q in self.quests.iter_mut() {
            if q.state == QuestState::Locked && q.requires.iter().all(|r| done.contains(r)) {
                q.state = QuestState::Available;
            }
        }
    }

    pub fn active(&self) -> Option<&Quest> {
        self.quests.iter().find(|q| q.state == QuestState::Active)
    }

    pub fn start(&mut self, id: &str) {
        if let Some(q) = self.quests.iter_mut().find(|q| q.id == id) {
            if q.state == QuestState::Available {
                q.state = QuestState::Active;
            }
        }
    }

    /// Comprueba objetivos; retorna quests recién completadas.
    pub fn evaluate(&mut self, ctx: &ObjectiveCtx) -> Vec<&'static str> {
        let mut done = Vec::new();
        for q in self.quests.iter_mut() {
            if q.state != QuestState::Active {
                continue;
            }
            let all = q.objectives.iter().all(|o| objective_done(o, ctx));
            if all {
                q.state = QuestState::Done;
                self.completed.push(q.id);
                done.push(q.id);
            }
        }
        done
    }
}

/// Estado del juego para evaluar objetivos.
#[derive(Debug, Clone, Default)]
pub struct ObjectiveCtx {
    pub items: Vec<(u16, u32)>,
    pub crafted: Vec<u16>,
    pub talked: Vec<String>,
    pub dimension: u8,
    pub compasion: f32,
    pub justicia: f32,
}

fn objective_done(o: &Objective, ctx: &ObjectiveCtx) -> bool {
    match o {
        Objective::Collect { item, count } => ctx
            .items
            .iter()
            .find(|(i, _)| i == item)
            .map(|(_, c)| c >= count)
            .unwrap_or(false),
        Objective::Kill { .. } => false,
        Objective::Reach { dim } => ctx.dimension == *dim,
        Objective::Talk { npc } => ctx.talked.iter().any(|n| n == npc),
        Objective::Karma { axis, min } => match axis {
            0 => ctx.compasion >= *min,
            1 => ctx.justicia >= *min,
            _ => true,
        },
        Objective::Craft { item } => ctx.crafted.contains(item),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unlock_chain() {
        let mut log = QuestLog::campaign();
        assert_eq!(log.quests[1].state, QuestState::Locked);
        log.completed.push("primer_aliento");
        log.unlock_available();
        assert_eq!(log.quests[1].state, QuestState::Available);
    }
    #[test]
    fn complete_collect() {
        let mut log = QuestLog::campaign();
        log.start("primer_aliento");
        let ctx = ObjectiveCtx {
            items: vec![(40, 5)],
            ..Default::default()
        };
        let done = log.evaluate(&ctx);
        assert_eq!(done, vec!["primer_aliento"]);
    }
}
