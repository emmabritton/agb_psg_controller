use crate::save_controller::SaveController;
use crate::scenes::{SceneAction, SceneCtx, clicked};
use crate::sound_controller::SoundEffect;
use agb::display::tiled::RegularBackground;
use agb::display::{GraphicsFrame, Priority};
use agb::input::Button;
use agb_eb_ext::backgrounds::create_filled_bg;
use resources::bg;

pub struct MenuState {
    bg: RegularBackground,
    bad_save: Option<RegularBackground>,
}

impl MenuState {
    pub fn new(save_controller: &Option<SaveController>) -> Self {
        let bad_save = if save_controller.is_none() {
            Some(create_filled_bg(&bg::menu_bad_save, Priority::P0))
        } else {
            None
        };
        Self {
            bg: create_filled_bg(&bg::menu, Priority::P3),
            bad_save,
        }
    }

    pub fn update(&mut self, ctx: &mut SceneCtx) -> Option<SceneAction> {
        if clicked(ctx, Button::L, SoundEffect::CursorSelect) {
            return Some(SceneAction::OpenSfxMenu);
        }
        if self.bad_save.is_none() && clicked(ctx, Button::Start, SoundEffect::CursorSelect) {
            return Some(SceneAction::OpenLoad);
        }
        None
    }

    pub fn show(&self, frame: &mut GraphicsFrame) {
        if let Some(bg) = &self.bad_save {
            bg.show(frame);
        }
        self.bg.show(frame);
    }
}
