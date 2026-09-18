//! Widgets: construyen el DrawList y devuelven interacción.
//! Todo con clipping y sin desbordes; tamaños táctiles en móvil.

use crate::input::InputState;
use crate::layout::Rect;
use crate::theme::*;
use rivaren_core::{DrawList, TextAlign};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default)]
pub struct Response {
    pub hovered: bool,
    pub clicked: bool,
    pub changed: bool,
    pub right_clicked: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SlotView {
    pub color: [f32; 4],
    pub label: String,
    pub count: u8,
    /// 0..1, 1 = intacto.
    pub durability: f32,
}

pub struct Ui<'a> {
    pub draw: &'a mut DrawList,
    pub input: &'a InputState,
    pub palette: Palette,
    pub breakpoint: Breakpoint,
    hot: Option<u64>,
    active: Option<u64>,
    pub focus: Option<u64>,
    pub text_buffers: HashMap<u64, String>,
    pub clip: Rect,
    pub frame: u64,
}

impl<'a> Ui<'a> {
    pub fn new(
        draw: &'a mut DrawList,
        input: &'a InputState,
        breakpoint: Breakpoint,
        frame: u64,
    ) -> Self {
        let screen = Rect::new(0.0, 0.0, input.screen.0, input.screen.1);
        Self {
            draw,
            input,
            palette: palette(),
            breakpoint,
            hot: None,
            active: None,
            focus: None,
            text_buffers: HashMap::new(),
            clip: screen,
            frame,
        }
    }

    fn hit(&self, rect: Rect) -> bool {
        rect.contains(self.input.mouse_x, self.input.mouse_y)
            && self.clip.contains(self.input.mouse_x, self.input.mouse_y)
    }

    fn begin_interact(&mut self, id: u64, rect: Rect) -> Response {
        let hovered = self.hit(rect);
        if hovered {
            self.hot = Some(id);
        }
        let mut clicked = false;
        let right_clicked = false;
        if hovered && self.input.mouse_pressed {
            self.active = Some(id);
        }
        if self.active == Some(id) && self.input.mouse_released {
            if hovered {
                clicked = true;
            }
            self.active = None;
        }
        // Táctil: press = click directo.
        if hovered && self.input.touch.is_some() && self.input.mouse_pressed {
            clicked = true;
        }
        Response {
            hovered,
            clicked,
            changed: false,
            right_clicked,
        }
    }

    // ── primitivas ──

    pub fn panel(&mut self, rect: Rect) {
        self.draw.rounded(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            RADIUS,
            self.palette.panel,
        );
    }

    pub fn panel_titled(&mut self, rect: Rect, title: &str) {
        self.panel(rect);
        self.draw.text(
            title,
            rect.x + SPACE,
            rect.y + SPACE * 0.7,
            HEADING_SIZE * self.breakpoint.scale(),
            self.palette.text_title,
        );
    }

    pub fn label(&mut self, rect: Rect, text: &str, color: [f32; 4]) {
        self.draw.text(
            text,
            rect.x,
            rect.y,
            BODY_SIZE * self.breakpoint.scale(),
            color,
        );
    }

    pub fn label_sized(&mut self, rect: Rect, text: &str, size: f32, color: [f32; 4]) {
        self.draw.text(text, rect.x, rect.y, size, color);
    }

    pub fn label_centered(&mut self, rect: Rect, text: &str, size: f32, color: [f32; 4]) {
        let y = rect.y + (rect.h - size) * 0.5;
        self.draw
            .text_centered(text, rect.center_x(), y, size, color);
    }

    pub fn separator(&mut self, rect: Rect) {
        self.draw
            .rect(rect.x, rect.y + rect.h * 0.5 - 0.5, rect.w, 1.0, [1.0, 1.0, 1.0, 0.08]);
    }

    // ── widgets ──

