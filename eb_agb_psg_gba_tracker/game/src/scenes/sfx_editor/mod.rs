pub mod channels;
pub(crate) mod common;
mod editor;
pub mod wave;

pub use common::shell::EditorShell;
pub use editor::SfxEditor;
pub use wave::WaveEditor;

use agb::input::{Button, ButtonController};

pub(crate) const DELETE_HOLD_FRAMES: u8 = 60;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ListId {
    Instruments,
    Waves,
}

pub enum InputResult {
    None,
    EditFilename,
    Exit,
    Save,
    Edited,
    EditedTop,
    EditedList { list: ListId, item: u8 },
    ListChanged { list: ListId, move_to: Option<u8> },
    EditedRow(u8),
    RowsChanged { move_to: Option<u8> },
    Regen,
}

/// -1 for L, +1 for R, 0 otherwise
fn lr_delta(button_controller: &ButtonController) -> i32 {
    if button_controller.is_just_pressed(Button::L) {
        -1
    } else if button_controller.is_just_pressed(Button::R) {
        1
    } else {
        0
    }
}

fn coarse_step(button_controller: &ButtonController, fine: i32, coarse: i32) -> i32 {
    if button_controller.is_pressed(Button::A) {
        coarse
    } else {
        fine
    }
}

#[inline]
fn add_clamped(value: u8, delta: i32, min: u8, max: u8) -> u8 {
    (value as i32 + delta).clamp(min as i32, max as i32) as u8
}
