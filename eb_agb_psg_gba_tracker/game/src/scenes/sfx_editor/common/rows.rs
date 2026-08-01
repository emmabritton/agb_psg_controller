use crate::scenes::sfx_editor::common::layout::{ROWS_ORIGIN, VISIBLE_COLUMNS};
use crate::scenes::sfx_editor::common::shell::{handle_list_edit, list_edit_result};
use crate::scenes::sfx_editor::common::table::{RowDesc, column_grid, for_each_visible, text_row};
use crate::scenes::sfx_editor::common::text::{push_fixed, push_num};
use crate::scenes::sfx_editor::common::tooltip::{BUTTONS_NUM, TooltipText};
use crate::scenes::sfx_editor::{InputResult, coarse_step, lr_delta};
use crate::sfx_doc::{FRAMES_PER_TICK_UI_MAX, SfxDocument};
use agb::display::tiled::RegularBackground;
use agb::fixnum::Num;
use agb::input::{Button, ButtonController};
use arrayvec::ArrayString;
use eb_agb_psg_controller::{NOTE_NONE, NOTE_OFF, PatternSlot, PsgEffect, limits};
use gba_agb_font_renderer::prelude::TextRenderer;

pub const ROW_NUM: usize = 0;
pub const ROW_IID: usize = 1;
pub const ROW_NOTE: usize = 2;
pub const ROW_EFFECT: usize = 3;
pub const ROW_PARAM1: usize = 4;
pub const ROW_PARAM2: usize = 5;

const BUTTONS_ROW: &[u8] = b"NEW ROW: A\nDELETE: HOLD B";
const BUTTONS_EFFECT: &[u8] = b"CYCLE: L | R";
const BUTTONS_PAN: &[u8] = b"LEFT: L | RIGHT: R";

pub static ROWS_GRID: [RowDesc<PatternSlot, ()>; 6] = [
    text_row((), 0, b"ROW NUMBER", BUTTONS_ROW, text_num),
    text_row((), 8, b"ROW INSTRUMENT", BUTTONS_NUM, text_iid),
    text_row(
        (),
        16,
        b"NOTE",
        b"DEC: L | INC: R\nOCTAVE: HOLD A",
        text_note,
    ),
    text_row((), 24, b"EFFECT", BUTTONS_EFFECT, text_effect),
    text_row((), 32, b"EFFECT", BUTTONS_NUM, text_param1),
    text_row((), 40, b"EFFECT", BUTTONS_NUM, text_param2),
];

enum NumDisplay {
    Decimal,
    Frames,
}

enum ParamKind {
    Num {
        min: i32,
        max: i32,
        step: i32,
        get: fn(&PsgEffect) -> i32,
        set: fn(&PsgEffect, i32) -> PsgEffect,
        display: NumDisplay,
    },
    Pan,
}

struct ParamDesc {
    help: &'static [u8],
    buttons: &'static [u8],
    kind: ParamKind,
}

const fn num_param(
    help: &'static [u8],
    min: i32,
    max: i32,
    get: fn(&PsgEffect) -> i32,
    set: fn(&PsgEffect, i32) -> PsgEffect,
) -> ParamDesc {
    ParamDesc {
        help,
        buttons: BUTTONS_NUM,
        kind: ParamKind::Num {
            min,
            max,
            step: 1,
            get,
            set,
            display: NumDisplay::Decimal,
        },
    }
}

macro_rules! value_param {
    ($variant:ident, $help:expr, $min:expr, $max:expr) => {
        value_param!($variant, $help, $min, $max, u8)
    };
    ($variant:ident, $help:expr, $min:expr, $max:expr, $cast:ty) => {
        num_param(
            $help,
            $min,
            $max,
            |e| match e {
                PsgEffect::$variant(value) => *value as i32,
                _ => 0,
            },
            |_, v| PsgEffect::$variant(v as $cast),
        )
    };
}

