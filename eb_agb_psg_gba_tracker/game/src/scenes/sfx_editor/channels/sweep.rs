
use crate::scenes::sfx_editor::add_clamped;
use crate::scenes::sfx_editor::channels::{
    EnvField, edit_envelope, edit_instrument_with, env_inc_row, env_time_row, env_vol_row, iid_row,
    starter_instrument,
};
use crate::scenes::sfx_editor::common::table::{
    RowDesc, RowInput, RowKind, sprite_row, text_row, updown_sprite,
};
use crate::scenes::sfx_editor::common::text::push_opt_num;
use crate::scenes::sfx_editor::common::tooltip::{BUTTONS_NUM, TooltipText};
use crate::scenes::sfx_editor::editor::ChannelEditor;
use crate::scenes::sfx_editor::{InputResult, ListId};
use crate::sfx_doc::SfxDocument;
use agb::display::Priority;
use agb::display::object::Tag;
use agb::display::tiled::RegularBackground;
use agb_eb_ext::backgrounds::create_filled_bg;
use arrayvec::ArrayString;
use eb_agb_psg_controller::{EnvelopeSpec, Instrument, SfxChannel, SweepSpec, limits};
use resources::{bg, sprites};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SweepRow {
    Iid,
    Duty,
    Len,
    EnvVol,
    EnvInc,
    EnvTime,
    SweepTime,
    SweepDec,
    SweepShift,
}

static DUTY_TEXT: [&str; 4] = ["1/8", "1/4", "1/2", "3/4"];

fn square(instrument: &Instrument) -> (u8, EnvelopeSpec, Option<SweepSpec>, Option<u8>) {
    match instrument {
        Instrument::Square {
            duty,
            envelope,
            sweep,
            length,
        } => (*duty, *envelope, *sweep, *length),
        _ => unreachable!("square accessor on non square instrument"),
    }
}

fn text_duty(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    let _ = out.try_push_str(DUTY_TEXT[square(instrument).0 as usize]);
}

fn text_len(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    push_opt_num(out, square(instrument).3);
}

// An instrument with no sweep still paints its cells (as `-`), both to wipe a
// sweep that was just cleared and because scrolling reuses the column for a
// different instrument
fn text_sweep_time(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    push_opt_num(out, square(instrument).2.map(|sweep| sweep.time));
}

fn text_sweep_shift(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    push_opt_num(out, square(instrument).2.map(|sweep| sweep.shift));
}

fn arrow_sweep(instrument: &Instrument) -> Option<&'static Tag> {
    square(instrument)
        .2
        .map(|sweep| updown_sprite(!sweep.decreasing))
}

pub static SQUARE_SWEEP_ROWS: [RowDesc<Instrument, SweepRow>; 9] = [
    iid_row(SweepRow::Iid),
    text_row(
        SweepRow::Duty,
        8,
        b"DUTY\nHOW OFTEN PULSE CHANGES",
        BUTTONS_NUM,
        text_duty,
    ),
    text_row(SweepRow::Len, 16, b"LENGTH", BUTTONS_NUM, text_len),
    env_vol_row(SweepRow::EnvVol, 24),
    env_inc_row(SweepRow::EnvInc, 30),
    env_time_row(SweepRow::EnvTime, 34),
    // Not `text_row`: holding B clears the sweep, so the input differs.
    RowDesc {
        id: SweepRow::SweepTime,
        content_off: 42,
        cursor_off: 40,
        tag: &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
        tooltip: TooltipText {
            help: b"SWEEP TIME",
            buttons: b"DEC: L | INC: R\nCLEAR: HOLD B",
        },
        kind: RowKind::Text(text_sweep_time),
        input: RowInput::EditWithHoldClear,
    },
    sprite_row(SweepRow::SweepDec, 48, b"SWEEP DECREASING", arrow_sweep),
    text_row(
        SweepRow::SweepShift,
        52,
        b"SWEEP SHIFT",
        BUTTONS_NUM,
        text_sweep_shift,
    ),
];

pub struct SweepChannel;

impl ChannelEditor for SweepChannel {
    type RowId = SweepRow;

    const NEW_INSTRUMENT: Instrument = starter_instrument(SfxChannel::SquareSweep);

    fn rows() -> &'static [RowDesc<Instrument, SweepRow>] {
        &SQUARE_SWEEP_ROWS
    }

    fn bg_ui() -> RegularBackground {
        create_filled_bg(&bg::sfx_sweep, Priority::P3)
    }

    fn matches(instrument: &Instrument) -> bool {
        matches!(instrument, Instrument::Square { .. })
    }

    fn edit_cell(doc: &mut SfxDocument, col: u8, row: SweepRow, delta: i32) -> bool {
        edit_square_cell(doc, col, row, delta)
    }

    fn hold_b_complete(doc: &mut SfxDocument, col: u8, _row: SweepRow) -> InputResult {
        let cleared = edit_instrument_with(doc, col, |instrument| {
            if let Instrument::Square { sweep, .. } = instrument {
                *sweep = None;
            }
        });
        if cleared {
            InputResult::EditedList {
                list: ListId::Instruments,
                item: col,
            }
        } else {
            InputResult::None
        }
    }
}

pub fn edit_square_cell(doc: &mut SfxDocument, col: u8, row: SweepRow, delta: i32) -> bool {
    let channel = doc.channel();
    edit_instrument_with(doc, col, |instrument| {
        let Instrument::Square {
            duty,
            envelope,
            sweep,
            length,
        } = instrument
        else {
            // A mismatched instrument draws as `err`; editing it does nothing.
            return;
        };
        match row {
            SweepRow::Iid => {
                unreachable!("{row:?} is routed to the shared instrument ID handling")
            }
            SweepRow::Duty => *duty = add_clamped(*duty, delta, 0, limits::DUTY_MAX),
            SweepRow::Len => {
                // Going below 1 sets length to None
                *length = match *length {
                    Some(l) => match add_clamped(l, delta, 0, limits::SQUARE_LENGTH_MAX) {
                        0 => None,
                        l => Some(l),
                    },
                    None if delta > 0 => Some(limits::SQUARE_LENGTH_MIN),
                    None => None,
                }
            }
            SweepRow::EnvVol => edit_envelope(envelope, EnvField::Vol, delta),
            SweepRow::EnvInc => edit_envelope(envelope, EnvField::Inc, delta),
            SweepRow::EnvTime => edit_envelope(envelope, EnvField::Time, delta),
            SweepRow::SweepTime => {
                *sweep = match *sweep {
                    Some(sweep) => Some(SweepSpec {
                        time: add_clamped(sweep.time, delta, 0, limits::SWEEP_TIME_MAX),
                        ..sweep
                    }),
                    None if delta > 0 && matches!(channel, SfxChannel::SquareSweep) => {
                        Some(SweepSpec {
                            time: 1,
                            decreasing: false,
                            shift: 0,
                        })
                    }
                    None => None,
                }
            }
            SweepRow::SweepDec => {
                if let Some(sweep) = sweep
                    && delta < 0
                {
                    sweep.decreasing = !sweep.decreasing;
                }
            }
            SweepRow::SweepShift => {
                if let Some(sweep) = sweep {
                    sweep.shift = add_clamped(sweep.shift, delta, 0, limits::SWEEP_SHIFT_MAX);
                }
            }
        }
    })
}
