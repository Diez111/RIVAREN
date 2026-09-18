//! Items: definiciones, stacks e inventario.

use serde::{Deserialize, Serialize};

pub type ItemId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    None,
    Hand,
    Pickaxe,
    Axe,
    Shovel,
    Blade,
}

#[derive(Debug, Clone)]
pub struct ItemDef {
    pub id: ItemId,
    pub name: &'static str,
    pub stack_max: u8,
    pub tool: ToolKind,
    pub tool_level: u8,
    pub durability: u16,
    pub color: [f32; 3],
    /// Bloque que coloca (0 = ninguno).
    pub places_block: u16,
    pub food: u8,
    pub description: &'static str,
}

pub fn item_table() -> &'static [ItemDef] {
    use ToolKind::*;
    &[
        ItemDef { id: 0, name: "Aire", stack_max: 0, tool: None, tool_level: 0, durability: 0, color: [0.0; 3], places_block: 0, food: 0, description: "" },
        // Bloques.
        ItemDef { id: 1, name: "Hierba Ferral", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.32, 0.58, 0.26], places_block: 1, food: 0, description: "El tapiz verde de la Tierra de los Vivos." },
        ItemDef { id: 2, name: "Tierra Ferral", stack_max: 99, tool: Shovel, tool_level: 0, durability: 0, color: [0.44, 0.31, 0.20], places_block: 2, food: 0, description: "Rica en minerales de la superficie." },
        ItemDef { id: 4, name: "Piedra Cristal", stack_max: 99, tool: Pickaxe, tool_level: 1, durability: 0, color: [0.54, 0.56, 0.60], places_block: 4, food: 0, description: "Brilla tenue bajo la luz del alba." },
        ItemDef { id: 6, name: "Arena Canto", stack_max: 99, tool: Shovel, tool_level: 0, durability: 0, color: [0.84, 0.74, 0.50], places_block: 6, food: 0, description: "Granos que cantan con el viento." },
        ItemDef { id: 21, name: "Losa de Plaza", stack_max: 99, tool: Pickaxe, tool_level: 1, durability: 0, color: [0.68, 0.66, 0.74], places_block: 21, food: 0, description: "Piedra tallada por los antiguos." },
        ItemDef { id: 30, name: "Marco de Portal", stack_max: 16, tool: Pickaxe, tool_level: 2, durability: 0, color: [0.72, 0.40, 0.92], places_block: 30, food: 0, description: "Vibra con la energía entre dimensiones." },
        // Recursos.
        ItemDef { id: 40, name: "Fibra de Bruma", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.55, 0.72, 0.85], places_block: 0, food: 0, description: "Hilada etérea del Bosque de Cristal." },
        ItemDef { id: 41, name: "Cristal Resonante", stack_max: 99, tool: Pickaxe, tool_level: 2, durability: 0, color: [0.70, 0.45, 0.95], places_block: 0, food: 0, description: "Zumba con una nota que no existe." },
        ItemDef { id: 42, name: "Esencia de Sueño", stack_max: 16, tool: Hand, tool_level: 0, durability: 0, color: [0.85, 0.75, 0.95], places_block: 0, food: 0, description: "Consumida al dormir para fijar tu punto de origen." },
        ItemDef { id: 43, name: "Núcleo de Penitente", stack_max: 16, tool: Hand, tool_level: 0, durability: 0, color: [0.95, 0.35, 0.25], places_block: 0, food: 0, description: "Late con la culpa de una vida pasada." },
        ItemDef { id: 44, name: "Brasa Estable", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [1.0, 0.55, 0.15], places_block: 0, food: 0, description: "Fuego que no consume: base del Pulso." },
        ItemDef { id: 45, name: "Aleación Vetusta", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.75, 0.70, 0.55], places_block: 0, food: 0, description: "Metal de la civilización perdida." },
        // Herramientas.
        ItemDef { id: 50, name: "Pico de Piedra Cristal", stack_max: 1, tool: Pickaxe, tool_level: 2, durability: 250, color: [0.60, 0.62, 0.66], places_block: 0, food: 0, description: "Golpea la piedra como si fuera hielo." },
        ItemDef { id: 51, name: "Hacha de Cristal", stack_max: 1, tool: Axe, tool_level: 2, durability: 250, color: [0.62, 0.50, 0.70], places_block: 0, food: 0, description: "Corta madera de hierro sin astillar." },
        ItemDef { id: 52, name: "Pala de Cristal", stack_max: 1, tool: Shovel, tool_level: 2, durability: 250, color: [0.58, 0.66, 0.64], places_block: 0, food: 0, description: "Mueve la tierra como agua." },
        ItemDef { id: 53, name: "Filo Resonante", stack_max: 1, tool: Blade, tool_level: 3, durability: 400, color: [0.80, 0.60, 1.0], places_block: 0, food: 0, description: "Canta al cortar el aire." },
        // Comida.
        ItemDef { id: 60, name: "Fruto de Vergel", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.90, 0.35, 0.35], places_block: 0, food: 6, description: "Dulce y luminoso." },
        ItemDef { id: 61, name: "Pan de Ferral", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.82, 0.65, 0.35], places_block: 0, food: 9, description: "El sustento de las aldeas." },
        // Módulos de Pulso (Pulso = sistema de señales).
        ItemDef { id: 70, name: "Cable de Pulso", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.85, 0.30, 0.25], places_block: 0, food: 0, description: "Transporta señales de 0 a 15." },
        ItemDef { id: 71, name: "Relé de Pulso", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.70, 0.55, 0.30], places_block: 0, food: 0, description: "Retrasa y amplifica la señal." },
        ItemDef { id: 72, name: "Compuerta Lógica", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.45, 0.70, 0.55], places_block: 0, food: 0, description: "Y / O / NO / XOR configurable." },
        ItemDef { id: 73, name: "Pistón de Vaho", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.60, 0.60, 0.68], places_block: 0, food: 0, description: "Empuja bloques con la señal." },
        ItemDef { id: 74, name: "Lámpara de Pulso", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [1.0, 0.85, 0.45], places_block: 87, food: 0, description: "Se enciende con cualquier señal." },
        // Componentes de Pulso (bloques interactivos).
        ItemDef { id: 80, name: "Cable de Pulso", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.80, 0.25, 0.22], places_block: 80, food: 0, description: "Transporta señales de 0 a 15 con pérdida." },
        ItemDef { id: 81, name: "Antorcha de Pulso", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [1.0, 0.65, 0.20], places_block: 81, food: 0, description: "Invierte la señal: brilla si no recibe energía." },
        ItemDef { id: 82, name: "Palanca", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.70, 0.62, 0.42], places_block: 82, food: 0, description: "Interactúa para encender o apagar." },
        ItemDef { id: 83, name: "Botón", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.55, 0.45, 0.40], places_block: 83, food: 0, description: "Pulso momentáneo de 1 segundo." },
        ItemDef { id: 84, name: "Compuerta Lógica", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.45, 0.70, 0.55], places_block: 84, food: 0, description: "Y / O / NO / XOR según su modo." },
        ItemDef { id: 85, name: "Pistón de Vaho", stack_max: 99, tool: Hand, tool_level: 0, durability: 0, color: [0.60, 0.60, 0.68], places_block: 85, food: 0, description: "Se extiende con la señal (versión simplificada)." },
    ]
}