macro_rules! pair_params {
    ($variant:ident(), $help1:expr, $help2:expr, $max:expr) => {
        [
            num_param(
                $help1,
                0,
                $max,
                |e| match e {
                    PsgEffect::$variant(first, _) => *first as i32,
                    _ => 0,
                },
                |e, v| match *e {
                    PsgEffect::$variant(_, second) => PsgEffect::$variant(v as u8, second),
                    other => other,
                },
            ),
            num_param(
                $help2,
                0,
                $max,
                |e| match e {
                    PsgEffect::$variant(_, second) => *second as i32,
                    _ => 0,
                },
                |e, v| match *e {
                    PsgEffect::$variant(first, _) => PsgEffect::$variant(first, v as u8),
                    other => other,
                },
            ),
        ]
    };
    ($variant:ident { $f1:ident, $f2:ident }, $help1:expr, $help2:expr, $max:expr) => {
        [
            num_param(
                $help1,
                0,
                $max,
                |e| match e {
                    PsgEffect::$variant { $f1, .. } => *$f1 as i32,
                    _ => 0,
                },
                |e, v| match *e {
                    PsgEffect::$variant { $f2, .. } => PsgEffect::$variant { $f1: v as u8, $f2 },
                    other => other,
                },
            ),
            num_param(
                $help2,
                0,
                $max,
                |e| match e {
                    PsgEffect::$variant { $f2, .. } => *$f2 as i32,
                    _ => 0,
                },
                |e, v| match *e {
                    PsgEffect::$variant { $f1, .. } => PsgEffect::$variant { $f1, $f2: v as u8 },
                    other => other,
                },
            ),
        ]
    };
}

struct EffectDesc {
    default: PsgEffect,
    /// Three letter cell name ("-" for no effect)
    name: &'static str,
    help: &'static [u8],
    params: &'static [ParamDesc],
}

static EFFECT_DESCS: [EffectDesc; 14] = [
    EffectDesc {
        default: PsgEffect::None,
        name: "-",
        help: b"NO EFFECT",
        params: &[],
    },
    EffectDesc {
        default: PsgEffect::Arpeggio(4, 7),
        name: "ARP",
        help: b"ARPEGGIO\nCYCLE BETWEEN SEMITONES",
        params: &pair_params!(
            Arpeggio(),
            b"ARPEGGIO\nFIRST OFFSET IN SEMITONES",
            b"ARPEGGIO\nSECOND OFFSET IN SEMITONES",
            15
        ),
    },
    EffectDesc {
        default: PsgEffect::PortamentoUp(0x10),
        name: "PUP",
        help: b"PORTAMENTO\nINCREASE PITCH",
        params: &[value_param!(PortamentoUp, b"PORTAMENTO\nSPEED", 0, 255)],
    },
    EffectDesc {
        default: PsgEffect::PortamentoDown(0x10),
        name: "PDN",
        help: b"PORTAMENTO\nDECREASE PITCH",
        params: &[value_param!(PortamentoDown, b"PORTAMENTO\nSPEED", 0, 255)],
    },
    EffectDesc {
        default: PsgEffect::TonePortamento(0x10),
        name: "TNP",
        help: b"PORTAMENTO\nSLIDE TO NOTE",
        params: &[value_param!(TonePortamento, b"PORTAMENTO\nSPEED", 0, 255)],
    },
    EffectDesc {
        default: PsgEffect::Vibrato { speed: 8, depth: 6 },
        name: "VIB",
        help: b"VIBRATO\nOSCILLATE PITCH",
        params: &pair_params!(
            Vibrato { speed, depth },
            b"VIBRATO\nSPEED",
            b"VIBRATO\nDEPTH",
            15
        ),
    },
    EffectDesc {
        default: PsgEffect::VolumeSlide(-1),
        name: "VLS",
        help: b"VOLUME SLIDE\nADJUST VOLUME EVERY ROW",
        // Slides beyond the 0-15 volume range don't do anything
        params: &[value_param!(
            VolumeSlide,
            b"VOLUME SLIDE\nCHANGE PER ROW",
            -15,
            15,
            i8
        )],
    },
    EffectDesc {
        default: PsgEffect::NoteCut(1),
        name: "CUT",
        help: b"NOTE CUT\nSILENCE",
        // Ticks per row caps at 31
        params: &[value_param!(NoteCut, b"NOTE CUT\nAT TICK", 0, 31)],
    },
    EffectDesc {
        default: PsgEffect::NoteDelay(1),
        name: "DLY",
        help: b"NOTE DELAY\nDELAY PLAYING NOTE",
        params: &[value_param!(NoteDelay, b"NOTE DELAY\nUNTIL TICK", 0, 31)],
    },
    EffectDesc {
        default: PsgEffect::SetTicksPerRow(4),
        name: "TPR",
        help: b"SET TICKS PER ROW",
        params: &[value_param!(
            SetTicksPerRow,
            b"SET TICKS PER ROW\nROW LENGTH IN TICKS",
            1,
            31
        )],
    },
    EffectDesc {
        default: PsgEffect::SetFramesPerTick(Num::from_raw(2 << 4)),
        name: "FPT",
        help: b"SET FRAMES PER TICK",
        // 4.4 fixed point can't step by exactly 0.1; raw 2 = 0.125 is the closest
        params: &[ParamDesc {
            help: b"SET FRAMES PER TICK\nTICK LENGTH IN FRAMES",
            buttons: BUTTONS_NUM,
            kind: ParamKind::Num {
                min: (limits::FRAMES_PER_TICK_MIN << 4) as i32,
                max: (FRAMES_PER_TICK_UI_MAX << 4) as i32,
                step: 2,
                get: |e| match e {
                    PsgEffect::SetFramesPerTick(frames) => frames.to_raw() as i32,
                    _ => 0,
                },
                set: |_, v| PsgEffect::SetFramesPerTick(Num::from_raw(v as u16)),
                display: NumDisplay::Frames,
            },
        }],
    },
    EffectDesc {
        default: PsgEffect::SetDuty(2),
        name: "DTY",
        help: b"SET DUTY",
        params: &[value_param!(
            SetDuty,
            b"SET DUTY\nHOW OFTEN PULSE CHANGES",
            0,
            3
        )],
    },
    EffectDesc {
        default: PsgEffect::SetPan {
            left: true,
            right: true,
        },
        name: "PAN",
        help: b"SET PANNING",
        params: &[ParamDesc {
            help: b"SET PANNING\nWHICH SPEAKERS PLAY",
            buttons: BUTTONS_PAN,
            kind: ParamKind::Pan,
        }],
    },
    EffectDesc {
        default: PsgEffect::SetVolume(15),
        name: "VOL",
        help: b"SET VOLUME",
        params: &[value_param!(
            SetVolume,
            b"SET VOLUME\nCHANNEL VOLUME",
            0,
            15
        )],
    },
];

