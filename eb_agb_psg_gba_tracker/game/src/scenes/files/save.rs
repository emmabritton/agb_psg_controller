use super::{GridEvent, SlotGrid};
use crate::UserStr;
use crate::save_controller::SaveController;
use crate::scenes::{SceneAction, SceneCtx};
use crate::sfx_doc::SfxDocument;
use crate::sfx_template::SfxTemplate;
use crate::sound_controller::SoundEffect;
use agb::display::GraphicsFrame;
use resources::bg;

pub struct SaveScene {
    grid: SlotGrid,
    doc: SfxDocument,
    template: Option<SfxTemplate>,
    filename: UserStr,
}

impl SaveScene {
    pub fn new(
        doc: SfxDocument,
        template: Option<SfxTemplate>,
        filename: UserStr,
        save_controller: &SaveController,
    ) -> Self {
        Self {
            grid: SlotGrid::new(&bg::save, b"SAVE: A", save_controller),
            doc,
            template,
            filename,
        }
    }

    fn back_to_editor(&mut self) -> SceneAction {
        let doc = core::mem::replace(&mut self.doc, SfxDocument::new());
        SceneAction::OpenSfx(doc, self.template, Some(self.filename.clone()))
    }

    pub fn update(&mut self, ctx: &mut SceneCtx) -> Option<SceneAction> {
        let Some(save_controller) = ctx.save_controller.as_mut() else {
            return Some(self.back_to_editor());
        };
        match self
            .grid
            .update(ctx.button_controller, ctx.sound_controller, save_controller)
        {
            GridEvent::Back => Some(self.back_to_editor()),
            GridEvent::Activate(slot) => {
                if self.filename.is_empty() {
                    self.filename = b"UNNAMED".to_vec();
                }
                let sfx = self.doc.build_sfx();
                if save_controller.save_sfx(slot, &self.filename, &sfx) {
                    ctx.sound_controller.play_sfx(SoundEffect::CursorSelect);
                    self.grid.redraw_slot(slot, save_controller);
                } else {
                    ctx.sound_controller.play_sfx(SoundEffect::CursorCancel);
                }
                None
            }
            GridEvent::None => None,
        }
    }

    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.grid.show(frame);
    }
}
