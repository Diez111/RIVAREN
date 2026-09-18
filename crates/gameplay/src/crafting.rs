//! Crafteo: recetas con forma y sin forma, fundición, y mesa 3×3.

use serde::{Deserialize, Serialize};
use crate::items::{ItemId, ItemStack};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecipeKind {
    /// Patrón 3×3 (con huecos). `None` = vacío.
    Shaped { pattern: Vec<Vec<Option<ItemId>>> },
    /// Ingredientes en cualquier orden.
    Shapeless { items: Vec<ItemId> },
    /// Fundición: 1 entrada → 1 salida con tiempo.
    Smelting { input: ItemId, time_ms: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub kind: RecipeKind,
    pub output: ItemStack,
    /// Solo mesa 3×3 (false = también 2×2 de inventario).
    pub needs_table: bool,
}

pub fn recipe_book() -> Vec<Recipe> {
    vec![
        Recipe {
            kind: RecipeKind::Shapeless { items: vec![40, 40, 40] },
            output: ItemStack::with_count(70, 3),
            needs_table: false,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![Some(4), Some(4), Some(4)],
                    vec![Some(4), Some(70), Some(4)],
                    vec![Some(4), Some(4), Some(4)],
                ],
            },
            output: ItemStack::with_count(71, 1),
            needs_table: true,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![Some(45), Some(45)],
                    vec![Some(45), Some(45)],
                ],
            },
            output: ItemStack::with_count(72, 1),
            needs_table: false,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![Some(45), Some(45), Some(45)],
                    vec![Some(44), Some(70), Some(44)],
                    vec![Some(45), Some(45), Some(45)],
                ],
            },
            output: ItemStack::with_count(73, 1),
            needs_table: true,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![Some(45), Some(41)],
                    vec![Some(41), Some(45)],
                ],
            },
            output: ItemStack::with_count(74, 1),
            needs_table: false,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![Some(4), Some(4), Some(4)],
                    vec![None, Some(40), None],
                    vec![None, Some(40), None],
                ],
            },
            output: ItemStack::with_count(50, 1),
            needs_table: true,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![Some(4), Some(4)],
                    vec![Some(4), Some(4)],
                    vec![None, Some(40)],
                ],
            },
            output: ItemStack::with_count(51, 1),
            needs_table: true,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![Some(4)],
                    vec![Some(4)],
                    vec![Some(40)],
                ],
            },
            output: ItemStack::with_count(52, 1),
            needs_table: true,
        },
        Recipe {
            kind: RecipeKind::Shaped {
                pattern: vec![
                    vec![None, Some(41)],
                    vec![None, Some(41)],
                    vec![None, Some(45)],
                ],
            },
            output: ItemStack::with_count(53, 1),
            needs_table: true,
        },
        Recipe {
            kind: RecipeKind::Smelting { input: 60, time_ms: 2000 },
            output: ItemStack::with_count(61, 1),
            needs_table: false,
        },
    ]
}

/// Intenta craftear con una rejilla de 2×2 o 3×3.
/// `grid` es row-major; retorna el resultado y los slots consumidos.
pub fn try_craft(
    grid: &[Option<ItemStack>],
    w: usize,
    h: usize,
    table: bool,
    book: &[Recipe],
) -> Option<(ItemStack, Vec<usize>)> {
    let present: Vec<ItemId> = grid.iter().flatten().map(|s| s.id).collect();
    if present.is_empty() {
        return None;
    }
    for r in book {
        if r.needs_table && !table {
            continue;
        }
        match &r.kind {
            RecipeKind::Shapeless { items } => {
                let mut want = items.clone();
                let mut ok = present.len() == want.len();
                if ok {
                    for p in &present {
                        if let Some(pos) = want.iter().position(|i| i == p) {
                            want.remove(pos);
                        } else {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok {
                    let slots = (0..grid.len()).filter(|i| grid[*i].is_some()).collect();
                    return Some((r.output, slots));
                }
            }
            RecipeKind::Shaped { pattern } => {
                // La rejilla debe coincidir con el patrón recortado.
                let pw = pattern.iter().map(|r| r.len()).max().unwrap_or(0);
                let ph = pattern.len();
                if ph > h {
                    continue;
                }
                let gw = w;
                for oy in 0..=(h.saturating_sub(ph)) {
                    for ox in 0..=(gw.saturating_sub(pw.min(gw))) {
                        let mut ok = true;
                        for (py, row) in pattern.iter().enumerate() {
                            for (px, cell) in row.iter().enumerate() {
                                let gx = ox + px;
                                let gy = oy + py;
                                if gx >= w || gy >= h {
                                    ok = false;
                                    break;
                                }
                                let g = grid[gy * w + gx];
                                match (cell, g) {
                                    (Some(want), Some(have)) if *want == have.id => {}
                                    (None, None) => {}
                                    _ => {
                                        ok = false;
                                        break;
                                    }
                                }
                            }
                            if !ok {
                                break;
                            }
                        }
                        // Rejilla no debe tener extras fuera del patrón.
                        if ok {
                            for gy in 0..h {
                                for gx in 0..w {
                                    let in_pat = gx >= ox
                                        && gx < ox + pw
                                        && gy >= oy
                                        && gy < oy + ph
                                        && pattern[gy - oy][gx - ox].is_some();
                                    if !in_pat && grid[gy * w + gx].is_some() {
                                        ok = false;
                                    }
                                }
                            }
                        }
                        if ok {
                            let slots = (0..grid.len()).filter(|i| grid[*i].is_some()).collect();
                            return Some((r.output, slots));
                        }
                    }
                }
            }
            RecipeKind::Smelting { .. } => {}
        }
    }
    None
}

/// Fundición de un item (horno).
pub fn smelt(item: ItemId, book: &[Recipe]) -> Option<(ItemStack, u32)> {
    for r in book {
        if let RecipeKind::Smelting { input, time_ms } = &r.kind {
            if *input == item {
                return Some((r.output, *time_ms));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shapeless_craft() {
        let book = recipe_book();
        let mut grid = vec![None; 4];
        grid[0] = Some(ItemStack::new(40));
        grid[1] = Some(ItemStack::new(40));
        grid[2] = Some(ItemStack::new(40));
        let r = try_craft(&grid, 2, 2, false, &book);
        assert!(r.is_some());
        assert_eq!(r.unwrap().0.id, 70);
    }
    #[test]
    fn shaped_pickaxe() {
        let book = recipe_book();
        let mut grid = vec![None; 9];
        grid[0] = Some(ItemStack::new(4));
        grid[1] = Some(ItemStack::new(4));
        grid[2] = Some(ItemStack::new(4));
        grid[4] = Some(ItemStack::new(40));
        grid[7] = Some(ItemStack::new(40));
        let r = try_craft(&grid, 3, 3, true, &book);
        assert!(r.is_some());
        assert_eq!(r.unwrap().0.id, 50);
    }
    #[test]
    fn no_craft_wrong_pattern() {
        let book = recipe_book();
        let grid = vec![Some(ItemStack::new(4)); 4];
        assert!(try_craft(&grid, 2, 2, false, &book).is_none());
    }
}
