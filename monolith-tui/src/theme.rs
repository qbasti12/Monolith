//! Catppuccin Mocha color palette with truecolor, 256-color, and basic ANSI fallback.

use ratatui::style::Color;

/// Color capability tier detected from terminal environment variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTier {
    TrueColor,
    Ansi256,
    Ansi16,
}

impl ColorTier {
    /// Detects terminal color capability from `COLORTERM`, `TERM`, and `NO_COLOR`.
    pub fn detect() -> Self {
        if std::env::var_os("NO_COLOR").is_some() {
            return Self::Ansi16;
        }
        if let Ok(ct) = std::env::var("COLORTERM") {
            let ct_low = ct.to_ascii_lowercase();
            if ct_low.contains("truecolor") || ct_low.contains("24bit") {
                return Self::TrueColor;
            }
        }
        if let Ok(term) = std::env::var("TERM") {
            let t = term.to_ascii_lowercase();
            if t.contains("256color") || t.contains("kitty") || t.contains("alacritty") {
                return Self::Ansi256;
            }
            if t == "dumb" || t == "vt100" {
                return Self::Ansi16;
            }
        }
        // Modern terminals on macOS/Linux (e.g., Terminal.app, iTerm2, Ghostty, WezTerm) default
        // safely to 256 colors unless truecolor was explicitly advertised.
        if cfg!(target_os = "macos") || std::env::var_os("TERM_PROGRAM").is_some() {
            return Self::TrueColor;
        }
        Self::Ansi256
    }
}

/// Semantic Catppuccin Mocha theme tokens mapped to the active [`ColorTier`].
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub tier: ColorTier,
    pub base: Color,
    pub mantle: Color,
    pub crust: Color,
    pub surface0: Color,
    pub surface1: Color,
    pub surface2: Color,
    pub text: Color,
    pub subtext0: Color,
    pub subtext1: Color,
    pub overlay1: Color,
    pub green: Color,
    pub yellow: Color,
    pub peach: Color,
    pub red: Color,
    pub blue: Color,
    pub sapphire: Color,
    pub lavender: Color,
    pub teal: Color,
    pub mauve: Color,
    // Chess board palette
    pub board_light: Color,
    pub board_dark: Color,
    pub board_last_light: Color,
    pub board_last_dark: Color,
    pub board_cursor: Color,
    pub board_selected: Color,
    pub board_check: Color,
    pub piece_white: Color,
    pub piece_black: Color,
}

impl Theme {
    /// Builds the Catppuccin Mocha palette for the detected environment tier.
    pub fn detect() -> Self {
        Self::for_tier(ColorTier::detect())
    }

    /// Builds the Catppuccin Mocha palette for an explicit [`ColorTier`].
    pub fn for_tier(tier: ColorTier) -> Self {
        let c = |r: u8, g: u8, b: u8, idx256: u8, ansi16: Color| -> Color {
            match tier {
                ColorTier::TrueColor => Color::Rgb(r, g, b),
                ColorTier::Ansi256 => Color::Indexed(idx256),
                ColorTier::Ansi16 => ansi16,
            }
        };

        Self {
            tier,
            base: c(0x1e, 0x1e, 0x2e, 234, Color::Black),
            mantle: c(0x18, 0x18, 0x25, 233, Color::Black),
            crust: c(0x11, 0x11, 0x1b, 232, Color::Black),
            surface0: c(0x31, 0x32, 0x44, 236, Color::DarkGray),
            surface1: c(0x45, 0x47, 0x5a, 238, Color::DarkGray),
            surface2: c(0x58, 0x5b, 0x70, 240, Color::Gray),
            text: c(0xcd, 0xd6, 0xf4, 189, Color::White),
            subtext0: c(0xa6, 0xad, 0xc8, 146, Color::Gray),
            subtext1: c(0xba, 0xc2, 0xde, 152, Color::Gray),
            overlay1: c(0x7f, 0x84, 0x9c, 103, Color::Gray),
            green: c(0xa6, 0xe3, 0xa1, 151, Color::Green),
            yellow: c(0xf9, 0xe2, 0xaf, 223, Color::Yellow),
            peach: c(0xfa, 0xb3, 0x87, 216, Color::LightRed),
            red: c(0xf3, 0x8b, 0xa8, 211, Color::Red),
            blue: c(0x89, 0xb4, 0xfa, 111, Color::Blue),
            sapphire: c(0x74, 0xc7, 0xec, 117, Color::Cyan),
            lavender: c(0xb4, 0xbe, 0xfe, 147, Color::LightBlue),
            teal: c(0x94, 0xe2, 0xd5, 158, Color::LightCyan),
            mauve: c(0xcb, 0xa6, 0xf7, 183, Color::Magenta),
            board_light: c(0x93, 0x99, 0xb2, 103, Color::Gray),
            board_dark: c(0x58, 0x5b, 0x70, 240, Color::DarkGray),
            board_last_light: c(0x84, 0xa5, 0x9d, 109, Color::Cyan),
            board_last_dark: c(0x4b, 0x6b, 0x72, 66, Color::DarkGray),
            board_cursor: c(0xdf, 0xc4, 0x82, 180, Color::Yellow),
            board_selected: c(0x78, 0xb8, 0x82, 108, Color::Green),
            board_check: c(0xc7, 0x5c, 0x75, 168, Color::Red),
            piece_white: c(0xff, 0xff, 0xff, 231, Color::White),
            piece_black: c(0x11, 0x11, 0x1b, 232, Color::Black),
        }
    }
}

/// Converts centipawns (White POV) into White win probability in `[0.0, 1.0]`.
///
/// Uses the logistic curve `1 / (1 + exp(-0.004 * cp))` and clamps extreme/mate values cleanly.
pub fn eval_to_win_prob(score_cp: Option<i32>, score_mate: Option<i32>) -> f64 {
    if let Some(mate) = score_mate {
        return if mate > 0 { 1.0 } else { 0.0 };
    }
    let cp = score_cp.unwrap_or(0).clamp(-3000, 3000) as f64;
    1.0 / (1.0 + (-0.004 * cp).exp())
}

/// Formats an evaluation score (White POV) concisely for the eval bar and engine headers.
pub fn format_eval_short(score_cp: Option<i32>, score_mate: Option<i32>) -> String {
    if let Some(m) = score_mate {
        if m > 0 {
            return format!("+M{m}");
        } else {
            return format!("-M{}", m.saturating_abs());
        }
    }
    let cp = score_cp.unwrap_or(0);
    format!("{:+.2}", cp as f64 / 100.0)
}
