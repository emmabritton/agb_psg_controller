use crate::UserStr;
use crate::scenes::sfx_editor::common::cursor::{Cursor, MidZones, NavCtx, Target, settle};
use crate::scenes::sfx_editor::common::layout::{
    COL_STRIDE, CURSOR_X_NUDGE, TABLE_ORIGIN, VISIBLE_COLUMNS,
};
use crate::scenes::sfx_editor::common::rows;
use crate::scenes::sfx_editor::common::shell::{
    CommonTarget, EditorShell, ShellState, handle_hold_action, handle_iid_input,
};
use crate::scenes::sfx_editor::common::table::{
    Grid, RowDesc, RowInput, column_grid, for_each_visible,
};
use crate::scenes::sfx_editor::common::tooltip::TooltipText;
use crate::scenes::sfx_editor::common::top_bar;
use crate::scenes::sfx_editor::common::top_bar::TOP_FIELDS;
use crate::scenes::sfx_editor::{InputResult, ListId, lr_delta};
use crate::sfx_doc::SfxDocument;
use crate::sfx_template::SfxTemplate;
use agb::display::GraphicsFrame;
use agb::display::object::Tag;
use agb::display::tiled::RegularBackground;
use agb::fixnum::{Vector2D, vec2};
use agb::input::{Button, ButtonController};
use agb_eb_ext::direction::Direction;
use core::marker::PhantomData;
use eb_agb_psg_controller::Instrument;
use gba_agb_font_renderer::prelude::TextRenderer;

pub trait ChannelEditor {
    type RowId: Copy + 'static;

    const NEW_INSTRUMENT: Instrument;

    fn rows() -> &'static [RowDesc<Instrument, Self::RowId>];
    fn bg_ui() -> RegularBackground;
    fn matches(instrument: &Instrument) -> bool;
    fn edit_cell(doc: &mut SfxDocument, col: u8, row: Self::RowId, delta: i32) -> bool;
    fn hold_b_complete(_doc: &mut SfxDocument, _col: u8, _row: Self::RowId) -> InputResult {
        InputResult::None
    }
}

#[derive(Debug, Copy, Clone)]
pub struct InstrCell {
    pub row: usize,
    pub col: u8,
}

pub struct InstrumentsMid<C: ChannelEditor> {
    pub first_visible: u8,
    _channel: PhantomData<C>,
}

impl<C: ChannelEditor> MidZones for InstrumentsMid<C> {
    type Zone = InstrCell;
    type Moved = bool;
    const ZONE_COUNT: u8 = 1;

    fn nav(
        &mut self,
        InstrCell { row, col }: InstrCell,
        direction: Direction,
        rows_first: u8,
        ctx: &NavCtx,
    ) -> Option<Target<InstrCell>> {
        let max_col = (ctx.doc.instruments().len() as u8).saturating_sub(1);
        let max_row_col = (ctx.doc.row_count() - 1) as u8;
        Some(match direction {
            Direction::Up => {
                if row == 0 {
                    Target::Top(top_bar::field_above(
                        col.saturating_sub(self.first_visible),
                        ctx.template,
                    ))
                } else {
                    Target::Zone(InstrCell { row: row - 1, col })
                }
            }
            Direction::Down => {
                if row + 1 < C::rows().len() {
                    Target::Zone(InstrCell { row: row + 1, col })
                } else {
                    Target::Rows {
                        row: rows::ROW_NUM,
                        col: rows_first
                            .saturating_add(col.saturating_sub(self.first_visible))
                            .min(max_row_col),
                    }
                }
            }
            Direction::Left => Target::Zone(InstrCell {
                row,
                col: col.saturating_sub(1),
            }),
            Direction::Right => Target::Zone(InstrCell {
                row,
                col: col.saturating_add(1).min(max_col),
            }),
        })
    }

