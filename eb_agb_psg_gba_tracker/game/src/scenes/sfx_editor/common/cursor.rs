use crate::scenes::sfx_editor::common::layout::{
    COL_STRIDE, CURSOR_X_NUDGE, ROWS_ORIGIN, VISIBLE_COLUMNS,
};
use crate::scenes::sfx_editor::common::rows;
use crate::scenes::sfx_editor::common::tooltip::TooltipText;
use crate::scenes::sfx_editor::common::top_bar;
use crate::scenes::sfx_editor::common::top_bar::TOP_FIELDS;
use crate::sfx_doc::SfxDocument;
use agb::display::object::Tag;
use agb::fixnum::{Vector2D, vec2};
use agb_eb_ext::direction::Direction;

#[derive(Debug, Copy, Clone)]
pub enum Target<Z: Copy> {
    Top(usize),
    Zone(Z),
    /// `col` is the absolute pattern-row index, not the on-screen column
    Rows {
        row: usize,
        col: u8,
    },
}

pub struct NavCtx<'a> {
    pub doc: &'a SfxDocument,
    /// Whether the regen top-bar button exists 
    pub template: bool,
}

pub trait MidZones {
    type Zone: Copy;
    type Moved;
    const ZONE_COUNT: u8;

    fn nav(
        &mut self,
        zone: Self::Zone,
        direction: Direction,
        rows_first: u8,
        ctx: &NavCtx,
    ) -> Option<Target<Self::Zone>>;
    fn enter_from_top(&mut self, idx: usize, ctx: &NavCtx) -> Option<Target<Self::Zone>>;
    fn enter_from_rows(
        &mut self,
        col: u8,
        rows_first: u8,
        ctx: &NavCtx,
    ) -> Option<Target<Self::Zone>>;
    fn note_zone(&mut self, _zone: Self::Zone) {}

    fn clamp(&self, zone: &mut Self::Zone, doc: &SfxDocument);
    fn settle_windows(&mut self, target: Option<Self::Zone>, doc: &SfxDocument) -> Self::Moved;

    fn pos(&self, zone: Self::Zone) -> Vector2D<i32>;
    fn tag(&self, zone: Self::Zone) -> &'static Tag;
    fn tooltip(&self, zone: Self::Zone) -> TooltipText;
    fn zone_key(&self, zone: Self::Zone) -> (u8, usize);
}

pub struct Cursor<M: MidZones> {
    pub target: Target<M::Zone>,
    pub rows_first: u8,
    pub mid: M,
}

impl<M: MidZones> Cursor<M> {
    pub fn new(target: Target<M::Zone>, mid: M) -> Self {
        Self {
            target,
            rows_first: 0,
            mid,
        }
    }

    pub fn next(&mut self, direction: Direction, ctx: &NavCtx) -> bool {
        let new_target = match self.target {
            Target::Top(idx) => match direction {
                Direction::Up => None,
                Direction::Down => self.mid.enter_from_top(idx, ctx),
                Direction::Left => top_bar::left_of(idx, ctx.template).map(Target::Top),
                Direction::Right => top_bar::right_of(idx, ctx.template).map(Target::Top),
            },
            Target::Zone(zone) => self.mid.nav(zone, direction, self.rows_first, ctx),
            Target::Rows { row, col } => match rows_nav(direction, row, col, ctx.doc) {
                RowsNav::To { row, col } => Some(Target::Rows { row, col }),
                RowsNav::Exit { col } => self.mid.enter_from_rows(col, self.rows_first, ctx),
                RowsNav::None => None,
            },
        };
        if let Some(target) = new_target {
            if let Target::Zone(zone) = target {
                self.mid.note_zone(zone);
            }
            self.target = target;
            true
        } else {
            false
        }
    }

    pub fn scroll_to_cursor(&mut self, doc: &SfxDocument) -> (M::Moved, bool) {
        let mid_target = match self.target {
            Target::Zone(zone) => Some(zone),
            _ => None,
        };
        let mid_moved = self.mid.settle_windows(mid_target, doc);
        let rows_focus = match self.target {
            Target::Rows { col, .. } => Some(col),
            _ => None,
        };
        let rows_moved = settle(
            &mut self.rows_first,
            doc.row_count() as u16,
            VISIBLE_COLUMNS,
            rows_focus,
        );
        (mid_moved, rows_moved)
    }