    pub fn button(&mut self, id: u64, rect: Rect, label: &str) -> Response {
        let r = self.begin_interact(id, rect);
        let hover = r.hovered;
        let pressed = self.active == Some(id);
        let (mut color, border) = if pressed {
            (self.palette.accent_dim, self.palette.accent)
        } else if hover {
            (self.palette.panel_light, self.palette.accent)
        } else {
            (self.palette.panel, [1.0, 1.0, 1.0, 0.10])
        };
        color[3] = color[3].max(0.9);
        self.draw
            .bordered(rect.x, rect.y, rect.w, rect.h, RADIUS_SMALL, 1.5, color, border);
        let size = BODY_SIZE * self.breakpoint.scale();
        let text_color = if pressed || hover {
            self.palette.text_title
        } else {
            self.palette.text
        };
        self.label_centered(rect, label, size, text_color);
        r
    }

    pub fn button_primary(&mut self, id: u64, rect: Rect, label: &str) -> Response {
        let r = self.begin_interact(id, rect);
        let hover = r.hovered;
        let pressed = self.active == Some(id);
        let color = if pressed {
            [0.75, 0.55, 0.18, 1.0]
        } else if hover {
            [1.0, 0.80, 0.35, 1.0]
        } else {
            self.palette.accent
        };
        self.draw
            .bordered(rect.x, rect.y, rect.w, rect.h, RADIUS_SMALL, 0.0, color, color);
        self.label_centered(
            rect,
            label,
            BODY_SIZE * self.breakpoint.scale(),
            [0.08, 0.06, 0.03, 1.0],
        );
        r
    }

    pub fn button_danger(&mut self, id: u64, rect: Rect, label: &str) -> Response {
        let r = self.begin_interact(id, rect);
        let hover = r.hovered;
        let color = if hover { [0.95, 0.38, 0.32, 1.0] } else { self.palette.danger };
        self.draw
            .bordered(rect.x, rect.y, rect.w, rect.h, RADIUS_SMALL, 0.0, color, color);
        self.label_centered(
            rect,
            label,
            BODY_SIZE * self.breakpoint.scale(),
            [1.0, 1.0, 1.0, 1.0],
        );
        r
    }

    pub fn toggle(&mut self, id: u64, rect: Rect, label: &str, value: &mut bool) -> Response {
        let r = self.begin_interact(id, rect);
        let mut resp = r;
        if r.clicked {
            *value = !*value;
            resp.changed = true;
        }
        let knob_w = rect.h * 0.9;
        let track = Rect::new(rect.x, rect.y, knob_w * 2.0, rect.h);
        let on = *value;
        let track_color = if on {
            self.palette.accent_dim
        } else {
            [0.25, 0.25, 0.28, 1.0]
        };
        self.draw
            .rounded(track.x, track.y, track.w, track.h, rect.h * 0.5, track_color);
        let kx = if on { track.x + track.w - knob_w } else { track.x };
        let knob_color = if resp.hovered || on {
            self.palette.accent
        } else {
            self.palette.text_dim
        };
        self.draw
            .rounded(kx + 2.0, track.y + 2.0, knob_w - 4.0, rect.h - 4.0, rect.h * 0.5, knob_color);
        let text_rect = Rect::new(rect.x + track.w + SPACE, rect.y, rect.w - track.w - SPACE, rect.h);
        self.draw.text(
            label,
            text_rect.x,
            text_rect.y + (rect.h - BODY_SIZE) * 0.5,
            BODY_SIZE * self.breakpoint.scale(),
            self.palette.text,
        );
        resp
    }

