use crate::scenes::sfx_editor::common::cursor::{Cursor, MidZones, NavCtx, Target, settle};
use crate::scenes::sfx_editor::common::layout::{
    COL_STRIDE, CURSOR_X_NUDGE, ROWS_ORIGIN, VISIBLE_COLUMNS,
};
use crate::scenes::sfx_editor::common::rows;
use crate::scenes::sfx_editor::common::tooltip::TooltipText;
use crate::scenes::sfx_editor::common::top_bar;
use crate::scenes::sfx_editor::common::top_bar::TOP_FIELDS;
use crate::scenes::sfx_editor::wave::layout::INSTR_ORIGIN;
use crate::scenes::sfx_editor::wave::layout::{
    TOOLTIP_DATA, TOOLTIP_IID, TOOLTIP_INSTR_WID, TOOLTIP_VOL, TOOLTIP_WAVE_WID, VISIBLE_ITEMS,
    WAVE_WID_X, item_text_y, nibble_x,
};
use crate::sfx_doc::SfxDocument;
use agb::display::object::Tag;
use agb::fixnum::{Vector2D, vec2};
use agb_eb_ext::direction::Direction;
use resources::sprites;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InstrField {
    Iid,
    Wid,
    Vol,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WaveField {
    Wid,
    Nibble(u8),
}

#[derive(Debug, Copy, Clone)]
pub enum WaveZone {
    Instr { item: u8, field: InstrField },
    Wave { item: u8, field: WaveField },
}

pub struct WaveMid {
    pub instr_first: u8,
    pub wave_first: u8,
    pub instr_field: InstrField,
    pub wave_field: WaveField,
    pub(super) in_wave_table: bool,
}

pub type WaveCursor = Cursor<WaveMid>;

pub fn new_wave_cursor() -> WaveCursor {
    Cursor::new(
        Target::Top(0),
        WaveMid {
            instr_first: 0,
            wave_first: 0,
            instr_field: InstrField::Iid,
            wave_field: WaveField::Wid,
            in_wave_table: false,
        },
    )
}

fn instr_field_x(field: InstrField) -> i32 {
    INSTR_ORIGIN.x + COL_STRIDE * field as i32
}

fn wave_field_x(field: WaveField) -> i32 {
    match field {
        WaveField::Wid => WAVE_WID_X,
        WaveField::Nibble(nibble) => nibble_x(nibble),
    }
}

/// The rows-grid on-screen column offset nearest a screen x.
fn rows_offset_for_x(x: i32) -> u8 {
    let mut offset = 0;
    while offset + 1 < VISIBLE_COLUMNS && ROWS_ORIGIN.x + COL_STRIDE * (offset as i32 + 1) <= x {
        offset += 1;
    }
    offset
}

fn bottom_visible(first: u8, count: u8) -> u8 {
    first
        .saturating_add(VISIBLE_ITEMS - 1)
        .min(count.saturating_sub(1))
}

fn cross_item(item: u8, from_first: u8, to_first: u8, to_count: u8) -> u8 {
    to_first
        .saturating_add(item.saturating_sub(from_first))
        .min(to_count.saturating_sub(1))
}

impl MidZones for WaveMid {
    type Zone = WaveZone;
    type Moved = (bool, bool);
    const ZONE_COUNT: u8 = 2;

    fn nav(
        &mut self,
        zone: WaveZone,
        direction: Direction,
        rows_first: u8,
        ctx: &NavCtx,
    ) -> Option<Target<WaveZone>> {
        let instr_count = ctx.doc.instruments().len() as u8;
        let wave_count = ctx.doc.wave_tables().len() as u8;
        // A document always has at least one row, so this never underflows.
        let max_row_col = (ctx.doc.row_count() - 1) as u8;
        Some(match zone {
            WaveZone::Instr { item, field } => match direction {
                Direction::Up => {
                    if item == 0 {
                        Target::Top(top_bar::field_above(
                            rows_offset_for_x(instr_field_x(field)),
                            ctx.template,
                        ))
                    } else {
                        Target::Zone(WaveZone::Instr {
                            item: item - 1,
                            field,
                        })
                    }
                }
                Direction::Down => {
                    if item + 1 < instr_count {
                        Target::Zone(WaveZone::Instr {
                            item: item + 1,
                            field,
                        })
                    } else {
                        Target::Rows {
                            row: rows::ROW_NUM,
                            col: rows_first
                                .saturating_add(rows_offset_for_x(instr_field_x(field)))
                                .min(max_row_col),
                        }
                    }
                }
                Direction::Left => Target::Zone(WaveZone::Instr {
                    item,
                    field: match field {
                        InstrField::Iid | InstrField::Wid => InstrField::Iid,
                        InstrField::Vol => InstrField::Wid,
                    },
                }),
                Direction::Right => Target::Zone(match field {
                    InstrField::Iid => WaveZone::Instr {
                        item,
                        field: InstrField::Wid,
                    },
                    InstrField::Wid => WaveZone::Instr {
                        item,
                        field: InstrField::Vol,
                    },
                    InstrField::Vol if wave_count > 0 => WaveZone::Wave {
                        item: cross_item(item, self.instr_first, self.wave_first, wave_count),
                        field: WaveField::Wid,
                    },
                    InstrField::Vol => WaveZone::Instr { item, field },
                }),
            },
            WaveZone::Wave { item, field } => match direction {
                Direction::Up => {
                    if item == 0 {
                        Target::Top(top_bar::field_above(
                            rows_offset_for_x(wave_field_x(field)),
                            ctx.template,
                        ))
                    } else {
                        Target::Zone(WaveZone::Wave {
                            item: item - 1,
                            field,
                        })
                    }
                }
                Direction::Down => {
                    if item + 1 < wave_count {
                        Target::Zone(WaveZone::Wave {
                            item: item + 1,
                            field,
                        })
                    } else {
                        Target::Rows {
                            row: rows::ROW_NUM,
                            col: rows_first
                                .saturating_add(rows_offset_for_x(wave_field_x(field)))
                                .min(max_row_col),
                        }
                    }
                }
                Direction::Left => Target::Zone(match field {
                    WaveField::Wid if instr_count > 0 => WaveZone::Instr {
                        item: cross_item(item, self.wave_first, self.instr_first, instr_count),
                        field: InstrField::Vol,
                    },
                    WaveField::Wid => WaveZone::Wave { item, field },
                    WaveField::Nibble(0) => WaveZone::Wave {
                        item,
                        field: WaveField::Wid,
                    },
                    WaveField::Nibble(nibble) => WaveZone::Wave {
                        item,
                        field: WaveField::Nibble(nibble - 1),
                    },
                }),
                Direction::Right => Target::Zone(WaveZone::Wave {
                    item,
                    field: match field {
                        WaveField::Wid => WaveField::Nibble(0),
                        WaveField::Nibble(nibble) => WaveField::Nibble((nibble + 1).min(31)),
                    },
                }),
            },
        })
    }

    fn enter_from_top(&mut self, idx: usize, ctx: &NavCtx) -> Option<Target<WaveZone>> {
        let instr_count = ctx.doc.instruments().len() as u8;
        let wave_count = ctx.doc.wave_tables().len() as u8;
        let into_instr = WaveZone::Instr {
            item: self.instr_first,
            field: self.instr_field,
        };
        let into_wave = WaveZone::Wave {
            item: self.wave_first,
            field: self.wave_field,
        };
        let zone = if TOP_FIELDS[idx].cursor_pos.x < WAVE_WID_X {
            (instr_count > 0)
                .then_some(into_instr)
                .or((wave_count > 0).then_some(into_wave))
        } else {
            (wave_count > 0)
                .then_some(into_wave)
                .or((instr_count > 0).then_some(into_instr))
        };
        zone.map(Target::Zone)
    }

    fn enter_from_rows(
        &mut self,
        _col: u8,
        _rows_first: u8,
        ctx: &NavCtx,
    ) -> Option<Target<WaveZone>> {
        let instr_count = ctx.doc.instruments().len() as u8;
        let wave_count = ctx.doc.wave_tables().len() as u8;
        let into_instr = (instr_count > 0).then(|| WaveZone::Instr {
            item: bottom_visible(self.instr_first, instr_count),
            field: self.instr_field,
        });
        let into_wave = (wave_count > 0).then(|| WaveZone::Wave {
            item: bottom_visible(self.wave_first, wave_count),
            field: self.wave_field,
        });
        let zone = if self.in_wave_table {
            into_wave.or(into_instr)
        } else {
            into_instr.or(into_wave)
        };
        Some(zone.map(Target::Zone).unwrap_or(Target::Top(0)))
    }

    fn note_zone(&mut self, zone: WaveZone) {
        match zone {
            WaveZone::Instr { field, .. } => {
                self.instr_field = field;
                self.in_wave_table = false;
            }
            WaveZone::Wave { field, .. } => {
                self.wave_field = field;
                self.in_wave_table = true;
            }
        }
    }

    fn clamp(&self, zone: &mut WaveZone, doc: &SfxDocument) {
        match zone {
            WaveZone::Instr { item, .. } => {
                *item = (*item).min((doc.instruments().len() as u8).saturating_sub(1))
            }
            WaveZone::Wave { item, .. } => {
                *item = (*item).min((doc.wave_tables().len() as u8).saturating_sub(1))
            }
        }
    }

    fn settle_windows(&mut self, target: Option<WaveZone>, doc: &SfxDocument) -> (bool, bool) {
        let instr_focus = match target {
            Some(WaveZone::Instr { item, .. }) => Some(item),
            _ => None,
        };
        let wave_focus = match target {
            Some(WaveZone::Wave { item, .. }) => Some(item),
            _ => None,
        };
        (
            settle(
                &mut self.instr_first,
                doc.instruments().len() as u16,
                VISIBLE_ITEMS,
                instr_focus,
            ),
            settle(
                &mut self.wave_first,
                doc.wave_tables().len() as u16,
                VISIBLE_ITEMS,
                wave_focus,
            ),
        )
    }

    fn pos(&self, zone: WaveZone) -> Vector2D<i32> {
        match zone {
            WaveZone::Instr { item, field } => vec2(
                instr_field_x(field) + CURSOR_X_NUDGE,
                item_text_y(item, self.instr_first) - 2,
            ),
            WaveZone::Wave { item, field } => {
                let y = item_text_y(item, self.wave_first) - 2;
                match field {
                    WaveField::Wid => vec2(WAVE_WID_X + CURSOR_X_NUDGE, y),
                    // The 7px highlight surrounds one 3px digit.
                    WaveField::Nibble(nibble) => vec2(nibble_x(nibble) - 2, y),
                }
            }
        }
    }

    fn tag(&self, zone: WaveZone) -> &'static Tag {
        match zone {
            WaveZone::Instr { .. }
            | WaveZone::Wave {
                field: WaveField::Wid,
                ..
            } => &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
            WaveZone::Wave {
                field: WaveField::Nibble(_),
                ..
            } => &sprites::HIGHLIGHT_SFX_DATA,
        }
    }

    fn tooltip(&self, zone: WaveZone) -> TooltipText {
        match zone {
            WaveZone::Instr { field, .. } => match field {
                InstrField::Iid => TOOLTIP_IID,
                InstrField::Wid => TOOLTIP_INSTR_WID,
                InstrField::Vol => TOOLTIP_VOL,
            },
            WaveZone::Wave { field, .. } => match field {
                WaveField::Wid => TOOLTIP_WAVE_WID,
                WaveField::Nibble(_) => TOOLTIP_DATA,
            },
        }
    }

    /// Items and nibbles are excluded so moves along a field stay silent.
    fn zone_key(&self, zone: WaveZone) -> (u8, usize) {
        match zone {
            WaveZone::Instr { field, .. } => (0, field as usize),
            WaveZone::Wave { field, .. } => (
                1,
                match field {
                    WaveField::Wid => 0,
                    WaveField::Nibble(_) => 1,
                },
            ),
        }
    }
}
