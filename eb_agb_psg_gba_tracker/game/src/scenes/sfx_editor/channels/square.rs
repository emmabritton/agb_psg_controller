//! The plain square channel editor (channel 2: no sweep unit). It reuses the
//! sweep editor's row descriptors and edit logic — see [`sweep`] — with the
//! sweep rows sliced off, so both tables stay in the same screen positions.

use crate::scenes::sfx_editor::channels::sweep::{self, SQUARE_SWEEP_ROWS, SweepChannel, SweepRow};
use crate::scenes::sfx_editor::common::table::RowDesc;
use crate::scenes::sfx_editor::editor::ChannelEditor;
use crate::sfx_doc::SfxDocument;
use agb::display::Priority;
use agb::display::tiled::RegularBackground;
use agb_eb_ext::backgrounds::create_filled_bg;
use eb_agb_psg_controller::Instrument;
use resources::bg;

const SQUARE_ROW_COUNT: usize = 6;

pub struct SquareChannel;

impl ChannelEditor for SquareChannel {
    type RowId = SweepRow;

    const NEW_INSTRUMENT: Instrument = SweepChannel::NEW_INSTRUMENT;

    fn rows() -> &'static [RowDesc<Instrument, SweepRow>] {
        &SQUARE_SWEEP_ROWS[..SQUARE_ROW_COUNT]
    }

    fn bg_ui() -> RegularBackground {
        create_filled_bg(&bg::sfx_square, Priority::P3)
    }

    fn matches(instrument: &Instrument) -> bool {
        matches!(instrument, Instrument::Square { sweep: None, .. })
    }

    fn edit_cell(doc: &mut SfxDocument, col: u8, row: SweepRow, delta: i32) -> bool {
        sweep::edit_square_cell(doc, col, row, delta)
    }
}