    pub fn slider(
        &mut self,
        id: u64,
        rect: Rect,
        label: &str,
        value: &mut f32,
        min: f32,
        max: f32,
    ) -> Response {
        let r = self.begin_interact(id, rect);
        let mut resp = r;
        let label_h = (BODY_SIZE * self.breakpoint.scale()) + 4.0;
        self.draw.text(
            format!("{label}: {:.2}", *value),
            rect.x,
            rect.y,
            BODY_SIZE * self.breakpoint.scale(),
            self.palette.text,
        );
        let track = Rect::new(
            rect.x,
            rect.y + label_h,
            rect.w,
            (rect.h - label_h).max(10.0),
        );
        let t = ((*value - min) / (max - min).max(1e-6)).clamp(0.0, 1.0);
        self.draw.rounded(
            track.x,
            track.y + track.h * 0.5 - 3.0,
            track.w,
            6.0,
            3.0,
            [0.25, 0.25, 0.28, 1.0],
        );
        self.draw.rounded(
            track.x,
            track.y + track.h * 0.5 - 3.0,
            track.w * t,
            6.0,
            3.0,
            self.palette.accent_dim,
        );
        let knob = Rect::new(
            track.x + track.w * t - 9.0,
            track.y + track.h * 0.5 - 9.0,
            18.0,
            18.0,
        );
        let knob_color = if resp.hovered || self.active == Some(id) {
            self.palette.accent
        } else {
            self.palette.text
        };
        self.draw.rounded(knob.x, knob.y, knob.w, knob.h, 9.0, knob_color);
        if self.active == Some(id) && self.input.mouse_down {
            let nt = ((self.input.mouse_x - track.x) / track.w.max(1.0)).clamp(0.0, 1.0);
            *value = min + nt * (max - min);
            resp.changed = true;
        }
        resp
    }

    pub fn text_field(
        &mut self,
        id: u64,
        rect: Rect,
        buffer: &mut String,
        placeholder: &str,
    ) -> Response {
        let r = self.begin_interact(id, rect);
        let mut resp = r;
        if r.clicked {
            self.focus = Some(id);
        } else if self.input.mouse_pressed && !r.hovered && self.focus == Some(id) {
            self.focus = None;
        }
        let focused = self.focus == Some(id);
        if focused {
            for ch in self.input.text_input.chars() {
                if ch == '\u{8}' {
                    buffer.pop();
                } else if !ch.is_control() {
                    buffer.push(ch);
                }
                resp.changed = true;
            }
            if self.input.key_pressed(crate::input::Key::Backspace) {
                buffer.pop();
                resp.changed = true;
            }
        }
        let border = if focused {
            self.palette.accent
        } else if r.hovered {
            [1.0, 1.0, 1.0, 0.25]
        } else {
            [1.0, 1.0, 1.0, 0.12]
        };
        self.draw.bordered(
            rect.x,
            rect.y,
            rect.w,
            rect.h,
            RADIUS_SMALL,
            1.5,
            self.palette.bg_deep,
            border,
        );
        let size = BODY_SIZE * self.breakpoint.scale();
        let empty = buffer.is_empty();
        let text: &str = if empty { placeholder } else { buffer.as_str() };
        let color = if empty {
            self.palette.text_dim
        } else {
            self.palette.text
        };
        self.draw.text(
            text,
            rect.x + 8.0,
            rect.y + (rect.h - size) * 0.5,
            size,
            color,
        );
        if focused && (self.frame / 30).is_multiple_of(2) {
            // Cursor aproximado tras el texto.
            let w = buffer.chars().count() as f32 * size * 0.55;
            self.draw.rect(
                rect.x + 8.0 + w,
                rect.y + 6.0,
                2.0,
                rect.h - 12.0,
                self.palette.accent,
            );
        }
        resp
    }

    pub fn progress(&mut self, rect: Rect, value: f32, color: [f32; 4]) {
        self.draw
            .rounded(rect.x, rect.y, rect.w, rect.h, rect.h * 0.5, [0.15, 0.15, 0.18, 1.0]);
        let v = value.clamp(0.0, 1.0);
        if v > 0.0 {
            self.draw.rounded(
                rect.x,
                rect.y,
                rect.w * v,
                rect.h,
                rect.h * 0.5,
                color,
            );
        }
    }

    pub fn tabs(
        &mut self,
        base_id: u64,
        rect: Rect,
        labels: &[&str],
        selected: &mut usize,
    ) -> Response {
        let n = labels.len().max(1);
        let w = rect.w / n as f32;
        let mut resp = Response::default();
        for (i, label) in labels.iter().enumerate() {
            let r = Rect::new(rect.x + w * i as f32, rect.y, w, rect.h);
            let id = base_id + i as u64;
            let inter = self.begin_interact(id, r);
            let active = *selected == i;
            let bg = if active {
                self.palette.accent_dim
            } else if inter.hovered {
                self.palette.panel_light
            } else {
                self.palette.panel
            };
            self.draw
                .rounded(r.x + 2.0, r.y, r.w - 4.0, r.h, RADIUS_SMALL, bg);
            self.label_centered(
                r,
                label,
                BODY_SIZE * self.breakpoint.scale(),
                if active { self.palette.text_title } else { self.palette.text },
            );
            if inter.clicked {
                *selected = i;
                resp.changed = true;
                resp.clicked = true;
            }
            resp.hovered |= inter.hovered;
        }
        resp
    }

