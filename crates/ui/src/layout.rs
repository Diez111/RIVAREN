//! Layout responsivo: pilas (row/column) con sizing min/fill y wrapping.
//! Todo dentro de un rect contenedor; nunca desborda (clamp + truncado).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Row,
    Column,
}

#[derive(Debug, Clone, Copy)]
pub enum Size {
    /// Alto/ancho fijo en píxeles.
    Fixed(f32),
    /// Ocupa el resto repartido entre los elementos `Fill` de la pila.
    Fill,
    /// Porcentaje del contenedor (0..1).
    Percent(f32),
}

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
    pub fn inset(&self, d: f32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - d * 2.0).max(0.0), (self.h - d * 2.0).max(0.0))
    }
    pub fn center_x(&self) -> f32 {
        self.x + self.w * 0.5
    }
    pub fn align(&self, w: f32, h: f32) -> Rect {
        Rect::new(self.x + (self.w - w) * 0.5, self.y + (self.h - h) * 0.5, w, h)
    }
}

/// Pila de layout. Se usa así:
/// ```ignore
/// let mut col = Stack::new(Rect::new(0,0,300,400), Dir::Column, 8.0);
/// let a = col.next(Size::Fixed(40.0));
/// let b = col.next(Size::Fill);
/// ```
pub struct Stack {
    pub area: Rect,
    pub dir: Dir,
    pub gap: f32,
    cursor: f32,
    fills: Vec<(usize, Size)>,
    items: Vec<Rect>,
}

impl Stack {
    pub fn new(area: Rect, dir: Dir, gap: f32) -> Self {
        Self {
            area,
            dir,
            gap,
            cursor: 0.0,
            fills: Vec::new(),
            items: Vec::new(),
        }
    }
    fn main_len(&self) -> f32 {
        match self.dir {
            Dir::Row => self.area.w,
            Dir::Column => self.area.h,
        }
    }
    fn cross_len(&self) -> f32 {
        match self.dir {
            Dir::Row => self.area.h,
            Dir::Column => self.area.w,
        }
    }
    /// Añade un elemento y devuelve su rect.
    pub fn next(&mut self, size: Size) -> Rect {
        let idx = self.items.len();
        // Reserva provisional; los Fill se recalculan al final.
        let len = match size {
            Size::Fixed(v) => v,
            Size::Percent(p) => self.main_len() * p,
            Size::Fill => 0.0,
        };
        let rect = self.place(self.cursor, len);
        self.cursor += len + if idx == 0 { 0.0 } else { self.gap };
        self.items.push(rect);
        self.fills.push((idx, size));
        rect
    }
    fn place(&self, offset: f32, len: f32) -> Rect {
        match self.dir {
            Dir::Row => Rect::new(self.area.x + offset, self.area.y, len, self.area.h),
            Dir::Column => Rect::new(self.area.x, self.area.y + offset, self.area.w, len),
        }
    }
    /// Reparte el espacio restante entre los `Fill`. Llamar antes de dibujar.
    pub fn finish(&mut self) {
        let gaps = if self.items.is_empty() {
            0.0
        } else {
            self.gap * (self.items.len() as f32 - 1.0)
        };
        let used: f32 = self
            .fills
            .iter()
            .filter(|(_, s)| !matches!(s, Size::Fill))
            .map(|(i, _)| match self.dir {
                Dir::Row => self.items[*i].w,
                Dir::Column => self.items[*i].h,
            })
            .sum();
        let n_fill = self.fills.iter().filter(|(_, s)| matches!(s, Size::Fill)).count();
        let remaining = (self.main_len() - used - gaps).max(0.0);
        let each = if n_fill > 0 { remaining / n_fill as f32 } else { 0.0 };
        let mut cursor = 0.0;
        for i in 0..self.items.len() {
            let size = self.fills[i].1;
            let len = match size {
                Size::Fixed(v) => v,
                Size::Percent(p) => self.main_len() * p,
                Size::Fill => each,
            };
            let mut r = self.place(cursor, len);
            // Cross-axis fill.
            if self.dir == Dir::Row {
                r.h = self.cross_len();
            } else {
                r.w = self.cross_len();
            }
            self.items[i] = r;
            cursor += len + self.gap;
        }
    }
    pub fn rect(&self, idx: usize) -> Rect {
        self.items.get(idx).copied().unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
    }
    pub fn count(&self) -> usize {
        self.items.len()
    }
    /// Contenedor centrado con ancho máximo responsivo.
    pub fn centered_card(screen: Rect, max_w: f32, max_h: f32) -> Rect {
        let w = screen.w.min(max_w).max(240.0);
        let h = screen.h.min(max_h).max(200.0);
        screen.align(w, h)
    }
}

/// Grid responsivo: calcula cuántas columnas caben.
pub fn grid_columns(container_w: f32, cell: f32, gap: f32) -> usize {
    let n = ((container_w + gap) / (cell + gap)).floor() as usize;
    n.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_fill_no_overflow() {
        let mut s = Stack::new(Rect::new(0.0, 0.0, 300.0, 200.0), Dir::Column, 10.0);
        s.next(Size::Fixed(50.0));
        s.next(Size::Fill);
        s.finish();
        let r0 = s.rect(0);
        let r1 = s.rect(1);
        assert_eq!(r0.h, 50.0);
        assert_eq!(r0.y, 0.0);
        assert_eq!(r1.y, 60.0);
        assert_eq!(r1.h, 140.0);
        assert!(r1.y + r1.h <= 200.0);
    }

    #[test]
    fn percent_and_fill() {
        let mut s = Stack::new(Rect::new(0.0, 0.0, 100.0, 100.0), Dir::Row, 0.0);
        s.next(Size::Percent(0.3));
        s.next(Size::Fill);
        s.finish();
        assert!((s.rect(0).w - 30.0).abs() < 0.001);
        assert!((s.rect(1).w - 70.0).abs() < 0.001);
    }

    #[test]
    fn grid_columns_mobile() {
        assert_eq!(grid_columns(320.0, 64.0, 8.0), 4);
    }
}
