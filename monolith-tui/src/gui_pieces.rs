//! Chess piece set themes and SVG image rendering for Monolith GUI.

use egui::{Color32, Image, Painter, Rect, Ui};
use monolith_core::{Color, PieceType};

use crate::gui_board::draw_scalable_piece;

/// Available chess piece vector sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PieceSet {
    #[default]
    LichessClassic,
    LichessStaunty,
    MonolithMinimal,
    VectorProcedural,
}

impl PieceSet {
    pub const ALL: [PieceSet; 4] = [
        PieceSet::LichessClassic,
        PieceSet::LichessStaunty,
        PieceSet::MonolithMinimal,
        PieceSet::VectorProcedural,
    ];

    pub fn display_name(&self) -> &'static str {
        match self {
            PieceSet::LichessClassic => "Lichess Classic (cburnett)",
            PieceSet::LichessStaunty => "Lichess Staunty",
            PieceSet::MonolithMinimal => "Monolith Minimal",
            PieceSet::VectorProcedural => "Vektor-Staunton (Prozedural)",
        }
    }
}

// Embedded Lichess Classic (cburnett) SVGs
const LICHESS_WP: &[u8] = include_bytes!("../assets/pieces/lichess/wP.svg");
const LICHESS_WN: &[u8] = include_bytes!("../assets/pieces/lichess/wN.svg");
const LICHESS_WB: &[u8] = include_bytes!("../assets/pieces/lichess/wB.svg");
const LICHESS_WR: &[u8] = include_bytes!("../assets/pieces/lichess/wR.svg");
const LICHESS_WQ: &[u8] = include_bytes!("../assets/pieces/lichess/wQ.svg");
const LICHESS_WK: &[u8] = include_bytes!("../assets/pieces/lichess/wK.svg");
const LICHESS_BP: &[u8] = include_bytes!("../assets/pieces/lichess/bP.svg");
const LICHESS_BN: &[u8] = include_bytes!("../assets/pieces/lichess/bN.svg");
const LICHESS_BB: &[u8] = include_bytes!("../assets/pieces/lichess/bB.svg");
const LICHESS_BR: &[u8] = include_bytes!("../assets/pieces/lichess/bR.svg");
const LICHESS_BQ: &[u8] = include_bytes!("../assets/pieces/lichess/bQ.svg");
const LICHESS_BK: &[u8] = include_bytes!("../assets/pieces/lichess/bK.svg");

// Embedded Lichess Staunty SVGs
const STAUNTY_WP: &[u8] = include_bytes!("../assets/pieces/staunty/wP.svg");
const STAUNTY_WN: &[u8] = include_bytes!("../assets/pieces/staunty/wN.svg");
const STAUNTY_WB: &[u8] = include_bytes!("../assets/pieces/staunty/wB.svg");
const STAUNTY_WR: &[u8] = include_bytes!("../assets/pieces/staunty/wR.svg");
const STAUNTY_WQ: &[u8] = include_bytes!("../assets/pieces/staunty/wQ.svg");
const STAUNTY_WK: &[u8] = include_bytes!("../assets/pieces/staunty/wK.svg");
const STAUNTY_BP: &[u8] = include_bytes!("../assets/pieces/staunty/bP.svg");
const STAUNTY_BN: &[u8] = include_bytes!("../assets/pieces/staunty/bN.svg");
const STAUNTY_BB: &[u8] = include_bytes!("../assets/pieces/staunty/bB.svg");
const STAUNTY_BR: &[u8] = include_bytes!("../assets/pieces/staunty/bR.svg");
const STAUNTY_BQ: &[u8] = include_bytes!("../assets/pieces/staunty/bQ.svg");
const STAUNTY_BK: &[u8] = include_bytes!("../assets/pieces/staunty/bK.svg");

// Embedded Monolith Minimal SVGs
const MINIMAL_WP: &[u8] = include_bytes!("../assets/pieces/minimal/wP.svg");
const MINIMAL_WN: &[u8] = include_bytes!("../assets/pieces/minimal/wN.svg");
const MINIMAL_WB: &[u8] = include_bytes!("../assets/pieces/minimal/wB.svg");
const MINIMAL_WR: &[u8] = include_bytes!("../assets/pieces/minimal/wR.svg");
const MINIMAL_WQ: &[u8] = include_bytes!("../assets/pieces/minimal/wQ.svg");
const MINIMAL_WK: &[u8] = include_bytes!("../assets/pieces/minimal/wK.svg");
const MINIMAL_BP: &[u8] = include_bytes!("../assets/pieces/minimal/bP.svg");
const MINIMAL_BN: &[u8] = include_bytes!("../assets/pieces/minimal/bN.svg");
const MINIMAL_BB: &[u8] = include_bytes!("../assets/pieces/minimal/bB.svg");
const MINIMAL_BR: &[u8] = include_bytes!("../assets/pieces/minimal/bR.svg");
const MINIMAL_BQ: &[u8] = include_bytes!("../assets/pieces/minimal/bQ.svg");
const MINIMAL_BK: &[u8] = include_bytes!("../assets/pieces/minimal/bK.svg");