fn desc(effect: &PsgEffect) -> &'static EffectDesc {
    &EFFECT_DESCS[cycle_index(effect)]
}

pub fn cycle_index(effect: &PsgEffect) -> usize {
    match effect {
        PsgEffect::None | PsgEffect::PositionJump(_) | PsgEffect::PatternBreak(_) => 0,
        PsgEffect::Arpeggio(..) => 1,
        PsgEffect::PortamentoUp(_) => 2,
        PsgEffect::PortamentoDown(_) => 3,
        PsgEffect::TonePortamento(_) => 4,
        PsgEffect::Vibrato { .. } => 5,
        PsgEffect::VolumeSlide(_) => 6,
        PsgEffect::NoteCut(_) => 7,
        PsgEffect::NoteDelay(_) => 8,
        PsgEffect::SetTicksPerRow(_) => 9,
        PsgEffect::SetFramesPerTick(_) => 10,
        PsgEffect::SetDuty(_) => 11,
        PsgEffect::SetPan { .. } => 12,
        PsgEffect::SetVolume(_) => 13,
    }
}

pub fn max_nav_row(doc: &SfxDocument, col: u8) -> usize {
    match doc.rows().get(col as usize) {
        Some(slot) => ROW_EFFECT + desc(&slot.effect).params.len(),
        None => ROW_NUM,
    }
}

pub fn tooltip_for(doc: &SfxDocument, col: u8, row: usize) -> TooltipText {
    let effect = match doc.rows().get(col as usize) {
        Some(slot) => &slot.effect,
        None => return ROWS_GRID[row.min(ROWS_GRID.len() - 1)].tooltip,
    };
    let desc = desc(effect);
    if row == ROW_EFFECT {
        return TooltipText {
            help: desc.help,
            buttons: BUTTONS_EFFECT,
        };
    }
    match desc.params.get((row == ROW_PARAM2) as usize) {
        Some(param) => TooltipText {
            help: param.help,
            buttons: param.buttons,
        },
        None => TooltipText {
            help: b"NO EFFECT",
            buttons: BUTTONS_NUM,
        },
    }
}

fn text_num(_: &PatternSlot, idx: u8, out: &mut ArrayString<8>) {
    push_num(out, idx as i32);
}