/// Overlay de items añadidos por mods (id → definición).
/// Los nombres se filtran con `Box::leak` para obtener `&'static str`.
static MODDED: std::sync::OnceLock<Vec<(ItemId, ItemDef)>> = std::sync::OnceLock::new();

pub fn register_modded_items(entries: Vec<(ItemId, String, u16, [f32; 3])>) -> usize {
    let mut defs = Vec::new();
    for (id, name, block, color) in entries {
        let name: &'static str = Box::leak(name.into_boxed_str());
        defs.push((
            id,
            ItemDef {
                id,
                name,
                stack_max: 99,
                tool: ToolKind::Hand,
                tool_level: 0,
                durability: 0,
                color,
                places_block: block,
                food: 0,
                description: "Contenido aportado por un mod.",
            },
        ));
    }
    let n = defs.len();
    let _ = MODDED.set(defs);
    n
}

pub fn modded_items() -> &'static [(ItemId, ItemDef)] {
    MODDED.get().map(|v| v.as_slice()).unwrap_or(&[])
}

pub fn item_def(id: ItemId) -> &'static ItemDef {
    if let Some(found) = modded_items().iter().find(|(i, _)| *i == id) {
        return &found.1;
    }
    let t = item_table();
    t.get(id as usize).unwrap_or(&t[0])
}