/// Resolves piece SVG bytes and URI for the chosen piece set.
fn piece_svg_bytes(
    set: PieceSet,
    color: Color,
    piece_type: PieceType,
) -> (&'static str, &'static [u8]) {
    match set {
        PieceSet::LichessClassic => match (color, piece_type) {
            (Color::White, PieceType::Pawn) => ("bytes://lic_wp.svg", LICHESS_WP),
            (Color::White, PieceType::Knight) => ("bytes://lic_wn.svg", LICHESS_WN),
            (Color::White, PieceType::Bishop) => ("bytes://lic_wb.svg", LICHESS_WB),
            (Color::White, PieceType::Rook) => ("bytes://lic_wr.svg", LICHESS_WR),
            (Color::White, PieceType::Queen) => ("bytes://lic_wq.svg", LICHESS_WQ),
            (Color::White, PieceType::King) => ("bytes://lic_wk.svg", LICHESS_WK),
            (Color::Black, PieceType::Pawn) => ("bytes://lic_bp.svg", LICHESS_BP),
            (Color::Black, PieceType::Knight) => ("bytes://lic_bn.svg", LICHESS_BN),
            (Color::Black, PieceType::Bishop) => ("bytes://lic_bb.svg", LICHESS_BB),
            (Color::Black, PieceType::Rook) => ("bytes://lic_br.svg", LICHESS_BR),
            (Color::Black, PieceType::Queen) => ("bytes://lic_bq.svg", LICHESS_BQ),
            (Color::Black, PieceType::King) => ("bytes://lic_bk.svg", LICHESS_BK),
        },
        PieceSet::LichessStaunty => match (color, piece_type) {
            (Color::White, PieceType::Pawn) => ("bytes://sty_wp.svg", STAUNTY_WP),
            (Color::White, PieceType::Knight) => ("bytes://sty_wn.svg", STAUNTY_WN),
            (Color::White, PieceType::Bishop) => ("bytes://sty_wb.svg", STAUNTY_WB),
            (Color::White, PieceType::Rook) => ("bytes://sty_wr.svg", STAUNTY_WR),
            (Color::White, PieceType::Queen) => ("bytes://sty_wq.svg", STAUNTY_WQ),
            (Color::White, PieceType::King) => ("bytes://sty_wk.svg", STAUNTY_WK),
            (Color::Black, PieceType::Pawn) => ("bytes://sty_bp.svg", STAUNTY_BP),
            (Color::Black, PieceType::Knight) => ("bytes://sty_bn.svg", STAUNTY_BN),
            (Color::Black, PieceType::Bishop) => ("bytes://sty_bb.svg", STAUNTY_BB),
            (Color::Black, PieceType::Rook) => ("bytes://sty_br.svg", STAUNTY_BR),
            (Color::Black, PieceType::Queen) => ("bytes://sty_bq.svg", STAUNTY_BQ),
            (Color::Black, PieceType::King) => ("bytes://sty_bk.svg", STAUNTY_BK),
        },
        PieceSet::MonolithMinimal => match (color, piece_type) {
            (Color::White, PieceType::Pawn) => ("bytes://min_wp.svg", MINIMAL_WP),
            (Color::White, PieceType::Knight) => ("bytes://min_wn.svg", MINIMAL_WN),
            (Color::White, PieceType::Bishop) => ("bytes://min_wb.svg", MINIMAL_WB),
            (Color::White, PieceType::Rook) => ("bytes://min_wr.svg", MINIMAL_WR),
            (Color::White, PieceType::Queen) => ("bytes://min_wq.svg", MINIMAL_WQ),
            (Color::White, PieceType::King) => ("bytes://min_wk.svg", MINIMAL_WK),
            (Color::Black, PieceType::Pawn) => ("bytes://min_bp.svg", MINIMAL_BP),
            (Color::Black, PieceType::Knight) => ("bytes://min_bn.svg", MINIMAL_BN),
            (Color::Black, PieceType::Bishop) => ("bytes://min_bb.svg", MINIMAL_BB),
            (Color::Black, PieceType::Rook) => ("bytes://min_br.svg", MINIMAL_BR),
            (Color::Black, PieceType::Queen) => ("bytes://min_bq.svg", MINIMAL_BQ),
            (Color::Black, PieceType::King) => ("bytes://min_bk.svg", MINIMAL_BK),
        },
        PieceSet::VectorProcedural => ("", &[]),
    }
}

/// Renders a chess piece in the given rectangle using the configured piece set.
pub fn render_piece_styled(
    ui: &mut Ui,
    painter: &Painter,
    rect: Rect,
    piece_type: PieceType,
    color: Color,
    set: PieceSet,
) {
    if set == PieceSet::VectorProcedural {
        let padding = rect.width() * 0.06;
        let piece_rect = rect.shrink(padding);
        draw_scalable_piece(painter, piece_rect, color, piece_type);
        return;
    }

    let (uri, bytes) = piece_svg_bytes(set, color, piece_type);
    if bytes.is_empty() {
        let padding = rect.width() * 0.06;
        let piece_rect = rect.shrink(padding);
        draw_scalable_piece(painter, piece_rect, color, piece_type);
        return;
    }

    // Shrink slightly so pieces have a balanced margin inside squares.
    let padding = rect.width() * 0.07;
    let piece_rect = rect.shrink(padding);

    // Subtle drop shadow under piece.
    let shadow_rect = piece_rect.translate(egui::vec2(0.0, 1.5));
    painter.circle_filled(
        shadow_rect.center_bottom() - egui::vec2(0.0, 4.0),
        piece_rect.width() * 0.28,
        Color32::from_rgba_unmultiplied(0, 0, 0, 35),
    );

    let img = Image::from_bytes(uri, bytes);
    img.paint_at(ui, piece_rect);
}
