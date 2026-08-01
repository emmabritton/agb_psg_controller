//! The save and load screens share this module's [`SlotGrid`]: a two-column,
//! five-row window onto the save slots, each slot showing its name and a
//! "<kind>: <n> INST <n> ROWS" summary next to a main (save/load) button and
//! a delete button. The grid owns the cursor, row scrolling with its up/down
//! indicators, the hold-B slot delete and the legend strip at the bottom;
//! the two scenes supply the background, what A on the main button does and
//! where backing out goes.

mod load;
mod save;

pub use load::LoadScene;
pub use save::SaveScene;

use crate::save_controller::{Metadata, SaveController, SaveKind};
use crate::scenes::sfx_editor::DELETE_HOLD_FRAMES;
use crate::scenes::sfx_editor::common::text::push_num;
use crate::sound_controller::{SoundController, SoundEffect};
use agb::display::object::Tag;
use agb::display::tile_data::TileData;
use agb::display::tiled::RegularBackground;
use agb::display::{GraphicsFrame, Priority};
use agb::fixnum::{Vector2D, vec2};
use agb::input::{Button, ButtonController};
use agb::save::Slot;
use agb_eb_ext::backgrounds::{create_filled_bg, create_text_bg};
use agb_eb_ext::direction::Direction;
use agb_eb_ext::gfx::{ShowAllTag, ShowTag};
use arrayvec::ArrayString;
use gba_agb_font_renderer::prelude::{PrintableFont, TextRenderer};
use gba_agb_font_renderer::{TextAlign, TextFormat, TextOverflow};
use resources::{FONT_WHITE, sprites};

const SLOT_XS: [i32; 2] = [6, 124];
const SLOT_YS: [i32; 5] = [30, 52, 74, 96, 118];
const COLS: usize = SLOT_XS.len();
const VISIBLE_ROWS: usize = SLOT_YS.len();

const MAIN_BUTTON: Vector2D<i32> = vec2(86, 3);
const DELETE_BUTTON: Vector2D<i32> = vec2(99, 3);
const LINE_NAME: Vector2D<i32> = vec2(3, 2);
const LINE_INFO: Vector2D<i32> = vec2(3, 9);

const INDICATOR_UP_POS: Vector2D<i32> = vec2(228, 4);
const INDICATOR_DOWN_POS: Vector2D<i32> = vec2(228, 140);
const TOOLTIP_POS: Vector2D<i32> = vec2(1, 154);

//frame count that counts as starting a delete
//and so, if released before 60 does nothing
//instead of going back
const BACK_TAP_FRAMES: u8 = 15;

static FONT: &PrintableFont = &FONT_WHITE;

static FORMAT_SLOT: TextFormat = TextFormat {
    overflow: TextOverflow::Cutoff(79),
    align: TextAlign::Left,
    clear: (80, 8),
};
static FORMAT_TOOLTIP_LEFT: TextFormat = TextFormat {
    overflow: TextOverflow::Nothing,
    align: TextAlign::Left,
    clear: (240, 8),
};
static FORMAT_LEGEND_RIGHT: TextFormat = TextFormat {
    overflow: TextOverflow::Nothing,
    align: TextAlign::Right(238),
    clear: (0, 0),
};

enum GridEvent {
    None,
    Activate(usize),
    Back,
}

struct SlotGrid {
    bg: RegularBackground,
    bg_text: RegularBackground,
    text_renderer: TextRenderer,
    slot: usize,
    on_delete: bool,
    scroll_row: usize,
    slot_count: usize,
    main_legend: &'static [u8],
    delete_hold: u8,
    hold_used: bool,
}

impl SlotGrid {
    fn new(
        bg: &'static TileData,
        main_legend: &'static [u8],
        save_controller: &SaveController,
    ) -> Self {
        let mut grid = Self {
            bg: create_filled_bg(bg, Priority::P3),
            bg_text: create_text_bg(),
            text_renderer: TextRenderer::default(),
            slot: 0,
            on_delete: false,
            scroll_row: 0,
            slot_count: save_controller.slot_count(),
            main_legend,
            delete_hold: 0,
            hold_used: false,
        };
        grid.redraw_slots(save_controller);
        grid.draw_tooltip();
        grid
    }

    fn screen_pos(&self, slot: usize) -> Vector2D<i32> {
        let row = (slot >> 1) - self.scroll_row;
        vec2(SLOT_XS[slot & 1], SLOT_YS[row])
    }

    fn cursor_pos(&self) -> Vector2D<i32> {
        self.screen_pos(self.slot)
            + if self.on_delete {
                DELETE_BUTTON
            } else {
                MAIN_BUTTON
            }
    }

    fn redraw_slot(&mut self, slot: usize, save_controller: &SaveController) {
        let row = slot >> 1;
        if row < self.scroll_row || row >= self.scroll_row + VISIBLE_ROWS {
            return;
        }
        let pos = self.screen_pos(slot);
        let mut info = ArrayString::<24>::new();
        let name: &[u8] = match save_controller.slot(slot) {
            Slot::Empty => b"EMPTY",
            Slot::Corrupted => b"CORRUPTED",
            Slot::Valid(meta) => {
                info = info_line(meta);
                if meta.name.is_empty() {
                    b"UNNAMED"
                } else {
                    &meta.name
                }
            }
        };
        self.text_renderer
            .draw_text(name, FONT, &mut self.bg_text, pos + LINE_NAME, &FORMAT_SLOT);
        self.text_renderer.draw_text(
            info.as_bytes(),
            FONT,
            &mut self.bg_text,
            pos + LINE_INFO,
            &FORMAT_SLOT,
        );
    }

