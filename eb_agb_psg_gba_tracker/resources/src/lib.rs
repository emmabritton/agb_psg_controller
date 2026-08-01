#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

extern crate alloc;

use agb::sound::mixer::SoundData;
use agb::{include_aseprite, include_background_gfx, include_wav};
use gba_agb_font_renderer::include_agb_font;

#[cfg(test)]
#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    loop {}
}

include_background_gfx!(
    pub mod bg,
    "000000",
    menu => deduplicate "gfx/bg_menu.aseprite",
    menu_bad_save => deduplicate "gfx/bg_menu_bad_save.aseprite",
    sfx_menu => deduplicate "gfx/bg_sfx_menu.aseprite",
    sfx_sweep => deduplicate "gfx/bg_sfx_sweep.aseprite",
    sfx_square => deduplicate "gfx/bg_sfx_square.aseprite",
    sfx_noise => deduplicate "gfx/bg_sfx_noise.aseprite",
    sfx_wave => deduplicate "gfx/bg_sfx_wave.aseprite",
    keyboard_filename => deduplicate "gfx/bg_keyboard_filename.aseprite",
    save => deduplicate "gfx/bg_save.aseprite",
    load => deduplicate "gfx/bg_load.aseprite",
);

include_aseprite!(
    pub mod sprites,
    "gfx/spr_menu_button.aseprite",
    "gfx/spr_labels.aseprite",
    "gfx/spr_buttons.aseprite",
    "gfx/spr_arrows.aseprite",
    "gfx/spr_button_highlights.aseprite",
    "gfx/spr_keyboard.aseprite",
);

include_agb_font!(pub FONT_BLACK, "fnt/mono_3x5_idx0.aseprite", monospace = 3, widths = { '.' = 1 });
include_agb_font!(pub FONT_WHITE, "fnt/mono_3x5_idx1.aseprite", monospace = 3, widths = { '.' = 1 });

pub static SFX_CURSOR_MOVE: SoundData = include_wav!("sfx/cursor_move.wav");
pub static SFX_CURSOR_SELECT: SoundData = include_wav!("sfx/cursor_select.wav");
pub static SFX_CURSOR_CANCEL: SoundData = include_wav!("sfx/cursor_cancel.wav");