fn text_iid(slot: &PatternSlot, _: u8, out: &mut ArrayString<8>) {
    if slot.instrument == 0 {
        let _ = out.try_push('-');
    } else {
        // References are stored 1-based; display matches the instrument
        // table's 0 based column numbers
        push_num(out, slot.instrument as i32 - 1);
    }
}

fn text_note(slot: &PatternSlot, _: u8, out: &mut ArrayString<8>) {
    format_note(slot.note, out);
}

/// Semitone names within an octave, C first; `true` marks a sharp
static SEMI_NAMES: [(char, bool); 12] = [
    ('C', false),
    ('C', true),
    ('D', false),
    ('D', true),
    ('E', false),
    ('F', false),
    ('F', true),
    ('G', false),
    ('G', true),
    ('A', false),
    ('A', true),
    ('B', false),
];

/// `NOTE_NONE` → "---", `NOTE_OFF` → "OFF"; 1 → "C2" up to 96 → "B9", sharps
/// as "C#2". The inverse of the parser's note mapping:
/// `note = (octave-2)*12 + semi + 1`
fn format_note(note: u8, out: &mut ArrayString<8>) {
    if note == NOTE_NONE {
        let _ = out.try_push_str("---");
        return;
    }
    if note == NOTE_OFF {
        let _ = out.try_push_str("OFF");
        return;
    }
    let mut semi = note - 1;
    let mut octave = b'2';
    while semi >= 12 {
        semi -= 12;
        octave += 1;
    }
    let (name, sharp) = SEMI_NAMES[semi as usize];
    let _ = out.try_push(name);
    if sharp {
        let _ = out.try_push('#');
    }
    let _ = out.try_push(octave as char);
}

fn text_effect(slot: &PatternSlot, _: u8, out: &mut ArrayString<8>) {
    let _ = out.try_push_str(desc(&slot.effect).name);
}

fn text_param1(slot: &PatternSlot, _: u8, out: &mut ArrayString<8>) {
    param_text(&slot.effect, 0, out);
}

fn text_param2(slot: &PatternSlot, _: u8, out: &mut ArrayString<8>) {
    param_text(&slot.effect, 1, out);
}

fn param_text(effect: &PsgEffect, param: usize, out: &mut ArrayString<8>) {
    let Some(param) = desc(effect).params.get(param) else {
        return;
    };
    match &param.kind {
        ParamKind::Num {
            get,
            display: NumDisplay::Decimal,
            ..
        } => push_num(out, get(effect)),
        ParamKind::Num {
            get,
            display: NumDisplay::Frames,
            ..
        } => format_frames(Num::from_raw(get(effect) as u16), out),
        ParamKind::Pan => {
            let PsgEffect::SetPan { left, right } = effect else {
                return;
            };
            let _ = out.try_push(if *left { 'L' } else { '-' });
            let _ = out.try_push(' ');
            let _ = out.try_push(if *right { 'R' } else { '-' });
        }
    }
}

/// One decimal place "2.5"; from 10 up only the integer part fits the cell
fn format_frames(frames: Num<u16, 4>, out: &mut ArrayString<8>) {
    let whole = frames.to_raw() >> 4;
    if whole >= 10 {
        push_num(out, whole as i32);
    } else {
        push_fixed(out, frames.to_raw() as u32, 4);
    }
}

pub fn handle_cell_input(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    col: u8,
    row: usize,
    delete_hold: &mut u8,
) -> InputResult {
    match row {
        ROW_NUM => handle_num(button_controller, doc, col, delete_hold),
        ROW_IID => handle_iid(button_controller, doc, col),
        ROW_NOTE => handle_note(button_controller, doc, col),
        ROW_EFFECT => handle_effect(button_controller, doc, col),
        ROW_PARAM1 | ROW_PARAM2 => handle_param(button_controller, doc, col, row == ROW_PARAM2),
        _ => InputResult::None,
    }
}

fn handle_num(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    col: u8,
    delete_hold: &mut u8,
) -> InputResult {
    let edit = handle_list_edit(
        button_controller,
        doc,
        delete_hold,
        Button::A,
        |doc| doc.remove_row(col as usize),
        // The new row goes after the cursor's, and the cursor follows it.
        |doc| doc.insert_row(col as usize + 1).then_some(col + 1),
    );
    list_edit_result(edit, |move_to| InputResult::RowsChanged { move_to })
}

