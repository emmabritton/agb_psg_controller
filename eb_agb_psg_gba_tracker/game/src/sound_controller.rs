use agb::sound::mixer::{Mixer, SoundChannel, SoundData};
use resources::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SoundEffect {
    CursorMove,
    CursorSelect,
    CursorCancel,
}

impl SoundEffect {
    fn data(self) -> SoundData {
        match self {
            SoundEffect::CursorMove => SFX_CURSOR_MOVE,
            SoundEffect::CursorSelect => SFX_CURSOR_SELECT,
            SoundEffect::CursorCancel => SFX_CURSOR_CANCEL,
        }
    }
}

pub struct SoundController<'gba> {
    mixer: Mixer<'gba>,
}

impl<'gba> SoundController<'gba> {
    pub fn new(mixer: Mixer<'gba>) -> Self {
        Self { mixer }
    }

    pub fn frame(&mut self) {
        self.mixer.frame();
    }

    pub fn play_sfx(&mut self, effect: SoundEffect) {
        self.mixer.play_sound(SoundChannel::new(effect.data()));
    }
}
