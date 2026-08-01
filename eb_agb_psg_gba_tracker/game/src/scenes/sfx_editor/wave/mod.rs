mod cursor;
mod layout;

use crate::UserStr;
use crate::scenes::sfx_editor::channels::{edit_instrument_with, starter_instrument};
use crate::scenes::sfx_editor::common::cursor::{NavCtx, Target};
use crate::scenes::sfx_editor::common::layout::COL_STRIDE;
use crate::scenes::sfx_editor::common::rows;
use crate::scenes::sfx_editor::common::shell::{
    CommonTarget, EditorShell, ShellState, handle_iid_input, handle_list_edit, list_edit_result,
};
use crate::scenes::sfx_editor::common::table::{
    CELL_FORMAT, Grid, RowDesc, RowInput, RowKind, for_each_visible, text_index,
};
use crate::scenes::sfx_editor::common::text::{HEX, push_num};
use crate::scenes::sfx_editor::common::tooltip::TooltipText;
use crate::scenes::sfx_editor::wave::cursor::{
    InstrField, WaveCursor, WaveField, WaveZone, new_wave_cursor,
};
use crate::scenes::sfx_editor::wave::layout::{
    DATA_CELL_WIDTH, DATA_CELL_X, DATA_X, INSTR_ORIGIN, ITEM_STRIDE, PAIR_STRIDE, TOOLTIP_IID,
    TOOLTIP_INSTR_WID, TOOLTIP_VOL, VISIBLE_ITEMS, WAVE_WID_X, item_text_y,
};
use crate::scenes::sfx_editor::{InputResult, ListId, add_clamped, coarse_step, lr_delta};
use crate::sfx_doc::SfxDocument;
use crate::sfx_template::SfxTemplate;
use crate::sound_controller::{SoundController, SoundEffect};
use agb::display::Priority;
use agb::display::object::Tag;
use agb::display::tiled::RegularBackground;
use agb::fixnum::{Vector2D, vec2};
use agb::input::{Button, ButtonController};
use agb::rng::RandomNumberGenerator;
use agb_eb_ext::backgrounds::create_filled_bg;
use agb_eb_ext::direction::Direction;
use agb_eb_ext::rng::SeedGen;
use arrayvec::ArrayString;
use eb_agb_psg_controller::{Instrument, SfxChannel, limits};
use gba_agb_font_renderer::prelude::{PrintableFont, TextRenderer};
use gba_agb_font_renderer::{TextAlign, TextFormat, TextOverflow};
use resources::{FONT_BLACK, bg, sprites};

const NEW_INSTRUMENT: Instrument = starter_instrument(SfxChannel::Wave);

// down
const TRIANGLE_TABLE: [u8; 16] = [
    0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0xFE, 0xDC, 0xBA, 0x98, 0x76, 0x54, 0x32, 0x10,
];

// up
const SAW_TABLE: [u8; 16] = [
    0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
];

// left
const SINE_TABLE: [u8; 16] = [
    0x89, 0xAC, 0xDE, 0xEF, 0xFF, 0xEE, 0xDC, 0xA9, 0x76, 0x53, 0x21, 0x10, 0x00, 0x11, 0x23, 0x56,
];

static FONT: &PrintableFont = &FONT_BLACK;

static PAIR_FORMAT: TextFormat = TextFormat {
    overflow: TextOverflow::Nothing,
    align: TextAlign::Left,
    clear: (0, 0),
};

pub struct WaveEditor {
    shell: ShellState,
    bg_ui: RegularBackground,
    cursor: WaveCursor,
}

pub fn ensure_seeded(doc: &mut SfxDocument) {
    if doc.wave_tables().is_empty() {
        doc.add_wave_table(TRIANGLE_TABLE);
    }
    if doc.instruments().is_empty() {
        doc.add_instrument(NEW_INSTRUMENT);
    }
}

impl WaveEditor {
    pub fn new(mut doc: SfxDocument, template: Option<SfxTemplate>, file: Option<UserStr>) -> Self {
        ensure_seeded(&mut doc);
        let mut shell = ShellState::new(doc, template, file);
        draw_all_instr_rows(0, &shell.doc, &mut shell.text_renderer, &mut shell.bg_text);
        draw_all_wave_rows(0, &shell.doc, &mut shell.text_renderer, &mut shell.bg_text);
        rows::draw_all_row_columns(0, &shell.doc, &mut shell.text_renderer, &mut shell.bg_text);
        let mut editor = Self {
            shell,
            bg_ui: create_filled_bg(&bg::sfx_wave, Priority::P3),
            cursor: new_wave_cursor(),
        };
        editor.refresh_tooltip(None);
        editor
    }

