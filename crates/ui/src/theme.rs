//! Tema visual RIVAREN: paleta, tipografía, espaciado, breakpoints.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breakpoint {
    Mobile,
    Tablet,
    Desktop,
}

impl Breakpoint {
    pub fn of(width: f32) -> Self {
        if width < 640.0 {
            Self::Mobile
        } else if width < 1024.0 {
            Self::Tablet
        } else {
            Self::Desktop
        }
    }
    pub fn scale(&self) -> f32 {
        match self {
            Self::Mobile => 0.9,
            Self::Tablet => 1.0,
            Self::Desktop => 1.0,
        }
    }
    pub fn touch_target(&self) -> f32 {
        match self {
            Self::Mobile => 48.0,
            _ => 36.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Fondo de pantalla completa (piedra oscura).
    pub bg: [f32; 4],
    pub bg_deep: [f32; 4],
    /// Panel principal.
    pub panel: [f32; 4],
    pub panel_light: [f32; 4],
    /// Acento principal (ámbar cristal).
    pub accent: [f32; 4],
    pub accent_dim: [f32; 4],
    /// Texto.
    pub text: [f32; 4],
    pub text_dim: [f32; 4],
    pub text_title: [f32; 4],
    /// Estados.
    pub danger: [f32; 4],
    pub success: [f32; 4],
    /// Karma.
    pub compassion: [f32; 4],
    pub justice: [f32; 4],
    pub wisdom: [f32; 4],
    /// Slot de inventario.
    pub slot: [f32; 4],
    pub slot_hover: [f32; 4],
    /// HUD.
    pub hud_bg: [f32; 4],
    pub crosshair: [f32; 4],
}

pub fn palette() -> Palette {
    Palette {
        bg: [0.07, 0.07, 0.09, 1.0],
        bg_deep: [0.04, 0.04, 0.06, 1.0],
        panel: [0.12, 0.12, 0.15, 0.96],
        panel_light: [0.18, 0.18, 0.22, 0.98],
        accent: [0.95, 0.72, 0.28, 1.0],
        accent_dim: [0.55, 0.42, 0.18, 1.0],
        text: [0.92, 0.92, 0.90, 1.0],
        text_dim: [0.62, 0.62, 0.60, 1.0],
        text_title: [1.0, 0.86, 0.55, 1.0],
        danger: [0.85, 0.28, 0.25, 1.0],
        success: [0.35, 0.78, 0.42, 1.0],
        compassion: [0.95, 0.78, 0.35, 1.0],
        justice: [0.40, 0.62, 0.95, 1.0],
        wisdom: [0.68, 0.45, 0.95, 1.0],
        slot: [0.18, 0.18, 0.22, 0.95],
        slot_hover: [0.30, 0.28, 0.34, 1.0],
        hud_bg: [0.05, 0.05, 0.07, 0.72],
        crosshair: [1.0, 1.0, 1.0, 0.85],
    }
}

pub const RADIUS: f32 = 8.0;
pub const RADIUS_SMALL: f32 = 5.0;
pub const SPACE: f32 = 12.0;
pub const TITLE_SIZE: f32 = 46.0;
pub const HEADING_SIZE: f32 = 24.0;
pub const BODY_SIZE: f32 = 17.0;
pub const SMALL_SIZE: f32 = 14.0;
