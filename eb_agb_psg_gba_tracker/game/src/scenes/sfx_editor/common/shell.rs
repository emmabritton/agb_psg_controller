
use crate::scenes::sfx_editor::common::keyboard::{Keyboard, KeyboardResult};
use crate::scenes::sfx_editor::common::rows;
use crate::scenes::sfx_editor::common::tooltip::{TooltipText, draw_tooltip};
use crate::scenes::sfx_editor::common::top_bar;
use crate::scenes::sfx_editor::common::top_bar::TOP_FIELDS;
use crate::scenes::sfx_editor::{DELETE_HOLD_FRAMES, InputResult, ListId};
use crate::scenes::{SceneAction, SceneCtx};
use crate::sfx_doc::SfxDocument;
use crate::sfx_template::SfxTemplate;
use crate::sound_controller::{SoundController, SoundEffect};
use crate::{MAX_FILENAME_LEN, UserStr};
use agb::display::GraphicsFrame;
use agb::display::object::Tag;
use agb::display::tiled::RegularBackground;
use agb::fixnum::Vector2D;
use agb::input::{Button, ButtonController};
use agb_eb_ext::backgrounds::create_text_bg;
use agb_eb_ext::direction::Direction;
use agb_eb_ext::gfx::{ShowAllTag, ShowTag};
use agb_eb_ext::rng::SeedGen;
use alloc::rc::Rc;
use alloc::vec;
use eb_agb_psg_controller::{Instrument, Player, Sfx};
use gba_agb_font_renderer::prelude::TextRenderer;
use resources::{bg, sprites};

pub enum DocSource {
    Template(SfxTemplate),
    File,
    Blank,
}

pub struct ShellState {
    pub doc: SfxDocument,
    pub source: DocSource,
    sfx: Option<Rc<Sfx>>,
    pub text_renderer: TextRenderer,
    pub bg_text: RegularBackground,
    pub keyboard: Option<Keyboard>,
    tooltip_key: Option<(u8, usize, u8)>,
    pub delete_hold: u8,
    pub filename: UserStr,
}

impl ShellState {
    pub fn new(doc: SfxDocument, template: Option<SfxTemplate>, file: Option<UserStr>) -> Self {
        let (filename, source) = match (template, file) {
            (Some(template), file) => (
                file.unwrap_or_else(|| template.name().to_vec()),
                DocSource::Template(template),
            ),
            (None, Some(file)) => (file, DocSource::File),
            (None, None) => (vec![], DocSource::Blank),
        };
        let mut text_renderer = TextRenderer::default();
        let mut bg_text = create_text_bg();
        top_bar::draw_top_text(&filename, &doc, &mut text_renderer, &mut bg_text);
        Self {
            doc,
            source,
            sfx: None,
            text_renderer,
            bg_text,
            tooltip_key: None,
            delete_hold: 0,
            keyboard: None,
            filename,
        }
    }

    pub fn is_template(&self) -> bool {
        matches!(self.source, DocSource::Template(_))
    }

    pub fn playable_sfx(&mut self) -> Rc<Sfx> {
        self.sfx
            .get_or_insert_with(|| Rc::new(self.doc.build_sfx()))
            .clone()
    }

    pub fn mark_sfx_dirty(&mut self) {
        self.sfx = None;
    }

    pub fn redraw_top(&mut self) {
        top_bar::draw_top_text(
            &self.filename,
            &self.doc,
            &mut self.text_renderer,
            &mut self.bg_text,
        );
    }
}

pub enum CommonTarget {
    Top(usize),
    Rows { row: usize, col: u8 },
    Mid,
}

pub trait EditorShell {
    fn shell(&mut self) -> &mut ShellState;
    fn shell_ref(&self) -> &ShellState;

    fn bg_ui(&self) -> &RegularBackground;
    fn common_target(&self) -> CommonTarget;
    fn cursor_tag(&self) -> &'static Tag;
    fn cursor_pos(&self) -> Vector2D<i32>;
    fn cursor_tooltip(&self) -> TooltipText;
    fn tooltip_zone_row(&self) -> (u8, usize);
    fn rows_first(&self) -> u8;