    fn fill_wave_table(
        &mut self,
        item: u8,
        dir: Direction,
        sound_controller: &mut SoundController,
        seed_gen: &SeedGen,
    ) {
        let samples = match dir {
            Direction::Left => SINE_TABLE,
            Direction::Right => random_samples(&mut seed_gen.create_rng()),
            Direction::Up => SAW_TABLE,
            Direction::Down => TRIANGLE_TABLE,
        };
        if samples != self.shell.doc.wave_tables()[item as usize]
            && self.shell.doc.set_wave_table(item, samples)
        {
            sound_controller.play_sfx(SoundEffect::CursorSelect);
            self.shell.mark_sfx_dirty();
            self.draw_wave_row(item);
        }
    }

    fn draw_wave_pair(&mut self, item: u8, nibble: u8) {
        let pair = (nibble >> 1) as usize;
        let Some(samples) = self.shell.doc.wave_tables().get(item as usize) else {
            return;
        };
        let byte = samples[pair];
        let at = vec2(
            DATA_X + PAIR_STRIDE * pair as i32,
            item_text_y(item, self.cursor.mid.wave_first),
        );
        self.shell
            .text_renderer
            .clear_pixel_rect(at, PAIR_STRIDE - 1, 8);
        let digits = [HEX[(byte >> 4) as usize], HEX[(byte & 0xF) as usize]];
        self.shell.text_renderer.draw_text(
            &digits,
            FONT,
            &mut self.shell.bg_text,
            at,
            &PAIR_FORMAT,
        );
    }

    fn draw_instr_row(&mut self, item: u8) {
        draw_one_instr_row(
            item,
            self.cursor.mid.instr_first,
            &self.shell.doc,
            &mut self.shell.text_renderer,
            &mut self.shell.bg_text,
        );
    }

    fn redraw_instruments(&mut self) {
        draw_all_instr_rows(
            self.cursor.mid.instr_first,
            &self.shell.doc,
            &mut self.shell.text_renderer,
            &mut self.shell.bg_text,
        );
    }

    fn draw_wave_row(&mut self, item: u8) {
        draw_one_wave_row(
            item,
            self.cursor.mid.wave_first,
            &self.shell.doc,
            &mut self.shell.text_renderer,
            &mut self.shell.bg_text,
        );
    }

    fn redraw_waves(&mut self) {
        draw_all_wave_rows(
            self.cursor.mid.wave_first,
            &self.shell.doc,
            &mut self.shell.text_renderer,
            &mut self.shell.bg_text,
        );
    }
}

impl EditorShell for WaveEditor {
    fn shell(&mut self) -> &mut ShellState {
        &mut self.shell
    }

    fn shell_ref(&self) -> &ShellState {
        &self.shell
    }

    fn bg_ui(&self) -> &RegularBackground {
        &self.bg_ui
    }

    fn common_target(&self) -> CommonTarget {
        match self.cursor.target {
            Target::Top(idx) => CommonTarget::Top(idx),
            Target::Rows { row, col } => CommonTarget::Rows { row, col },
            _ => CommonTarget::Mid,
        }
    }