    pub fn item_slot(
        &mut self,
        id: u64,
        rect: Rect,
        slot: Option<&SlotView>,
        highlight: bool,
    ) -> Response {
        let r = self.begin_interact(id, rect);
        let bg = if highlight {
            [0.30, 0.28, 0.34, 1.0]
        } else if r.hovered {
            self.palette.slot_hover
        } else {
            self.palette.slot
        };
        self.draw
            .bordered(rect.x, rect.y, rect.w, rect.h, RADIUS_SMALL, 1.2, bg, [1.0, 1.0, 1.0, 0.10]);
        if let Some(s) = slot {
            let pad = rect.w * 0.18;
            self.draw.rounded(
                rect.x + pad,
                rect.y + pad,
                rect.w - pad * 2.0,
                rect.h - pad * 2.0,
                3.0,
                s.color,
            );
            if s.count > 1 {
                let size = SMALL_SIZE * self.breakpoint.scale();
                self.draw.text(
                    format!("{}", s.count),
                    rect.x + rect.w - size * 0.7,
                    rect.y + rect.h - size - 2.0,
                    size,
                    self.palette.text,
                );
            }
            if s.durability < 1.0 {
                let bar = Rect::new(rect.x + 4.0, rect.y + rect.h - 6.0, rect.w - 8.0, 3.0);
                let hue = if s.durability > 0.5 {
                    self.palette.success
                } else if s.durability > 0.2 {
                    [0.9, 0.75, 0.2, 1.0]
                } else {
                    self.palette.danger
                };
                self.progress(bar, s.durability, hue);
            }
        }
        r
    }

    pub fn tooltip(&mut self, rect: Rect, text: &str) {
        let size = SMALL_SIZE * self.breakpoint.scale();
        let w = (text.chars().count() as f32 * size * 0.55).min(280.0) + 16.0;
        let h = size + 12.0;
        let x = (self.input.mouse_x + 14.0).min(self.input.screen.0 - w - 4.0);
        let y = (self.input.mouse_y + 14.0).min(self.input.screen.1 - h - 4.0);
        let _ = rect;
        self.draw
            .bordered(x, y, w, h, RADIUS_SMALL, 1.0, [0.06, 0.06, 0.08, 0.97], [1.0, 1.0, 1.0, 0.15]);
        self.draw.text(text, x + 8.0, y + 6.0, size, self.palette.text);
    }

    pub fn heading(&mut self, rect: Rect, text: &str) {
        self.draw.text(
            text,
            rect.x,
            rect.y,
            HEADING_SIZE * self.breakpoint.scale(),
            self.palette.text_title,
        );
    }

    pub fn title(&mut self, rect: Rect, text: &str) {
        self.draw.text_centered(
            text,
            rect.center_x(),
            rect.y,
            TITLE_SIZE * self.breakpoint.scale().min(1.0),
            self.palette.text_title,
        );
    }

    pub fn dim(&mut self, rect: Rect, alpha: f32) {
        self.draw
            .rect(rect.x, rect.y, rect.w, rect.h, [0.0, 0.0, 0.0, alpha]);
    }

    pub fn text_wrapped(&mut self, rect: Rect, text: &str, size: f32, color: [f32; 4]) {
        self.draw.texts.push(rivaren_core::UiText {
            text: text.to_string(),
            x: rect.x,
            y: rect.y,
            size,
            color,
            align: TextAlign::Left,
            max_width: rect.w,
            clip: [rect.x, rect.y, rect.x + rect.w, rect.y + rect.h],
            bold: false,
        });
    }
}