fn handle_iid(button_controller: &ButtonController, doc: &mut SfxDocument, col: u8) -> InputResult {
    let delta = lr_delta(button_controller);
    if delta == 0 {
        return InputResult::None;
    }
    let count = doc.instruments().len() as i32;
    let Some(slot) = doc.rows().get(col as usize) else {
        return InputResult::None;
    };
    let mut candidate = slot.instrument as i32 + delta;
    while (0..=count).contains(&candidate) {
        if doc.set_row_instrument(col as usize, candidate as u8) {
            return InputResult::RowsChanged { move_to: None };
        }
        candidate += delta;
    }
    InputResult::None
}

fn handle_note(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    col: u8,
) -> InputResult {
    let delta = lr_delta(button_controller);
    if delta == 0 {
        return InputResult::None;
    }
    let Some(&before) = doc.rows().get(col as usize) else {
        return InputResult::None;
    };
    let step = coarse_step(button_controller, 1, 12);
    let current = match before.note {
        NOTE_NONE => 0,
        NOTE_OFF => 1,
        pitch => pitch as i32 + 1,
    };
    let target = (current + delta * step).clamp(0, NOTE_OFF as i32);
    doc.set_note(
        col as usize,
        match target {
            0 => NOTE_NONE,
            1 => NOTE_OFF,
            pitch => (pitch - 1) as u8,
        },
    );
    if doc.rows()[col as usize] != before {
        InputResult::EditedRow(col)
    } else {
        InputResult::None
    }
}

fn handle_effect(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    col: u8,
) -> InputResult {
    let delta = lr_delta(button_controller);
    if delta == 0 {
        return InputResult::None;
    }
    let Some(&before) = doc.rows().get(col as usize) else {
        return InputResult::None;
    };
    let sweep_active = doc.sweep_active_at(col as usize);
    let len = EFFECT_DESCS.len();
    let mut idx = cycle_index(&before.effect);
    for _ in 0..len {
        idx = if delta > 0 {
            if idx + 1 == len { 0 } else { idx + 1 }
        } else if idx == 0 {
            len - 1
        } else {
            idx - 1
        };
        if doc.effect_allowed(EFFECT_DESCS[idx].default, sweep_active) {
            doc.set_effect(col as usize, EFFECT_DESCS[idx].default);
            break;
        }
    }
    if doc.rows()[col as usize] != before {
        InputResult::EditedRow(col)
    } else {
        InputResult::None
    }
}

fn handle_param(
    button_controller: &ButtonController,
    doc: &mut SfxDocument,
    col: u8,
    second: bool,
) -> InputResult {
    let Some(&before) = doc.rows().get(col as usize) else {
        return InputResult::None;
    };
    let Some(param) = desc(&before.effect).params.get(second as usize) else {
        return InputResult::None;
    };
    let edited = match &param.kind {
        ParamKind::Pan => {
            // Each shoulder button owns one side rather than L/R being a delta
            let PsgEffect::SetPan { left, right } = before.effect else {
                return InputResult::None;
            };
            if button_controller.is_just_pressed(Button::L) {
                PsgEffect::SetPan { left: !left, right }
            } else if button_controller.is_just_pressed(Button::R) {
                PsgEffect::SetPan {
                    left,
                    right: !right,
                }
            } else {
                return InputResult::None;
            }
        }
        ParamKind::Num {
            min,
            max,
            step,
            get,
            set,
            ..
        } => {
            let delta = lr_delta(button_controller);
            if delta == 0 {
                return InputResult::None;
            }
            let value = (get(&before.effect) + delta * step).clamp(*min, *max);
            set(&before.effect, value)
        }
    };
    if doc.set_effect(col as usize, edited) && doc.rows()[col as usize] != before {
        InputResult::EditedRow(col)
    } else {
        InputResult::None
    }
}

pub fn draw_row_column(
    idx: u8,
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    column_grid(ROWS_ORIGIN, &ROWS_GRID, first_visible).draw_slot(
        idx,
        doc.rows().get(idx as usize),
        |_| true,
        text_renderer,
        bg_text,
    );
}

pub fn draw_all_row_columns(
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    for_each_visible(first_visible, VISIBLE_COLUMNS, |idx| {
        draw_row_column(idx, first_visible, doc, text_renderer, bg_text);
    });
}
