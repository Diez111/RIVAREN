//! Construcción de prompts: persona + estado del mundo + karma + memoria.

use crate::provider::ChatMessage;
use rivaren_gameplay::dialogue::NpcMemory;

/// Contexto del NPC y del jugador para el prompt.
#[derive(Debug, Clone, Default)]
pub struct NpcContext {
    pub npc_name: String,
    pub npc_role: String,
    pub personality: String,
    pub world_lore: String,
    pub compasion: f32,
    pub justicia: f32,
    pub sabiduria: f32,
    pub dimension: &'static str,
    pub time_of_day: &'static str,
    pub weather: &'static str,
    pub nearby_village: Option<String>,
    pub memory: NpcMemory,
}

pub fn build_system_prompt(ctx: &NpcContext) -> String {
    format!(
        "Eres {name}, {role} del mundo de RIVAREN. Personalidad: {personality}. \
Reglas: responde SIEMPRE en español, en 1-3 frases, en personaje, sin romper la inmersión. \
No menciones que eres una IA, ni sistemas reales, ni contenido de otros juegos. \
El mundo tiene tres dimensiones: Tierra de los Vivos, Cielo (sabiduría) e Infierno (redención). \
El Pulso es el sistema de señales de la civilización perdida. \
Lore: {lore} \
El jugador tiene karma compasión {c:.0}/100, justicia {j:.0}/100, sabiduría {s:.0}/100 y está en {dim}. \
Hora: {time}. Clima: {weather}. {village} \
Recuerdos de conversaciones previas: {mem}.",
        name = ctx.npc_name,
        role = ctx.npc_role,
        personality = ctx.personality,
        lore = ctx.world_lore,
        c = ctx.compasion,
        j = ctx.justicia,
        s = ctx.sabiduria,
        dim = ctx.dimension,
        time = ctx.time_of_day,
        weather = ctx.weather,
        village = ctx
            .nearby_village
            .as_ref()
            .map(|v| format!("Estáis en {v}. "))
            .unwrap_or_default(),
        mem = ctx.memory.summary(),
    )
}

pub fn build_messages(ctx: &NpcContext, player_input: &str) -> Vec<ChatMessage> {
    vec![
        ChatMessage::system(build_system_prompt(ctx)),
        ChatMessage::user(player_input),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prompt_contains_state() {
        let ctx = NpcContext {
            npc_name: "Iluminado".into(),
            npc_role: "sabio del Bosque de Cristal".into(),
            personality: "sereno y enigmático".into(),
            world_lore: "Los antiguos construyeron portales-espejo.".into(),
            compasion: 50.0,
            justicia: 20.0,
            sabiduria: 70.0,
            dimension: "Tierra",
            time_of_day: "mediodía",
            weather: "despejado",
            nearby_village: Some("La Aldea del Río Roto".into()),
            memory: NpcMemory::default(),
        };
        let p = build_system_prompt(&ctx);
        assert!(p.contains("RIVAREN"));
        assert!(p.contains("compasión 50"));
        assert!(p.contains("Río Roto"));
    }
}