/// Mapea bloque colocado a item que lo suelta.
pub fn item_for_block(block: u16) -> ItemId {
    for d in item_table() {
        if d.places_block == block && block != 0 {
            return d.id;
        }
    }
    0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: u8,
    /// Durabilidad restante (solo herramientas).
    pub durability: u16,
}

impl ItemStack {
    pub fn new(id: ItemId) -> Self {
        let def = item_def(id);
        Self {
            id,
            count: 1,
            durability: def.durability,
        }
    }
    pub fn with_count(id: ItemId, count: u8) -> Self {
        let mut s = Self::new(id);
        s.count = count.min(item_def(id).stack_max.max(1));
        s
    }
    pub fn is_tool(&self) -> bool {
        item_def(self.id).tool != ToolKind::None && item_def(self.id).durability > 0
    }
    pub fn durability_ratio(&self) -> f32 {
        let d = item_def(self.id).durability;
        if d == 0 {
            1.0
        } else {
            self.durability as f32 / d as f32
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    /// 0..9 hotbar, 9..36 principal, 36..40 armadura, 40 offhand.
    pub slots: Vec<Option<ItemStack>>,
    pub selected: usize,
    pub craft: Vec<Option<ItemStack>>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: vec![None; 41],
            selected: 0,
            craft: vec![None; 9],
        }
    }
}

impl Inventory {
    pub fn held(&self) -> Option<ItemStack> {
        self.slots.get(self.selected).copied().flatten()
    }
    pub fn held_mut(&mut self) -> Option<&mut ItemStack> {
        self.slots.get_mut(self.selected).and_then(|s| s.as_mut())
    }
    /// Añade un stack; retorna lo que no cupo.
    pub fn add(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        if stack.count == 0 || stack.id == 0 {
            return None;
        }
        let max = item_def(stack.id).stack_max.max(1);
        // Primero completa stacks existentes (hotbar + principal).
        for s in self.slots.iter_mut().take(36) {
            if let Some(existing) = s {
                if existing.id == stack.id && existing.count < max {
                    let space = max - existing.count;
                    let take = space.min(stack.count);
                    existing.count += take;
                    stack.count -= take;
                    if stack.count == 0 {
                        return None;
                    }
                }
            }
        }
        // Luego busca slots vacíos.
        for s in self.slots.iter_mut().take(36) {
            if s.is_none() {
                *s = Some(stack);
                return None;
            }
        }
        Some(stack)
    }
    pub fn count_of(&self, id: ItemId) -> u32 {
        self.slots
            .iter()
            .flatten()
            .filter(|s| s.id == id)
            .map(|s| s.count as u32)
            .sum()
    }
    /// Consume n de un item; retorna true si había suficiente.
    pub fn consume(&mut self, id: ItemId, mut n: u32) -> bool {
        if self.count_of(id) < n {
            return false;
        }
        for s in self.slots.iter_mut().take(36) {
            if let Some(stack) = s {
                if stack.id == id {
                    let take = (stack.count as u32).min(n);
                    stack.count -= take as u8;
                    n -= take;
                    if stack.count == 0 {
                        *s = None;
                    }
                    if n == 0 {
                        break;
                    }
                }
            }
        }
        true
    }
    /// Daña la herramienta en mano; retorna true si se rompió.
    pub fn damage_held(&mut self, amount: u16) -> bool {
        let def = item_def(self.held().map(|s| s.id).unwrap_or(0));
        if def.durability == 0 {
            return false;
        }
        if let Some(s) = self.held_mut() {
            s.durability = s.durability.saturating_sub(amount);
            if s.durability == 0 {
                self.slots[self.selected] = None;
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn add_and_consume() {
        let mut inv = Inventory::default();
        inv.add(ItemStack::with_count(4, 64));
        assert_eq!(inv.count_of(4), 64);
        assert!(inv.consume(4, 10));
        assert_eq!(inv.count_of(4), 54);
        assert!(!inv.consume(4, 1000));
    }
    #[test]
    fn stacking_respects_max() {
        let mut inv = Inventory::default();
        inv.add(ItemStack::with_count(50, 1)); // pico, stack 1
        inv.add(ItemStack::new(50));
        // El segundo pico no se apila: ocupa otro slot.
        assert_eq!(inv.slots.iter().flatten().filter(|s| s.id == 50).count(), 2);
    }
}
