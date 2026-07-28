#![no_std]
#![no_main]

use agb::input::{Button, ButtonController};
use eb_agb_psg_controller::{Player, Sfx, Track, include_pmus, include_psfx};

static SONG: Track = include_pmus!("assets/example_song.pmus");
static JUMP: Sfx = include_psfx!("assets/sfx/jump.psfx");
static EXPLOSION: Sfx = include_psfx!("assets/sfx/explosion.psfx");

#[agb::entry]
fn main(mut _gba: agb::Gba) -> ! {
    let vblank = agb::interrupt::VBlank::get();
    let mut input = ButtonController::new();
    let mut player = Player::new(&SONG);

    loop {
        input.update();
        if input.is_just_pressed(Button::A) {
            player.play_sfx(&JUMP);
        }
        if input.is_just_pressed(Button::B) {
            player.play_sfx(&EXPLOSION);
        }

        player.frame();
        vblank.wait_for_vblank();
    }
}