    fn enter_from_top(&mut self, idx: usize, ctx: &NavCtx) -> Option<Target<InstrCell>> {
        let count = ctx.doc.instruments().len() as u8;
        (count > 0).then(|| {
            Target::Zone(InstrCell {
                row: 0,
                col: self
                    .first_visible
                    .saturating_add(TOP_FIELDS[idx].down_col)
                    .min(count - 1),
            })
        })
    }

    fn enter_from_rows(
        &mut self,
        col: u8,
        rows_first: u8,
        ctx: &NavCtx,
    ) -> Option<Target<InstrCell>> {
        let count = ctx.doc.instruments().len() as u8;
        // With no instruments there is nothing above to land on
        (count > 0).then(|| {
            Target::Zone(InstrCell {
                row: C::rows().len() - 1,
                col: self
                    .first_visible
                    .saturating_add(col.saturating_sub(rows_first))
                    .min(count - 1),
            })
        })
    }

    fn clamp(&self, zone: &mut InstrCell, doc: &SfxDocument) {
        zone.col = zone
            .col
            .min((doc.instruments().len() as u8).saturating_sub(1));
    }

    fn settle_windows(&mut self, target: Option<InstrCell>, doc: &SfxDocument) -> bool {
        settle(
            &mut self.first_visible,
            doc.instruments().len() as u16,
            VISIBLE_COLUMNS,
            target.map(|zone| zone.col),
        )
    }

    fn pos(&self, zone: InstrCell) -> Vector2D<i32> {
        TABLE_ORIGIN
            + vec2(
                CURSOR_X_NUDGE + COL_STRIDE * zone.col.saturating_sub(self.first_visible) as i32,
                C::rows()[zone.row].cursor_off,
            )
    }

    fn tag(&self, zone: InstrCell) -> &'static Tag {
        C::rows()[zone.row].tag
    }

    fn tooltip(&self, zone: InstrCell) -> TooltipText {
        C::rows()[zone.row].tooltip
    }

    fn zone_key(&self, zone: InstrCell) -> (u8, usize) {
        (0, zone.row)
    }
}

type EditorCursor<C> = Cursor<InstrumentsMid<C>>;

fn new_cursor<C: ChannelEditor>(regen: bool) -> EditorCursor<C> {
    Cursor::new(
        Target::Top(if regen { 4 } else { 1 }),
        InstrumentsMid {
            first_visible: 0,
            _channel: PhantomData,
        },
    )
}

pub struct SfxEditor<C: ChannelEditor> {
    shell: ShellState,
    bg_ui: RegularBackground,
    cursor: EditorCursor<C>,
}

impl<C: ChannelEditor> SfxEditor<C> {
    pub fn new(doc: SfxDocument, template: Option<SfxTemplate>, file: Option<UserStr>) -> Self {
        let mut shell = ShellState::new(doc, template, file);
        draw_all_columns::<C>(0, &shell.doc, &mut shell.text_renderer, &mut shell.bg_text);
        rows::draw_all_row_columns(0, &shell.doc, &mut shell.text_renderer, &mut shell.bg_text);
        let mut editor = Self {
            shell,
            bg_ui: C::bg_ui(),
            cursor: new_cursor::<C>(template.is_some()),
        };
        editor.refresh_tooltip(None);
        editor
    }

    fn draw_instrument_column(&mut self, idx: u8) {
        draw_one_column::<C>(
            idx,
            self.cursor.mid.first_visible,
            &self.shell.doc,
            &mut self.shell.text_renderer,
            &mut self.shell.bg_text,
        );
    }

    fn redraw_instruments(&mut self) {
        draw_all_columns::<C>(
            self.cursor.mid.first_visible,
            &self.shell.doc,
            &mut self.shell.text_renderer,
            &mut self.shell.bg_text,
        );
    }
}

impl<C: ChannelEditor> EditorShell for SfxEditor<C> {
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
            Target::Zone(_) => CommonTarget::Mid,
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