    fn cursor_tag(&self) -> &'static Tag {
        self.cursor.tag()
    }

    fn cursor_pos(&self) -> Vector2D<i32> {
        self.cursor.pos()
    }

    fn cursor_tooltip(&self) -> TooltipText {
        self.cursor.tooltip()
    }

    fn tooltip_zone_row(&self) -> (u8, usize) {
        self.cursor.tooltip_zone_row()
    }

    fn rows_first(&self) -> u8 {
        self.cursor.rows_first
    }

    fn cursor_on_hold_cell(&self) -> bool {
        match self.cursor.target {
            Target::Zone(WaveZone::Instr {
                field: InstrField::Iid,
                ..
            })
            | Target::Zone(WaveZone::Wave {
                field: WaveField::Wid,
                ..
            }) => true,
            Target::Rows { row, .. } => row == rows::ROW_NUM,
            _ => false,
        }
    }

    fn nav(&mut self, direction: Direction) -> bool {
        self.cursor.next(
            direction,
            &NavCtx {
                doc: &self.shell.doc,
                template: self.shell.is_template(),
            },
        )
    }

    fn scroll_and_redraw_moved(&mut self) {
        let ((instr_moved, wave_moved), rows_moved) = self.cursor.scroll_to_cursor(&self.shell.doc);
        if instr_moved {
            self.redraw_instruments();
        }
        if wave_moved {
            self.redraw_waves();
        }
        if rows_moved {
            self.redraw_rows();
        }
    }

    fn clamp_and_scroll(&mut self) {
        self.cursor.clamp_to(&self.shell.doc);
        self.cursor.scroll_to_cursor(&self.shell.doc);
    }

    fn reset_scroll(&mut self) {
        self.cursor.mid.instr_first = 0;
        self.cursor.mid.wave_first = 0;
        self.cursor.rows_first = 0;
    }

    fn set_cursor_list_item(&mut self, list: ListId, new_item: u8) {
        match (list, &mut self.cursor.target) {
            (ListId::Instruments, Target::Zone(WaveZone::Instr { item, .. }))
            | (ListId::Waves, Target::Zone(WaveZone::Wave { item, .. })) => *item = new_item,
            _ => {}
        }
    }

    fn set_cursor_rows_col(&mut self, new_col: u8) {
        if let Target::Rows { col, .. } = &mut self.cursor.target {
            *col = new_col;
        }
    }

    fn handle_mid_input(&mut self, button_controller: &ButtonController) -> InputResult {
        match self.cursor.target {
            Target::Top(_) | Target::Rows { .. } => InputResult::None,
            Target::Zone(WaveZone::Instr {
                item,
                field: InstrField::Iid,
            }) => handle_iid_input(
                button_controller,
                &mut self.shell.doc,
                item,
                &mut self.shell.delete_hold,
                NEW_INSTRUMENT,
            ),
            Target::Zone(WaveZone::Instr { item, field }) => {
                let delta = lr_delta(button_controller);
                if delta != 0 && edit_instrument(&mut self.shell.doc, item, field, delta) {
                    InputResult::EditedList {
                        list: ListId::Instruments,
                        item,
                    }
                } else {
                    InputResult::None
                }
            }
            Target::Zone(WaveZone::Wave {
                item,
                field: WaveField::Wid,
            }) => handle_wave_wid_input(
                button_controller,
                &mut self.shell.doc,
                item,
                &mut self.shell.delete_hold,
            ),
            Target::Zone(WaveZone::Wave {
                item,
                field: WaveField::Nibble(nibble),
            }) => {
                if edit_nibble(button_controller, &mut self.shell.doc, item, nibble) {
                    self.draw_wave_pair(item, nibble);
                    InputResult::Edited
                } else {
                    InputResult::None
                }
            }
        }
    }

    fn draw_list_item(&mut self, list: ListId, item: u8) {
        match list {
            ListId::Instruments => self.draw_instr_row(item),
            ListId::Waves => self.draw_wave_row(item),
        }
    }

    fn redraw_after_list_change(&mut self, list: ListId) {
        match list {
            ListId::Instruments => {
                self.redraw_instruments();
                self.redraw_rows();
            }
            ListId::Waves => {
                self.redraw_waves();
                self.redraw_instruments();
            }
        }
    }

    fn redraw_all(&mut self) {
        self.redraw_instruments();
        self.redraw_waves();
        self.redraw_rows();
    }

    fn pre_nav(
        &mut self,
        button_controller: &ButtonController,
        direction: Direction,
        sound_controller: &mut SoundController,
        seed_gen: &SeedGen,
    ) -> bool {
        if button_controller.is_pressed(Button::A)
            && let Target::Zone(WaveZone::Wave {
                item,
                field: WaveField::Wid,
            }) = self.cursor.target
        {
            self.fill_wave_table(item, direction, sound_controller, seed_gen);
            return true;
        }
        false
    }

    fn seed_doc(doc: &mut SfxDocument) {
        ensure_seeded(doc);
    }
}

fn edit_instrument(doc: &mut SfxDocument, item: u8, field: InstrField, delta: i32) -> bool {
    edit_instrument_with(doc, item, |instrument| {
        let Instrument::Wave { wave_table, volume } = instrument else {
            return;
        };
        match field {
            InstrField::Iid => unreachable!("routed to the shared instrument ID handling"),
            InstrField::Wid => *wave_table = add_clamped(*wave_table, delta, 0, u8::MAX),
            InstrField::Vol => {
                *volume = add_clamped(*volume, delta, 0, limits::volume_max(SfxChannel::Wave))
            }
        }
    })
}

fn handle_wave_wid_input(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    item: u8,
    delete_hold: &mut u8,
) -> InputResult {
    let edit = handle_list_edit(
        button_controller,
        doc,
        delete_hold,
        Button::R,
        // Every wave instrument needs a table to reference, so the last one
        // can't be deleted; the document refuses referenced tables itself.
        |doc| doc.wave_tables().len() > 1 && doc.remove_wave_table(item),
        // A copy of the current table, as a starting point for variations.
        |doc| {
            let samples = doc.wave_tables()[item as usize];
            doc.add_wave_table(samples)
        },
    );
    list_edit_result(edit, |move_to| InputResult::ListChanged {
        list: ListId::Waves,
        move_to,
    })
}

