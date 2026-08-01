pub mod noise;
pub mod square;
pub mod sweep;

use crate::scenes::sfx_editor::add_clamped;
use crate::scenes::sfx_editor::common::table::{
    RowDesc, RowInput, RowKind, sprite_row, text_index, text_row, updown_sprite,
};
use crate::scenes::sfx_editor::common::text::push_num;
use crate::scenes::sfx_editor::common::tooltip::{BUTTONS_NUM, TooltipText};
use crate::sfx_doc::SfxDocument;
use agb::display::object::Tag;
use arrayvec::ArrayString;
use eb_agb_psg_controller::{EnvelopeSpec, Instrument, SfxChannel, limits};
use resources::sprites;

const STARTER_ENVELOPE: EnvelopeSpec = EnvelopeSpec {
    initial_volume: 15,
    increasing: false,
    step_time: 4,
};

pub const fn starter_instrument(channel: SfxChannel) -> Instrument {
    match channel {
        SfxChannel::SquareSweep | SfxChannel::Square => Instrument::Square {
            duty: 2,
            envelope: STARTER_ENVELOPE,
            sweep: None,
            length: None,
        },
        SfxChannel::Wave => Instrument::Wave {
            wave_table: 0,
            volume: 4,
        },
        SfxChannel::Noise => Instrument::Noise {
            envelope: STARTER_ENVELOPE,
            short_lfsr: false,
        },
    }
}

pub const fn iid_row<Id: Copy>(id: Id) -> RowDesc<Instrument, Id> {
    RowDesc {
        id,
        content_off: 0,
        cursor_off: -2,
        tag: &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
        tooltip: TooltipText {
            help: b"INSTRUMENT ID",
            buttons: b"NEW: R\nDELETE: HOLD B",
        },
        kind: RowKind::Text(text_index),
        input: RowInput::InstrumentId,
    }
}

fn envelope(instrument: &Instrument) -> EnvelopeSpec {
    match instrument {
        Instrument::Square { envelope, .. } | Instrument::Noise { envelope, .. } => *envelope,
        Instrument::Wave { .. } => unreachable!("envelope accessor on wave instrument"),
    }
}

pub fn text_env_vol(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    push_num(out, envelope(instrument).initial_volume as i32);
}

pub fn text_env_time(instrument: &Instrument, _: u8, out: &mut ArrayString<8>) {
    push_num(out, envelope(instrument).step_time as i32);
}

pub fn arrow_env(instrument: &Instrument) -> Option<&'static Tag> {
    Some(updown_sprite(envelope(instrument).increasing))
}

pub const fn env_vol_row<Id: Copy>(id: Id, y: i32) -> RowDesc<Instrument, Id> {
    text_row(id, y, b"ENVELOPE INITIAL VOLUME", BUTTONS_NUM, text_env_vol)
}

pub const fn env_inc_row<Id: Copy>(id: Id, y: i32) -> RowDesc<Instrument, Id> {
    sprite_row(id, y, b"ENVELOPE INCREASING\nLOUDER OR QUIETER?", arrow_env)
}

pub const fn env_time_row<Id: Copy>(id: Id, y: i32) -> RowDesc<Instrument, Id> {
    text_row(
        id,
        y,
        b"ENVELOPE STEP TIME\nHOW FAST IT CHANGES (0-7) HIGHER FASTER",
        BUTTONS_NUM,
        text_env_time,
    )
}

pub enum EnvField {
    Vol,
    Inc,
    Time,
}

pub fn edit_envelope(envelope: &mut EnvelopeSpec, field: EnvField, delta: i32) {
    match field {
        EnvField::Vol => {
            envelope.initial_volume =
                add_clamped(envelope.initial_volume, delta, 0, limits::ENV_VOLUME_MAX)
        }
        EnvField::Inc => {
            if delta < 0 {
                envelope.increasing = !envelope.increasing;
            }
        }
        EnvField::Time => {
            envelope.step_time =
                add_clamped(envelope.step_time, delta, 0, limits::ENV_STEP_TIME_MAX)
        }
    }
}

pub fn edit_instrument_with(
    doc: &mut SfxDocument,
    index: u8,
    f: impl FnOnce(&mut Instrument),
) -> bool {
    let Some(&before) = doc.instruments().get(index as usize) else {
        return false;
    };
    let mut instrument = before;
    f(&mut instrument);
    doc.set_instrument(index, instrument);
    doc.instruments()[index as usize] != before
}
