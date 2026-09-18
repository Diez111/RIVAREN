//! Diálogo: árboles deterministas + perfiles de NPC + memoria persistente.
//! El LLM (crate `rivaren-ai`) se usa opcionalmente para respuestas libres;
//! el árbol siempre funciona como fallback.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueNode {
    pub id: &'static str,
    pub text: &'static str,
    pub options: Vec<DialogueOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueOption {
    pub label: &'static str,
    /// Siguiente nodo; None = cerrar.
    pub next: Option<&'static str>,
    /// Acción de juego opcional.
    pub action: DialogueAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogueAction {
    None,
    CompleteQuest,
    StartQuest(&'static str),
    AcceptBlessing,
    RequestAid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DialogueTree {
    pub npc: &'static str,
    pub root: &'static str,
    pub nodes: Vec<DialogueNode>,
}

impl DialogueTree {
    pub fn node(&self, id: &str) -> Option<&DialogueNode> {
        self.nodes.iter().find(|n| n.id == id)
    }
}

pub fn tree_for(npc: &str) -> DialogueTree {
    match npc {
        "Iluminado" => DialogueTree {
            npc: "Iluminado",
            root: "saludo",
            nodes: vec![
                DialogueNode {
                    id: "saludo",
                    text: "Bienvenido, caminante. Tu alma pesa poco y brilla mucho. ¿Qué buscas?",
                    options: vec![
                        DialogueOption { label: "Busco la verdad de los portales.", next: Some("lore"), action: DialogueAction::None },
                        DialogueOption { label: "Necesito ayuda para mi viaje.", next: Some("ayuda"), action: DialogueAction::RequestAid },
                        DialogueOption { label: "Solo pasaba por aquí.", next: None, action: DialogueAction::None },
                    ],
                },
                DialogueNode {
                    id: "lore",
                    text: "Los antiguos no construyeron puertas: construyeron espejos. Cada portal refleja lo que ya eres. Por eso el Cielo abre solo a los compasivos y el Infierno llama a los que huyen.",
                    options: vec![
                        DialogueOption { label: "¿Y qué queda de ellos?", next: Some("lore2"), action: DialogueAction::None },
                        DialogueOption { label: "Gracias, seguiré mi camino.", next: None, action: DialogueAction::None },
                    ],
                },
                DialogueNode {
                    id: "lore2",
                    text: "Quedan sus señales. Aprende el Pulso y la red antigua te reconocerá como uno de los suyos.",
                    options: vec![
                        DialogueOption { label: "Enseñadme el Pulso.", next: None, action: DialogueAction::StartQuest("pulso_perdido") },
                        DialogueOption { label: "Otro día.", next: None, action: DialogueAction::None },
                    ],
                },
                DialogueNode {
                    id: "ayuda",
                    text: "Toma esta esencia. Fija tu hogar antes de que la noche te encuentre lejos.",
                    options: vec![
                        DialogueOption { label: "Gracias, sabio.", next: None, action: DialogueAction::AcceptBlessing },
                        DialogueOption { label: "Prefiero ganármelo.", next: None, action: DialogueAction::None },
                    ],
                },
            ],
        },
        "Penitente" => DialogueTree {
            npc: "Penitente",
            root: "saludo",
            nodes: vec![
                DialogueNode {
                    id: "saludo",
                    text: "Otro vivo... hueles a sol. Yo también tuve un hogar arriba. ¿Vienes a juzgarme?",
                    options: vec![
                        DialogueOption { label: "Vengo a ayudarte.", next: Some("ayuda"), action: DialogueAction::None },
                        DialogueOption { label: "¿Cómo se sale de aquí?", next: Some("salida"), action: DialogueAction::None },
                        DialogueOption { label: "No eres mi problema.", next: None, action: DialogueAction::None },
                    ],
                },
                DialogueNode {
                    id: "ayuda",
                    text: "Entonces escucha: los Guardianes de la Culpa guardan el núcleo que abre el Portal de Ascensión. Derrótalos y podré descansar. Toma, esto te ayudará.",
                    options: vec![
                        DialogueOption { label: "Lo haré.", next: None, action: DialogueAction::CompleteQuest },
                    ],
                },
                DialogueNode {
                    id: "salida",
                    text: "Arriba, en las grietas selladas, hay un marco que aún recuerda la luz. Llévale una brasa estable y te devolverá tu hogar.",
                    options: vec![
                        DialogueOption { label: "Gracias, alma errante.", next: None, action: DialogueAction::None },
                    ],
                },
            ],
        },
        _ => DialogueTree {
            npc: "Aldeano",
            root: "saludo",
            nodes: vec![DialogueNode {
                id: "saludo",
                text: "Buenos días. Si buscas trabajo, habla con nuestra alcaldesa.",
                options: vec![DialogueOption { label: "Adiós.", next: None, action: DialogueAction::None }],
            }],
        },
    }
}

/// Memoria persistente por NPC (para el LLM y para saludos contextuales).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NpcMemory {
    pub npc: String,
    pub facts: Vec<String>,
    pub last_topic: String,
    pub times_talked: u32,
    pub affinity: f32,
}

impl NpcMemory {
    pub fn remember(&mut self, fact: &str) {
        if self.facts.len() >= 32 {
            self.facts.remove(0);
        }
        if !self.facts.iter().any(|f| f == fact) {
            self.facts.push(fact.to_string());
        }
    }
    pub fn summary(&self) -> String {
        if self.facts.is_empty() {
            "sin recuerdos".to_string()
        } else {
            self.facts.join("; ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tree_has_root() {
        let t = tree_for("Iluminado");
        assert!(t.node(t.root).is_some());
    }
    #[test]
    fn memory_caps() {
        let mut m = NpcMemory::default();
        for i in 0..40 {
            m.remember(&format!("hecho {i}"));
        }
        assert!(m.facts.len() <= 32);
    }
}
