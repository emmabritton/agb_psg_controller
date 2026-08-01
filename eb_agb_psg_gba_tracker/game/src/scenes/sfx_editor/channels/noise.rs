
use crate::scenes::sfx_editor::channels::{
    EnvField, edit_envelope, edit_instrument_with, env_inc_row, env_time_row, env_vol_row, iid_row,
    starter_instrument,
};
use crate::scenes::sfx_editor::common::table::{RowDesc, RowInput, RowKind, tick_sprite};
use crate::scenes::sfx_editor::common::tooltip::{BUTTONS_ARROW, TooltipText};
use crate::scenes::sfx_editor::editor::ChannelEditor;
use crate::sfx_doc::SfxDocument;
use agb::display::Priority;
use agb::display::object::Tag;
use agb::display::tiled::RegularBackground;
use agb_eb_ext::backgrounds::create_filled_bg;
use eb_agb_psg_controller::{Instrument, SfxChannel};
use resources::{bg, sprites};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum NoiseRow {
    Iid,
    Lfsr,
    EnvVol,
    EnvInc,
    EnvTime,
}

fn flag_lfsr(instrument: &Instrument) -> Option<&'static Tag> {
    match instrument {
        Instrument::Noise { short_lfsr, .. } => Some(tick_sprite(*short_lfsr)),
        _ => unreachable!("lfsr accessor on non noise instrument"),
    }
}

pub static NOISE_ROWS: [RowDesc<Instrument, NoiseRow>; 5] = [
    iid_row(NoiseRow::Iid),
    // Not sprite_row
    RowDesc {
        id: NoiseRow::Lfsr,
        content_off: 8,
        cursor_off: 6,
        tag: &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
        tooltip: TooltipText {
            help: b"SHORT LFSR\n7 BIT METALLIC NOISE",
            buttons: BUTTONS_ARROW,
        },
        kind: RowKind::Sprite(flag_lfsr),
        input: RowInput::Edit,
    },
    env_vol_row(NoiseRow::EnvVol, 16),
    env_inc_row(NoiseRow::EnvInc, 22),
    env_time_row(NoiseRow::EnvTime, 26),
];

pub struct NoiseChannel;

impl ChannelEditor for NoiseChannel {
    type RowId = NoiseRow;

    const NEW_INSTRUMENT: Instrument = starter_instrument(SfxChannel::Noise);

    fn rows() -> &'static [RowDesc<Instrument, NoiseRow>] {
        &NOISE_ROWS
    }

    fn bg_ui() -> RegularBackground {
        create_filled_bg(&bg::sfx_noise, Priority::P3)
    }

    fn matches(instrument: &Instrument) -> bool {
        matches!(instrument, Instrument::Noise { .. })
    }

    fn edit_cell(doc: &mut SfxDocument, col: u8, row: NoiseRow, delta: i32) -> bool {
        edit_instrument_with(doc, col, |instrument| {
            let Instrument::Noise {
                envelope,
                short_lfsr,
            } = instrument
            else {
                // A mismatched instrument draws as `err`; editing it does
                // nothing
                return;
            };
            match row {
                NoiseRow::Iid => {
                    unreachable!("{row:?} is routed to the shared instrument ID handling")
                }
                NoiseRow::Lfsr => {
                    if delta < 0 {
                        *short_lfsr = !*short_lfsr;
                    }
                }
                NoiseRow::EnvVol => edit_envelope(envelope, EnvField::Vol, delta),
                NoiseRow::EnvInc => edit_envelope(envelope, EnvField::Inc, delta),
                NoiseRow::EnvTime => edit_envelope(envelope, EnvField::Time, delta),
            }
        })
    }
}