    fn cursor_on_hold_cell(&self) -> bool;

    fn nav(&mut self, direction: Direction) -> bool;
    fn scroll_and_redraw_moved(&mut self);
    fn clamp_and_scroll(&mut self);
    fn reset_scroll(&mut self);
    fn set_cursor_list_item(&mut self, list: ListId, item: u8);
    fn set_cursor_rows_col(&mut self, col: u8);

    fn handle_mid_input(&mut self, button_controller: &ButtonController) -> InputResult;

    fn draw_list_item(&mut self, list: ListId, item: u8);
    fn redraw_after_list_change(&mut self, list: ListId);
    fn extra_sprites(&self, _frame: &mut GraphicsFrame) {}
    fn redraw_all(&mut self);

    fn pre_nav(
        &mut self,
        _button_controller: &ButtonController,
        _direction: Direction,
        _sound_controller: &mut SoundController,
        _seed_gen: &SeedGen,
    ) -> bool {
        false
    }

    fn seed_doc(_doc: &mut SfxDocument) {}

    fn show(&self, frame: &mut GraphicsFrame) {
        self.bg_ui().show(frame);
        self.shell_ref().bg_text.show(frame);
        if let Some(keyboard) = &self.shell_ref().keyboard {
            keyboard.show(frame);
            return;
        }
        self.extra_sprites(frame);
        self.cursor_tag().show_all(self.cursor_pos(), frame);
        if self.shell_ref().is_template() {
            sprites::BUTTON_REGEN_DEFAULT.show(0, top_bar::regen_pos(), frame);
        }
    }

    fn update(&mut self, ctx: &mut SceneCtx) -> Option<SceneAction>
    where
        Self: Sized,
    {
        let shell = self.shell();
        if let Some(keyboard) = &mut shell.keyboard {
            let Some(result) = keyboard.update(
                ctx.button_controller,
                &mut shell.text_renderer,
                &mut shell.bg_text,
            ) else {
                return None;
            };
            if let KeyboardResult::NewValue(filename) = result {
                shell.filename = filename;
            }
            shell.keyboard = None;
            shell.text_renderer.reset(false);
            shell.tooltip_key = None;
            shell.redraw_top();
            self.redraw_all();
            self.refresh_tooltip(None);
            return None;
        }
        run_frame(
            self,
            ctx.button_controller,
            ctx.sound_controller,
            ctx.player,
            ctx.seed_gen,
        )
    }

    fn tooltip_key_now(&self) -> (u8, usize, u8) {
        let (zone, row) = self.tooltip_zone_row();
        let effect = match self.common_target() {
            CommonTarget::Rows { row, col } if row >= rows::ROW_EFFECT => {
                match self.shell_ref().doc.rows().get(col as usize) {
                    Some(slot) => rows::cycle_index(&slot.effect) as u8,
                    None => 0,
                }
            }
            _ => 0,
        };
        (zone, row, effect)
    }

    fn current_tooltip(&self) -> TooltipText {
        match self.common_target() {
            CommonTarget::Rows { row, col } if row >= rows::ROW_EFFECT => {
                rows::tooltip_for(&self.shell_ref().doc, col, row)
            }
            _ => self.cursor_tooltip(),
        }
    }

    fn handle_input(&mut self, button_controller: &ButtonController) -> InputResult {
        match self.common_target() {
            CommonTarget::Top(idx) => {
                let shell = self.shell();
                top_bar::handle_top_input(button_controller, &mut shell.doc, TOP_FIELDS[idx].id)
            }
            CommonTarget::Rows { row, col } => {
                let shell = self.shell();
                rows::handle_cell_input(
                    button_controller,
                    &mut shell.doc,
                    col,
                    row,
                    &mut shell.delete_hold,
                )
            }
            CommonTarget::Mid => self.handle_mid_input(button_controller),
        }
    }

