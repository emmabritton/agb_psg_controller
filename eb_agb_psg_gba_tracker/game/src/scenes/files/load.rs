use super::{GridEvent, SlotGrid};
use crate::save_controller::{SaveController, SavePayload};
use crate::scenes::{SceneAction, SceneCtx};
use crate::sfx_doc::SfxDocument;
use crate::sound_controller::SoundEffect;
use agb::display::GraphicsFrame;
use resources::bg;

pub struct LoadScene {
    grid: SlotGrid,
}

impl LoadScene {
    pub fn new(save_controller: &SaveController) -> Self {
        Self {
            grid: SlotGrid::new(&bg::load, b"LOAD: A", save_controller),
        }
    }

    pub fn update(&mut self, ctx: &mut SceneCtx) -> Option<SceneAction> {
        let Some(save_controller) = ctx.save_controller.as_mut() else {
            return Some(SceneAction::OpenMenu);
        };
        match self
            .grid
            .update(ctx.button_controller, ctx.sound_controller, save_controller)
        {
            GridEvent::Back => Some(SceneAction::OpenMenu),
            GridEvent::Activate(slot) => match save_controller.load(slot) {
                Some((name, SavePayload::Sfx(sfx))) => {
                    ctx.sound_controller.play_sfx(SoundEffect::CursorSelect);
                    Some(SceneAction::OpenSfx(
                        SfxDocument::from_sfx(sfx),
                        None,
                        Some(name),
                    ))
                }
                _ => {
                    ctx.sound_controller.play_sfx(SoundEffect::CursorCancel);
                    None
                }
            },
            GridEvent::None => None,
        }
    }

    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.grid.show(frame);
    }
}
