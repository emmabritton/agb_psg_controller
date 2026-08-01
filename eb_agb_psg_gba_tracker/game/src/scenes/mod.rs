pub mod files;
mod menu;
mod sfx_editor;
mod sfx_menu;

use crate::UserStr;
use crate::save_controller::SaveController;
use crate::scenes::files::{LoadScene, SaveScene};
use crate::scenes::menu::MenuState;
use crate::scenes::sfx_editor::channels::noise::NoiseChannel;
use crate::scenes::sfx_editor::channels::square::SquareChannel;
use crate::scenes::sfx_editor::channels::sweep::SweepChannel;
use crate::scenes::sfx_editor::{EditorShell, SfxEditor, WaveEditor};
use crate::scenes::sfx_menu::SfxMenuScene;
use crate::sfx_doc::SfxDocument;
use crate::sfx_template::SfxTemplate;
use crate::sound_controller::{SoundController, SoundEffect};
use agb::display::GraphicsFrame;
use agb::input::{Button, ButtonController};
use agb_eb_ext::rng::SeedGen;
use eb_agb_psg_controller::{Player, SfxChannel};

pub struct SceneCtx<'a, 'gba, 'track> {
    pub button_controller: &'a ButtonController,
    pub sound_controller: &'a mut SoundController<'gba>,
    pub player: &'a mut Player<'track>,
    pub seed_gen: &'a SeedGen,
    pub save_controller: &'a mut Option<SaveController>,
}

pub fn clicked(ctx: &mut SceneCtx, button: Button, effect: SoundEffect) -> bool {
    if ctx.button_controller.is_just_pressed(button) {
        ctx.sound_controller.play_sfx(effect);
        true
    } else {
        false
    }
}

#[allow(clippy::large_enum_variant)]
enum SceneState {
    Menu(MenuState),
    SfxSweep(SfxEditor<SweepChannel>),
    SfxSquare(SfxEditor<SquareChannel>),
    SfxNoise(SfxEditor<NoiseChannel>),
    SfxWave(WaveEditor),
    SfxMenu(SfxMenuScene),
    Save(SaveScene),
    Load(LoadScene),
    Transition,
}

impl SceneState {
    #[inline]
    fn update(&mut self, ctx: &mut SceneCtx) -> Option<SceneAction> {
        match self {
            SceneState::Menu(s) => s.update(ctx),
            SceneState::SfxSweep(s) => s.update(ctx),
            SceneState::SfxSquare(s) => s.update(ctx),
            SceneState::SfxNoise(s) => s.update(ctx),
            SceneState::SfxWave(s) => s.update(ctx),
            SceneState::SfxMenu(s) => s.update(ctx),
            SceneState::Save(s) => s.update(ctx),
            SceneState::Load(s) => s.update(ctx),
            SceneState::Transition => None,
        }
    }

    #[inline]
    fn show(&self, frame: &mut GraphicsFrame) {
        match self {
            SceneState::Menu(s) => s.show(frame),
            SceneState::SfxSweep(s) => s.show(frame),
            SceneState::SfxSquare(s) => s.show(frame),
            SceneState::SfxNoise(s) => s.show(frame),
            SceneState::SfxWave(s) => s.show(frame),
            SceneState::SfxMenu(s) => s.show(frame),
            SceneState::Save(s) => s.show(frame),
            SceneState::Load(s) => s.show(frame),
            SceneState::Transition => {}
        }
    }
}

pub struct SceneHostState {
    scene: SceneState,
    rng_seed: SeedGen,
    pending_action: Option<SceneAction>,
}

impl SceneHostState {
    pub fn new(save_controller: &Option<SaveController>) -> Self {
        Self {
            scene: SceneState::Menu(MenuState::new(save_controller)),
            rng_seed: Default::default(),
            pending_action: None,
        }
    }

    #[inline(always)]
    pub fn update(
        &mut self,
        button_controller: &ButtonController,
        sound_controller: &mut SoundController,
        save_controller: &mut Option<SaveController>,
        player: &mut Player,
    ) {
        self.rng_seed.update(button_controller);
        if let Some(action) = self.pending_action.take() {
            self.scene = match action {
                SceneAction::OpenMenu => SceneState::Menu(MenuState::new(save_controller)),
                SceneAction::OpenSfxMenu => SceneState::SfxMenu(SfxMenuScene::new()),
                SceneAction::OpenSfx(doc, template, file) => open_sfx(doc, template, file),
                SceneAction::OpenSave {
                    doc,
                    template,
                    filename,
                } => match save_controller {
                    Some(save_controller) => {
                        SceneState::Save(SaveScene::new(doc, template, filename, save_controller))
                    }
                    None => open_sfx(doc, template, Some(filename)),
                },
                SceneAction::OpenLoad => match save_controller {
                    Some(save_controller) => SceneState::Load(LoadScene::new(save_controller)),
                    None => SceneState::Menu(MenuState::new(&None)),
                },
            };
        } else {
            let mut ctx = SceneCtx {
                button_controller,
                sound_controller,
                player,
                seed_gen: &self.rng_seed,
                save_controller,
            };
            self.pending_action = self.scene.update(&mut ctx);
            if self.pending_action.is_some() {
                self.scene = SceneState::Transition;
            }
        }
    }

    #[inline(always)]
    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.scene.show(frame);
    }
}

fn open_sfx(doc: SfxDocument, template: Option<SfxTemplate>, file: Option<UserStr>) -> SceneState {
    match doc.channel() {
        SfxChannel::SquareSweep => SceneState::SfxSweep(SfxEditor::new(doc, template, file)),
        SfxChannel::Square => SceneState::SfxSquare(SfxEditor::new(doc, template, file)),
        SfxChannel::Noise => SceneState::SfxNoise(SfxEditor::new(doc, template, file)),
        SfxChannel::Wave => SceneState::SfxWave(WaveEditor::new(doc, template, file)),
    }
}

#[derive(Debug)]
pub enum SceneAction {
    OpenMenu,
    OpenSfxMenu,
    OpenSfx(SfxDocument, Option<SfxTemplate>, Option<UserStr>),
    OpenSave {
        doc: SfxDocument,
        template: Option<SfxTemplate>,
        filename: UserStr,
    },
    OpenLoad,
}