/// L/R steps one hex digit, by 6 while A is held; clamps at 0 and F
fn edit_nibble(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    item: u8,
    nibble: u8,
) -> bool {
    let delta = lr_delta(button_controller);
    if delta == 0 {
        return false;
    }
    let step = coarse_step(button_controller, 1, 6);
    let samples = doc.wave_tables()[item as usize];
    let stepped = stepped_samples(samples, nibble, delta * step);
    if stepped == samples {
        return false;
    }
    doc.set_wave_table(item, stepped)
}

fn stepped_samples(mut samples: [u8; 16], nibble: u8, delta: i32) -> [u8; 16] {
    let byte = samples[(nibble >> 1) as usize];
    let current = if nibble & 1 == 0 {
        byte >> 4
    } else {
        byte & 0xF
    };
    let target = (current as i32 + delta).clamp(0, 15) as u8;
    samples[(nibble >> 1) as usize] = if nibble & 1 == 0 {
        (target << 4) | (byte & 0xF)
    } else {
        (byte & 0xF0) | target
    };
    samples
}

/// Random nibbles — the metallic, noisy end of the wave palette
fn random_samples(rng: &mut RandomNumberGenerator) -> [u8; 16] {
    let mut samples = [0u8; 16];
    for byte in &mut samples {
        *byte = (rng.next_i32() & 0xFF) as u8;
    }
    samples
}

fn text_wid(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    if let Instrument::Wave { wave_table, .. } = instrument {
        push_num(out, *wave_table as i32);
    }
}

/// The stored 0-4 volume is shown as its percentage (0/25/50/75/100)
fn text_vol(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    if let Instrument::Wave { volume, .. } = instrument {
        push_num(out, *volume as i32 * 25);
    }
}

/// The instruments table's three fields, spread horizontally within each item
static INSTR_ROWS: [RowDesc<Instrument, InstrField>; 3] = [
    RowDesc {
        id: InstrField::Iid,
        content_off: 0,
        cursor_off: -1,
        tag: &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
        tooltip: TOOLTIP_IID,
        kind: RowKind::Text(text_index),
        input: RowInput::InstrumentId,
    },
    RowDesc {
        id: InstrField::Wid,
        content_off: COL_STRIDE,
        cursor_off: COL_STRIDE - 1,
        tag: &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
        tooltip: TOOLTIP_INSTR_WID,
        kind: RowKind::Text(text_wid),
        input: RowInput::Edit,
    },
    RowDesc {
        id: InstrField::Vol,
        content_off: COL_STRIDE * 2,
        cursor_off: COL_STRIDE * 2 - 1,
        tag: &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
        tooltip: TOOLTIP_VOL,
        kind: RowKind::Text(text_vol),
        input: RowInput::Edit,
    },
];

/// The instruments list as a vertical grid: items step in y, fields in x
fn instr_grid(first_visible: u8) -> Grid<Instrument, InstrField> {
    Grid {
        origin: INSTR_ORIGIN,
        axis: vec2(0, ITEM_STRIDE),
        cross: vec2(1, 0),
        rows: &INSTR_ROWS,
        clear_size: vec2(COL_STRIDE * 3 - 1, 8),
        first_visible,
    }
}

fn draw_one_instr_row(
    item: u8,
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    instr_grid(first_visible).draw_slot(
        item,
        doc.instruments().get(item as usize),
        |instrument| matches!(instrument, Instrument::Wave { .. }),
        text_renderer,
        bg_text,
    );
}

fn draw_one_wave_row(
    item: u8,
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    let y = item_text_y(item, first_visible);
    match doc.wave_tables().get(item as usize) {
        Some(samples) => {
            let mut buf = ArrayString::<8>::new();
            push_num(&mut buf, item as i32);
            text_renderer.draw_text(
                buf.as_bytes(),
                FONT,
                bg_text,
                vec2(WAVE_WID_X, y),
                &CELL_FORMAT,
            );
            text_renderer.clear_pixel_rect(vec2(DATA_CELL_X, y), DATA_CELL_WIDTH, 8);
            for (pair, byte) in samples.iter().enumerate() {
                let digits = [HEX[(byte >> 4) as usize], HEX[(byte & 0xF) as usize]];
                text_renderer.draw_text(
                    &digits,
                    FONT,
                    bg_text,
                    vec2(DATA_X + PAIR_STRIDE * pair as i32, y),
                    &PAIR_FORMAT,
                );
            }
        }
        None => {
            text_renderer.clear_pixel_rect(vec2(WAVE_WID_X, y), COL_STRIDE + DATA_CELL_WIDTH, 8);
        }
    }
}

fn draw_all_instr_rows(
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    for_each_visible(first_visible, VISIBLE_ITEMS, |item| {
        draw_one_instr_row(item, first_visible, doc, text_renderer, bg_text);
    });
}

fn draw_all_wave_rows(
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    for_each_visible(first_visible, VISIBLE_ITEMS, |item| {
        draw_one_wave_row(item, first_visible, doc, text_renderer, bg_text);
    });
}