    pub fn clamp_to(&mut self, doc: &SfxDocument) {
        match &mut self.target {
            Target::Top(_) => {}
            Target::Zone(zone) => self.mid.clamp(zone, doc),
            Target::Rows { row, col } => rows_clamp(row, col, doc),
        }
    }

    pub fn pos(&self) -> Vector2D<i32> {
        match self.target {
            Target::Top(idx) => TOP_FIELDS[idx].cursor_pos,
            Target::Zone(zone) => self.mid.pos(zone),
            Target::Rows { row, col } => rows_cell_pos(row, col, self.rows_first),
        }
    }

    pub fn tag(&self) -> &'static Tag {
        match self.target {
            Target::Top(idx) => TOP_FIELDS[idx].tag,
            Target::Zone(zone) => self.mid.tag(zone),
            Target::Rows { row, .. } => rows::ROWS_GRID[row].tag,
        }
    }

    pub fn tooltip(&self) -> TooltipText {
        match self.target {
            Target::Top(idx) => TOP_FIELDS[idx].tooltip,
            Target::Zone(zone) => self.mid.tooltip(zone),
            Target::Rows { row, .. } => rows::ROWS_GRID[row].tooltip,
        }
    }

    pub fn tooltip_zone_row(&self) -> (u8, usize) {
        match self.target {
            Target::Top(idx) => (0, idx),
            Target::Zone(zone) => {
                let (key, row) = self.mid.zone_key(zone);
                (1 + key, row)
            }
            Target::Rows { row, .. } => (1 + M::ZONE_COUNT, row),
        }
    }
}

pub enum RowsNav {
    To { row: usize, col: u8 },
    Exit { col: u8 },
    None,
}

pub fn rows_nav(direction: Direction, row: usize, col: u8, doc: &SfxDocument) -> RowsNav {
    // A document always has at least one row, so this never underflows.
    let max_row_col = (doc.row_count() - 1) as u8;
    match direction {
        Direction::Up => {
            if row == rows::ROW_NUM {
                RowsNav::Exit { col }
            } else {
                RowsNav::To { row: row - 1, col }
            }
        }
        Direction::Down => {
            if row < rows::max_nav_row(doc, col) {
                RowsNav::To { row: row + 1, col }
            } else {
                RowsNav::None
            }
        }
        Direction::Left => {
            let col = col.saturating_sub(1);
            RowsNav::To {
                row: row.min(rows::max_nav_row(doc, col)),
                col,
            }
        }
        Direction::Right => {
            let col = col.saturating_add(1).min(max_row_col);
            RowsNav::To {
                row: row.min(rows::max_nav_row(doc, col)),
                col,
            }
        }
    }
}

pub fn rows_clamp(row: &mut usize, col: &mut u8, doc: &SfxDocument) {
    *col = (*col).min((doc.row_count() - 1) as u8);
    *row = (*row).min(rows::max_nav_row(doc, *col));
}

/// Cursor sprite position for a rows-grid cell.
pub fn rows_cell_pos(row: usize, col: u8, first_visible: u8) -> Vector2D<i32> {
    ROWS_ORIGIN
        + vec2(
            CURSOR_X_NUDGE + COL_STRIDE * col.saturating_sub(first_visible) as i32,
            rows::ROWS_GRID[row].cursor_off,
        )
}

pub fn slide(first: &mut u8, value: u8, window: u8) {
    if value < *first {
        *first = value;
    } else if value >= first.saturating_add(window) {
        *first = value - (window - 1);
    }
}

pub fn settle(first: &mut u8, count: u16, window: u8, focus: Option<u8>) -> bool {
    let before = *first;
    *first = (*first).min(count.saturating_sub(window as u16) as u8);
    if let Some(value) = focus {
        slide(first, value, window);
    }
    *first != before
}
