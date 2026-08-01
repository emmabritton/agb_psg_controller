use crate::scenes::sfx_editor::InputResult;
use crate::scenes::sfx_editor::common::layout::{POS_FRAMES, POS_NAME, POS_TICKS};
use crate::scenes::sfx_editor::common::text::{push_fixed, push_num};
use crate::scenes::sfx_editor::common::tooltip::{BUTTONS_NUM, TooltipText};
use crate::scenes::sfx_editor::lr_delta;
use crate::sfx_doc::{FRAMES_PER_TICK_UI_MAX, SfxDocument};
use agb::display::object::Tag;
use agb::display::tiled::RegularBackground;
use agb::fixnum::{Vector2D, vec2};
use agb::input::{Button, ButtonController};
use arrayvec::ArrayString;
use gba_agb_font_renderer::prelude::{PrintableFont, TextRenderer};
use gba_agb_font_renderer::{TextAlign, TextFormat, TextOverflow};
use resources::{FONT_BLACK, sprites};

static FONT: &PrintableFont = &FONT_BLACK;

static FORMAT_NAME: TextFormat = TextFormat {
    overflow: TextOverflow::Cutoff(82),
    align: TextAlign::Left,
    clear: (83, 8),
};
static FORMAT_NUM: TextFormat = TextFormat {
    overflow: TextOverflow::Cutoff(21),
    align: TextAlign::Left,
    clear: (22, 8),
};

const FRAMES_RAW_STEP: i32 = 26; // ≈0.1

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TopId {
    Exit,
    Filename,
    FramesTick,
    TicksRow,
    Regen,
    Save,
}

pub struct TopField {
    pub id: TopId,
    pub cursor_pos: Vector2D<i32>,
    pub tag: &'static Tag,
    pub tooltip: TooltipText,
    pub col_max: u8,
    pub down_col: u8,
    pub template_only: bool,
}

pub static TOP_FIELDS: [TopField; 6] = [
    TopField {
        id: TopId::Exit,
        cursor_pos: vec2(1, 17),
        tag: &sprites::BUTTON_EXIT_ACTIVE,
        tooltip: TooltipText {
            help: b"EXIT",
            buttons: b"EXIT: A",
        },
        col_max: 0,
        down_col: 0,
        template_only: false,
    },
    TopField {
        id: TopId::Filename,
        cursor_pos: vec2(25, 17),
        tag: &sprites::HIGHLIGHT_SFX_FILENAME,
        tooltip: TooltipText {
            help: b"SFX FILENAME",
            buttons: b"EDIT: A",
        },
        col_max: 5,
        down_col: 0,
        template_only: false,
    },
    TopField {
        id: TopId::FramesTick,
        cursor_pos: vec2(140, 17),
        tag: &sprites::HIGHLIGHT_SFX_NUM,
        tooltip: TooltipText {
            help: b"FRAMES PER TICK",
            buttons: BUTTONS_NUM,
        },
        col_max: 8,
        down_col: 8,
        template_only: false,
    },
    TopField {
        id: TopId::TicksRow,
        cursor_pos: vec2(190, 17),
        tag: &sprites::HIGHLIGHT_SFX_NUM,
        tooltip: TooltipText {
            help: b"ROWS PLAYED PER TICK",
            buttons: BUTTONS_NUM,
        },
        col_max: 11,
        down_col: 12,
        template_only: false,
    },
    TopField {
        id: TopId::Regen,
        cursor_pos: vec2(216, 16),
        tag: &sprites::BUTTON_REGEN_ACTIVE,
        tooltip: TooltipText {
            help: b"REROLL SFX\nCAN'T BE UNDONE",
            buttons: b"REROLL: A",
        },
        col_max: 12,
        down_col: 12,
        template_only: true,
    },
    TopField {
        id: TopId::Save,
        cursor_pos: vec2(228, 16),
        tag: &sprites::BUTTON_SAVE_ACTIVE,
        tooltip: TooltipText {
            help: b"SAVE",
            buttons: b"SAVE: A",
        },
        col_max: u8::MAX,
        down_col: 13,
        template_only: false,
    },
];

fn visible(field: &TopField, template: bool) -> bool {
    template || !field.template_only
}

pub fn left_of(idx: usize, template: bool) -> Option<usize> {
    TOP_FIELDS[..idx]
        .iter()
        .rposition(|field| visible(field, template))
}

pub fn right_of(idx: usize, template: bool) -> Option<usize> {
    TOP_FIELDS[idx + 1..]
        .iter()
        .position(|field| visible(field, template))
        .map(|offset| idx + 1 + offset)
}

/// The field the cursor lands on when moving up from the given on-screen
/// column offset.
pub fn field_above(offset: u8, template: bool) -> usize {
    TOP_FIELDS
        .iter()
        .enumerate()
        .filter(|(_, field)| visible(field, template))
        .find(|(_, field)| offset <= field.col_max)
        .map(|(idx, _)| idx)
        .unwrap_or(TOP_FIELDS.len() - 1)
}

pub fn regen_pos() -> Vector2D<i32> {
    TOP_FIELDS
        .iter()
        .find(|field| field.id == TopId::Regen)
        .expect("regen field exists")
        .cursor_pos
}

pub fn handle_top_input(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    id: TopId,
) -> InputResult {
    let a_pressed = button_controller.is_just_pressed(Button::A);
    let delta = lr_delta(button_controller);
    match id {
        TopId::Exit if a_pressed => InputResult::Exit,
        TopId::Regen if a_pressed => InputResult::Regen,
        TopId::FramesTick
            if delta != 0
                && doc.adjust_frames_per_tick(delta * FRAMES_RAW_STEP, FRAMES_PER_TICK_UI_MAX) =>
        {
            InputResult::EditedTop
        }
        TopId::TicksRow if delta != 0 && doc.adjust_ticks_per_row(delta) => InputResult::EditedTop,
        TopId::Filename if a_pressed => InputResult::EditFilename,
        TopId::Save if a_pressed => InputResult::Save,
        _ => InputResult::None,
    }
}

pub fn draw_top_text(
    filename: &[u8],
    sfx_document: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    text_renderer.draw_text(filename, FONT, bg_text, POS_NAME, &FORMAT_NAME);

    let mut buf = ArrayString::<8>::new();
    push_num(&mut buf, sfx_document.ticks_per_row() as i32);
    text_renderer.draw_text(buf.as_bytes(), FONT, bg_text, POS_TICKS, &FORMAT_NUM);

    buf.clear();
    push_fixed(&mut buf, sfx_document.frames_per_tick().to_raw(), 8);
    text_renderer.draw_text(buf.as_bytes(), FONT, bg_text, POS_FRAMES, &FORMAT_NUM);
}
