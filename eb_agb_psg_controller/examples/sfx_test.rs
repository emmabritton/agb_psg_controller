#![no_std]
#![no_main]

use agb::input::{Button, ButtonController};
use eb_agb_psg_controller::{Player, Sfx, include_psfx};

static DRIP: Sfx = include_psfx!("assets/sfx/drip.psfx");
static STONES: Sfx = include_psfx!("assets/sfx/stones.psfx");
static WATER: Sfx = include_psfx!("assets/sfx/water_trap.psfx");
static WIND: Sfx = include_psfx!("assets/sfx/wind.psfx");

#[agb::entry]
fn main(mut _gba: agb::Gba) -> ! {
    let vblank = agb::interrupt::VBlank::get();
    let mut input = ButtonController::new();
    let mut player = Player::sfx_only();

    loop {
        input.update();
        if input.is_just_pressed(Button::A) {
            player.play_sfx(&DRIP);
        }
        if input.is_just_pressed(Button::B) {
            player.play_sfx(&STONES);
        }
        if input.is_just_pressed(Button::L) {
            player.play_sfx(&WATER);
        }
        if input.is_just_pressed(Button::R) {
            player.play_sfx(&WIND);
        }

        player.frame();
        vblank.wait_for_vblank();
    }
}