    fn redraw_slots(&mut self, save_controller: &SaveController) {
        let first = self.scroll_row << 1;
        let last = (first + VISIBLE_ROWS * COLS).min(self.slot_count);
        for slot in first..last {
            self.redraw_slot(slot, save_controller);
        }
    }

    fn draw_tooltip(&mut self) {
        let right: &[u8] = if self.on_delete {
            b"DELETE: HOLD B"
        } else {
            self.main_legend
        };
        self.text_renderer.draw_text(
            b"BACK: B",
            FONT,
            &mut self.bg_text,
            TOOLTIP_POS,
            &FORMAT_TOOLTIP_LEFT,
        );
        self.text_renderer.draw_text(
            right,
            FONT,
            &mut self.bg_text,
            TOOLTIP_POS,
            &FORMAT_LEGEND_RIGHT,
        );
    }

    fn nav(&mut self, direction: Direction) -> bool {
        let (slot, on_delete) = (self.slot, self.on_delete);
        match direction {
            Direction::Left => {
                if self.on_delete {
                    self.on_delete = false;
                } else if slot & 1 == 1 {
                    self.slot -= 1;
                    self.on_delete = true;
                }
            }
            Direction::Right => {
                if !self.on_delete {
                    self.on_delete = true;
                } else if slot & 1 == 0 && slot + 1 < self.slot_count {
                    self.slot += 1;
                    self.on_delete = false;
                }
            }
            Direction::Up => {
                if slot >= COLS {
                    self.slot -= COLS;
                }
            }
            Direction::Down => {
                if slot + COLS < self.slot_count {
                    self.slot += COLS;
                }
            }
        }
        self.slot != slot || self.on_delete != on_delete
    }

    fn scroll_into_view(&mut self, save_controller: &SaveController) {
        let row = self.slot >> 1;
        self.scroll_row = if row < self.scroll_row {
            row
        } else if row >= self.scroll_row + VISIBLE_ROWS {
            row + 1 - VISIBLE_ROWS
        } else {
            return;
        };
        self.redraw_slots(save_controller);
    }

    fn update(
        &mut self,
        button_controller: &ButtonController,
        sound_controller: &mut SoundController,
        save_controller: &mut SaveController,
    ) -> GridEvent {
        if button_controller.is_pressed(Button::B) {
            self.delete_hold = self.delete_hold.saturating_add(1);
            if self.delete_hold >= DELETE_HOLD_FRAMES {
                // One hold, one action; keeping B down starts the next hold.
                self.delete_hold = 0;
                self.hold_used = true;
                if self.on_delete
                    && !matches!(save_controller.slot(self.slot), Slot::Empty)
                    && save_controller.delete_save(self.slot)
                {
                    sound_controller.play_sfx(SoundEffect::CursorSelect);
                    self.redraw_slot(self.slot, save_controller);
                }
            }
            return GridEvent::None;
        }
        if button_controller.is_just_released(Button::B) {
            let tapped = self.delete_hold < BACK_TAP_FRAMES && !self.hold_used;
            self.delete_hold = 0;
            self.hold_used = false;
            if tapped {
                sound_controller.play_sfx(SoundEffect::CursorCancel);
                return GridEvent::Back;
            }
            return GridEvent::None;
        }
        if let Some(direction) = Direction::from_recent_input(button_controller) {
            let was_delete = self.on_delete;
            if self.nav(direction) {
                sound_controller.play_sfx(SoundEffect::CursorMove);
                self.scroll_into_view(save_controller);
                if self.on_delete != was_delete {
                    self.draw_tooltip();
                }
            }
            return GridEvent::None;
        }
        if button_controller.is_just_pressed(Button::A) && !self.on_delete {
            return GridEvent::Activate(self.slot);
        }
        GridEvent::None
    }

    fn show(&self, frame: &mut GraphicsFrame) {
        self.bg.show(frame);
        self.bg_text.show(frame);
        let tag: &Tag = if self.on_delete {
            &sprites::BUTTON_DELETE_ACTIVE
        } else {
            &sprites::BUTTON_SAVE_ACTIVE
        };
        tag.show_all(self.cursor_pos(), frame);
        if self.scroll_row > 0 {
            sprites::INDICATOR_UP.show(0, INDICATOR_UP_POS, frame);
        }
        if (self.scroll_row + VISIBLE_ROWS) << 1 < self.slot_count {
            sprites::INDICATOR_DOWN.show(0, INDICATOR_DOWN_POS, frame);
        }
    }
}

fn info_line(meta: &Metadata) -> ArrayString<24> {
    let mut out = ArrayString::new();
    let _ = out.try_push_str(match meta.kind {
        SaveKind::Sfx => "SFX: ",
        SaveKind::Track => "TRACK: ",
    });
    push_num(&mut out, meta.instruments as i32);
    let _ = out.try_push_str(" INST ");
    push_num(&mut out, meta.rows as i32);
    let _ = out.try_push_str(" ROWS");
    out
}