    fn draw_row_column(&mut self, col: u8) {
        let first = self.rows_first();
        let shell = self.shell();
        rows::draw_row_column(
            col,
            first,
            &shell.doc,
            &mut shell.text_renderer,
            &mut shell.bg_text,
        );
    }

    fn redraw_rows(&mut self) {
        let first = self.rows_first();
        let shell = self.shell();
        rows::draw_all_row_columns(
            first,
            &shell.doc,
            &mut shell.text_renderer,
            &mut shell.bg_text,
        );
    }

    fn refresh_tooltip(&mut self, sound_controller: Option<&mut SoundController>) {
        let new_key = self.tooltip_key_now();
        if self.shell_ref().tooltip_key != Some(new_key) {
            if let Some(sound_controller) = sound_controller {
                sound_controller.play_sfx(SoundEffect::CursorMove);
            }
            let tooltip = self.current_tooltip();
            let shell = self.shell();
            shell.tooltip_key = Some(new_key);
            draw_tooltip(tooltip, &mut shell.text_renderer, &mut shell.bg_text);
        }
    }
}

pub fn run_frame<E: EditorShell>(
    editor: &mut E,
    button_controller: &ButtonController,
    sound_controller: &mut SoundController,
    player: &mut Player,
    seed_gen: &SeedGen,
) -> Option<SceneAction> {
    if !button_controller.is_pressed(Button::B)
        || !editor.cursor_on_hold_cell()
        || !player.is_finished()
    {
        editor.shell().delete_hold = 0;
    }
    if !player.is_finished() {
        if button_controller.is_just_pressed(Button::Start) {
            player.stop();
        }
        return None;
    }
    if button_controller.is_just_pressed(Button::Start) {
        let sfx = editor.shell().playable_sfx();
        player.play_sfx_shared(sfx);
        return None;
    }
    if let Some(direction) = Direction::from_recent_input(button_controller) {
        if !editor.pre_nav(button_controller, direction, sound_controller, seed_gen)
            && editor.nav(direction)
        {
            editor.shell().delete_hold = 0;
            editor.scroll_and_redraw_moved();
            editor.refresh_tooltip(Some(sound_controller));
        }
        return None;
    }
    let rows_gen = editor.shell_ref().doc.rows_generation();
    let result = editor.handle_input(button_controller);
    if !matches!(
        result,
        InputResult::None | InputResult::Regen | InputResult::Save
    ) {
        editor.shell().mark_sfx_dirty();
    }
    match result {
        InputResult::None => {}
        InputResult::Save => {
            let shell = editor.shell();
            let template = if let DocSource::Template(template) = shell.source {
                Some(template)
            } else {
                None
            };
            return Some(SceneAction::OpenSave {
                doc: core::mem::replace(&mut shell.doc, SfxDocument::new()),
                template,
                filename: shell.filename.clone(),
            });
        }
        InputResult::EditFilename => {
            let filename = editor.shell_ref().filename.clone();
            editor.shell().text_renderer.reset(false);
            editor.shell().keyboard = Some(Keyboard::new(
                &bg::keyboard_filename,
                filename,
                MAX_FILENAME_LEN,
            ))
        }
        InputResult::Exit => return Some(SceneAction::OpenSfxMenu),
        InputResult::Edited => {
            sound_controller.play_sfx(SoundEffect::CursorSelect);
        }
        InputResult::EditedTop => {
            sound_controller.play_sfx(SoundEffect::CursorSelect);
            editor.shell().redraw_top();
        }
        InputResult::EditedList { list, item } => {
            sound_controller.play_sfx(SoundEffect::CursorSelect);
            editor.draw_list_item(list, item);
            if editor.shell_ref().doc.rows_generation() != rows_gen {
                editor.redraw_rows();
            }
        }
        InputResult::ListChanged { list, move_to } => {
            sound_controller.play_sfx(SoundEffect::CursorSelect);
            if let Some(item) = move_to {
                editor.set_cursor_list_item(list, item);
            }
            editor.clamp_and_scroll();
            editor.redraw_after_list_change(list);
        }
        InputResult::EditedRow(col) => {
            sound_controller.play_sfx(SoundEffect::CursorSelect);
            editor.draw_row_column(col);
            editor.refresh_tooltip(None);
        }
        InputResult::RowsChanged { move_to } => {
            sound_controller.play_sfx(SoundEffect::CursorSelect);
            if let Some(col) = move_to {
                editor.set_cursor_rows_col(col);
            }
            editor.clamp_and_scroll();
            editor.redraw_rows();
            editor.refresh_tooltip(None);
        }
        InputResult::Regen => {
            // No click sfx, it would interfere with playing the new sfx
            if let DocSource::Template(template) = editor.shell_ref().source {
                let channel = editor.shell_ref().doc.channel();
                let mut doc = template.to_doc(&mut seed_gen.create_rng(), Some(channel));
                E::seed_doc(&mut doc);
                if doc.channel() != channel {
                    let filename = editor.shell_ref().filename.clone();
                    return Some(SceneAction::OpenSfx(doc, Some(template), Some(filename)));
                }
                let shell = editor.shell();
                shell.doc = doc;
                shell.mark_sfx_dirty();
                shell.text_renderer.reset(false);
                shell.tooltip_key = None;
                shell.redraw_top();
                editor.reset_scroll();
                editor.clamp_and_scroll();
                editor.redraw_all();
                editor.refresh_tooltip(None);
                let sfx = editor.shell().playable_sfx();
                player.play_sfx_shared(sfx);
            }
        }
    }
    None
}

