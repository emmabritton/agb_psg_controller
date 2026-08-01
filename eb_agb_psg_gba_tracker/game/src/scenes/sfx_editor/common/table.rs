
use crate::scenes::sfx_editor::common::layout::{ARROW_X_NUDGE, CELL_WIDTH, COL_STRIDE};
use crate::scenes::sfx_editor::common::text::push_num;
use crate::scenes::sfx_editor::common::tooltip::{BUTTONS_ARROW, TooltipText};
use agb::display::GraphicsFrame;
use agb::display::object::Tag;
use agb::display::tiled::RegularBackground;
use agb::fixnum::{Vector2D, vec2};
use agb_eb_ext::gfx::ShowSprite;
use arrayvec::ArrayString;
use gba_agb_font_renderer::prelude::{PrintableFont, TextRenderer};
use gba_agb_font_renderer::{TextAlign, TextFormat, TextOverflow};
use resources::{FONT_BLACK, sprites};

static FONT: &PrintableFont = &FONT_BLACK;

pub static CELL_FORMAT: TextFormat = TextFormat {
    overflow: TextOverflow::Cutoff(CELL_WIDTH - 2),
    align: TextAlign::Center(CELL_WIDTH),
    clear: (CELL_WIDTH, 8),
};

pub enum RowKind<T: 'static> {
    Text(fn(&T, u8, &mut ArrayString<8>)),
    Sprite(fn(&T) -> Option<&'static Tag>),
}

pub fn updown_sprite(up: bool) -> &'static Tag {
    if up {
        &sprites::TABLE_UP
    } else {
        &sprites::TABLE_DOWN
    }
}

pub fn tick_sprite(on: bool) -> &'static Tag {
    if on {
        &sprites::TABLE_TICK
    } else {
        &sprites::TABLE_CROSS
    }
}

pub enum RowInput {
    Edit,
    InstrumentId,
    EditWithHoldClear,
}

pub struct RowDesc<T: 'static, Id: Copy + 'static> {
    pub id: Id,
    pub content_off: i32,
    pub cursor_off: i32,
    pub tag: &'static Tag,
    pub tooltip: TooltipText,
    pub kind: RowKind<T>,
    pub input: RowInput,
}

pub struct Grid<T: 'static, Id: Copy + 'static> {
    pub origin: Vector2D<i32>,
    pub axis: Vector2D<i32>,
    pub cross: Vector2D<i32>,
    pub rows: &'static [RowDesc<T, Id>],
    pub clear_size: Vector2D<i32>,
    pub first_visible: u8,
}

pub fn column_grid<T, Id: Copy>(
    origin: Vector2D<i32>,
    rows: &'static [RowDesc<T, Id>],
    first_visible: u8,
) -> Grid<T, Id> {
    let height = rows.iter().map(|row| row.content_off).max().unwrap_or(0) + 8;
    Grid {
        origin,
        axis: vec2(COL_STRIDE, 0),
        cross: vec2(0, 1),
        rows,
        clear_size: vec2(CELL_WIDTH as i32, height),
        first_visible,
    }
}

impl<T, Id: Copy> Grid<T, Id> {
    fn cell_origin(&self, idx: u8) -> Vector2D<i32> {
        self.origin + self.axis * idx.saturating_sub(self.first_visible) as i32
    }

    pub fn draw_slot(
        &self,
        idx: u8,
        cell: Option<&T>,
        matches: impl Fn(&T) -> bool,
        text_renderer: &mut TextRenderer,
        bg_text: &mut RegularBackground,
    ) {
        match cell {
            Some(cell) if matches(cell) => self.draw_text(idx, cell, text_renderer, bg_text),
            Some(_) => self.draw_err(idx, text_renderer, bg_text),
            None => self.clear(idx, text_renderer),
        }
    }

    pub fn draw_text(
        &self,
        idx: u8,
        cell: &T,
        text_renderer: &mut TextRenderer,
        bg_text: &mut RegularBackground,
    ) {
        let at = self.cell_origin(idx);
        let mut buf = ArrayString::<8>::new();
        for row in self.rows {
            if let RowKind::Text(cell_text) = &row.kind {
                buf.clear();
                cell_text(cell, idx, &mut buf);
                text_renderer.draw_text(
                    buf.as_bytes(),
                    FONT,
                    bg_text,
                    at + self.cross * row.content_off,
                    &CELL_FORMAT,
                );
            }
        }
    }

    pub fn draw_err(
        &self,
        idx: u8,
        text_renderer: &mut TextRenderer,
        bg_text: &mut RegularBackground,
    ) {
        self.clear(idx, text_renderer);
        text_renderer.draw_text(b"err", FONT, bg_text, self.cell_origin(idx), &CELL_FORMAT);
    }

    pub fn clear(&self, idx: u8, text_renderer: &mut TextRenderer) {
        text_renderer.clear_pixel_rect(self.cell_origin(idx), self.clear_size.x, self.clear_size.y);
    }

    pub fn draw_sprites(&self, idx: u8, cell: &T, frame: &mut GraphicsFrame) {
        let at = self.cell_origin(idx) + vec2(ARROW_X_NUDGE, 0);
        for row in self.rows {
            if let RowKind::Sprite(sprite) = &row.kind
                && let Some(tag) = sprite(cell)
            {
                tag.sprite(0).show(at + self.cross * row.content_off, frame);
            }
        }
    }
}

pub fn text_index<T>(_: &T, idx: u8, out: &mut ArrayString<8>) {
    push_num(out, idx as i32);
}

pub const fn text_row<T, Id: Copy>(
    id: Id,
    content_off: i32,
    help: &'static [u8],
    buttons: &'static [u8],
    text: fn(&T, u8, &mut ArrayString<8>),
) -> RowDesc<T, Id> {
    RowDesc {
        id,
        content_off,
        cursor_off: content_off - 2,
        tag: &sprites::HIGHLIGHT_SFX_SQR_INSTRUMENT,
        tooltip: TooltipText { help, buttons },
        kind: RowKind::Text(text),
        input: RowInput::Edit,
    }
}

pub const fn sprite_row<T, Id: Copy>(
    id: Id,
    content_off: i32,
    help: &'static [u8],
    sprite: fn(&T) -> Option<&'static Tag>,
) -> RowDesc<T, Id> {
    RowDesc {
        id,
        content_off,
        cursor_off: content_off - 1,
        tag: &sprites::HIGHLIGHT_SFX_SQR_ARROW,
        tooltip: TooltipText {
            help,
            buttons: BUTTONS_ARROW,
        },
        kind: RowKind::Sprite(sprite),
        input: RowInput::Edit,
    }
}

pub fn for_each_visible(first_visible: u8, visible: u8, mut draw: impl FnMut(u8)) {
    for offset in 0..visible {
        if let Some(idx) = first_visible.checked_add(offset) {
            draw(idx);
        }
    }
}