    fn extra_sprites(&self, frame: &mut GraphicsFrame) {
        let grid = instrument_grid::<C>(self.cursor.mid.first_visible);
        self.shell
            .doc
            .instruments()
            .iter()
            .enumerate()
            .skip(self.cursor.mid.first_visible as usize)
            .take(VISIBLE_COLUMNS as usize)
            .filter(|(_, instr)| C::matches(instr))
            .for_each(|(idx, instr)| {
                grid.draw_sprites(idx as u8, instr, frame);
            });
    }

    fn cursor_on_hold_cell(&self) -> bool {
        match self.cursor.target {
            Target::Zone(InstrCell { row, .. }) => matches!(
                C::rows()[row].input,
                RowInput::InstrumentId | RowInput::EditWithHoldClear
            ),
            Target::Rows { row, .. } => row == rows::ROW_NUM,
            Target::Top(_) => false,
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
        let (instruments_moved, rows_moved) = self.cursor.scroll_to_cursor(&self.shell.doc);
        if instruments_moved {
            self.redraw_instruments();
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
        self.cursor.mid.first_visible = 0;
        self.cursor.rows_first = 0;
    }

    fn set_cursor_list_item(&mut self, _list: ListId, item: u8) {
        if let Target::Zone(zone) = &mut self.cursor.target {
            zone.col = item;
        }
    }

    fn set_cursor_rows_col(&mut self, new_col: u8) {
        if let Target::Rows { col, .. } = &mut self.cursor.target {
            *col = new_col;
        }
    }

    fn handle_mid_input(&mut self, button_controller: &ButtonController) -> InputResult {
        let Target::Zone(InstrCell { row, col }) = self.cursor.target else {
            return InputResult::None;
        };
        let desc = &C::rows()[row];
        match desc.input {
            RowInput::InstrumentId => handle_iid_input(
                button_controller,
                &mut self.shell.doc,
                col,
                &mut self.shell.delete_hold,
                C::NEW_INSTRUMENT,
            ),
            RowInput::EditWithHoldClear if button_controller.is_pressed(Button::B) => {
                handle_hold_action(
                    button_controller,
                    &mut self.shell.doc,
                    &mut self.shell.delete_hold,
                    |doc| C::hold_b_complete(doc, col, desc.id),
                )
            }
            _ => {
                let delta = lr_delta(button_controller);
                if delta != 0 && C::edit_cell(&mut self.shell.doc, col, desc.id, delta) {
                    InputResult::EditedList {
                        list: ListId::Instruments,
                        item: col,
                    }
                } else {
                    InputResult::None
                }
            }
        }
    }

    fn draw_list_item(&mut self, list: ListId, item: u8) {
        debug_assert!(list == ListId::Instruments, "grid editors have one list");
        self.draw_instrument_column(item);
    }

    fn redraw_after_list_change(&mut self, _list: ListId) {
        self.redraw_instruments();
        self.redraw_rows();
    }

    fn redraw_all(&mut self) {
        self.redraw_instruments();
        self.redraw_rows();
    }
}

fn instrument_grid<C: ChannelEditor>(first_visible: u8) -> Grid<Instrument, C::RowId> {
    column_grid(TABLE_ORIGIN, C::rows(), first_visible)
}

fn draw_one_column<C: ChannelEditor>(
    idx: u8,
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    instrument_grid::<C>(first_visible).draw_slot(
        idx,
        doc.instruments().get(idx as usize),
        C::matches,
        text_renderer,
        bg_text,
    );
}

fn draw_all_columns<C: ChannelEditor>(
    first_visible: u8,
    doc: &SfxDocument,
    text_renderer: &mut TextRenderer,
    bg_text: &mut RegularBackground,
) {
    for_each_visible(first_visible, VISIBLE_COLUMNS, |idx| {
        draw_one_column::<C>(idx, first_visible, doc, text_renderer, bg_text);
    });
}