pub enum ListEdit {
    Unchanged,
    Removed,
    Added(u8),
}

pub fn list_edit_result(
    edit: ListEdit,
    changed: impl FnOnce(Option<u8>) -> InputResult,
) -> InputResult {
    match edit {
        ListEdit::Removed => changed(None),
        ListEdit::Added(index) => changed(Some(index)),
        ListEdit::Unchanged => InputResult::None,
    }
}

pub fn handle_hold_action(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    delete_hold: &mut u8,
    complete: impl FnOnce(&mut SfxDocument) -> InputResult,
) -> InputResult {
    if button_controller.is_pressed(Button::B) {
        *delete_hold = delete_hold.saturating_add(1);
        if *delete_hold >= DELETE_HOLD_FRAMES {
            let result = complete(doc);
            if !matches!(result, InputResult::None) {
                *delete_hold = 0;
            }
            return result;
        }
    }
    InputResult::None
}

pub fn handle_list_edit(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    delete_hold: &mut u8,
    add_button: Button,
    remove: impl FnOnce(&mut SfxDocument) -> bool,
    add: impl FnOnce(&mut SfxDocument) -> Option<u8>,
) -> ListEdit {
    if button_controller.is_pressed(Button::B) {
        *delete_hold = delete_hold.saturating_add(1);
        if *delete_hold >= DELETE_HOLD_FRAMES && remove(doc) {
            *delete_hold = 0;
            return ListEdit::Removed;
        }
        return ListEdit::Unchanged;
    }
    if button_controller.is_just_pressed(add_button)
        && let Some(index) = add(doc)
    {
        return ListEdit::Added(index);
    }
    ListEdit::Unchanged
}

pub fn handle_iid_input(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    col: u8,
    delete_hold: &mut u8,
    new_instrument: Instrument,
) -> InputResult {
    let edit = handle_list_edit(
        button_controller,
        doc,
        delete_hold,
        Button::R,
        |doc| doc.instruments().len() > 1 && doc.remove_instrument(col),
        |doc| doc.add_instrument(new_instrument),
    );
    list_edit_result(edit, |move_to| InputResult::ListChanged {
        list: ListId::Instruments,
        move_to,
    })
}
