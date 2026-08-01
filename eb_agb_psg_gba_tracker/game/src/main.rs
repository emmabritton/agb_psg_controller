#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

mod save_controller;
mod scenes;
mod sfx_doc;
mod sfx_template;
mod sound_controller;

extern crate alloc;

use crate::save_controller::SaveController;
use crate::scenes::SceneHostState;
use crate::sound_controller::SoundController;
use agb::display::Rgb15;
use agb::eprintln;
use agb::input::ButtonController;
use agb::sound::mixer::Frequency;
use alloc::vec::Vec;
use eb_agb_psg_controller::Player;

pub type GbaStr = &'static [u8];
pub type UserStr = Vec<u8>;
const SAVE_MAGIC: [u8; 32] = *b"AGB PSG Tracker - EB - SaveVer 1";

const MAX_FILENAME_LEN: u8 = 20;

const SAVE_SLOTS: usize = 16;

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    let mut save_controller = match gba.save.init_sram(SAVE_SLOTS, SAVE_MAGIC) {
        Ok(save_manager) => Some(SaveController::new(save_manager)),
        Err(e) => {
            eprintln!("save init: {:?}", e);
            None
        }
    };
    let mut button_controller = ButtonController::new();
    let mut gfx = gba.graphics.get();
    let mut scene = SceneHostState::new(&save_controller);
    let mut player = Player::new();
    let mixer = gba.mixer.mixer(Frequency::Hz10512);
    let mut sound_controller = SoundController::new(mixer);

    gfx.set_background_palettes(resources::bg::PALETTES);
    gfx.set_background_palette_colour(15, 15, Rgb15::BLACK);
    gfx.set_background_palette_colour(15, 14, Rgb15::WHITE);

    loop {
        let mut frame = gfx.frame();
        button_controller.update();

        scene.update(
            &button_controller,
            &mut sound_controller,
            &mut save_controller,
            &mut player,
        );
        scene.show(&mut frame);

        player.frame();
        sound_controller.frame();
        frame.commit();
    }
}
