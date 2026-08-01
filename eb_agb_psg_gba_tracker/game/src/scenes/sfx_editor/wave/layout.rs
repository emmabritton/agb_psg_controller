use crate::scenes::sfx_editor::common::tooltip::{BUTTONS_NUM, TooltipText};
use agb::fixnum::{Vector2D, vec2};

pub const INSTR_ORIGIN: Vector2D<i32> = vec2(3, 45);
pub const WAVE_WID_X: i32 = 61;
pub const DATA_CELL_X: i32 = 76;
pub const DATA_CELL_WIDTH: i32 = 150;
pub const DATA_X: i32 = 80;
pub const PAIR_STRIDE: i32 = 9;
const NIBBLE_OFFSET: i32 = 4;
pub const ITEM_STRIDE: i32 = 8;
pub const VISIBLE_ITEMS: u8 = 6;

pub fn nibble_x(nibble: u8) -> i32 {
    DATA_X + PAIR_STRIDE * (nibble >> 1) as i32 + NIBBLE_OFFSET * (nibble & 1) as i32
}

pub fn item_text_y(item: u8, first_visible: u8) -> i32 {
    INSTR_ORIGIN.y + ITEM_STRIDE * item.saturating_sub(first_visible) as i32
}

pub const BUTTONS_ITEM: &[u8] = b"NEW: R\nDELETE: HOLD B";

pub const TOOLTIP_IID: TooltipText = TooltipText {
    help: b"INSTRUMENT ID",
    buttons: BUTTONS_ITEM,
};
pub const TOOLTIP_INSTR_WID: TooltipText = TooltipText {
    help: b"WAVE TABLE ID\nWHICH WAVE TO PLAY",
    buttons: BUTTONS_NUM,
};
pub const TOOLTIP_VOL: TooltipText = TooltipText {
    help: b"VOLUME\nPERCENT OF FULL",
    buttons: BUTTONS_NUM,
};
pub const TOOLTIP_WAVE_WID: TooltipText = TooltipText {
    help: b"WAVE TABLE ID\nFILL: HOLD A AND ARROW",
    buttons: BUTTONS_ITEM,
};
pub const TOOLTIP_DATA: TooltipText = TooltipText {
    help: b"WAVE DATA\n32 SAMPLES 0 TO F",
    buttons: b"DEC: L | INC: R\nBY 6: HOLD A",
};
