use crate::UserStr;
use agb::display::object::Tag;
use agb::display::tile_data::TileData;
use agb::display::tiled::RegularBackground;
use agb::display::{GraphicsFrame, Priority};
use agb::fixnum::{Vector2D, vec2};
use agb::input::{Button, ButtonController};
use agb_eb_ext::backgrounds::create_filled_bg;
use agb_eb_ext::direction::Direction;
use agb_eb_ext::gfx::{ShowAllTag, ShowTag};
use gba_agb_font_renderer::prelude::{PrintableFont, TextRenderer};
use gba_agb_font_renderer::{TextAlign, TextFormat, TextOverflow};
use resources::{FONT_BLACK, sprites};

pub enum KeyboardResult {
    Cancel,
    NewValue(UserStr),
}

static KEYS: [&'static [u8]; 4] = [
    &[b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'0'],
    &[b'Q', b'W', b'E', b'R', b'T', b'Y', b'U', b'I', b'O', b'P'],
    &[b'A', b'S', b'D', b'F', b'G', b'H', b'J', b'K', b'L'],
    &[b'Z', b'X', b'C', b'V', b'B', b'N', b'M', b'(', b')'],
];

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Cursor {
    Cancel,
    Space,
    Submit,
    Backspace,
    Key(u8),
}

/// Adjacent key boxes share a 1px border, so the 16px highlight steps by 15.
const KEY_STEP: i32 = 15;

const POS_CANCEL: Vector2D<i32> = vec2(0, 147);
const POS_SPACE: Vector2D<i32> = vec2(40, 147);
const POS_SUBMIT: Vector2D<i32> = vec2(199, 147);
const POS_BACKSPACE: Vector2D<i32> = vec2(222, 76);
const POS_KEY_ROWS: [Vector2D<i32>; 4] = [vec2(44, 82), vec2(44, 97), vec2(52, 112), vec2(52, 127)];
const POS_FIELD: Vector2D<i32> = vec2(65, 34);

static TAG_CANCEL: &Tag = &sprites::KEYBOARD_OTHER_ACTIVE;
static TAG_SAVE: &Tag = &sprites::KEYBOARD_OTHER_ACTIVE;
static TAG_BACKSPACE: &Tag = &sprites::KEYBOARD_DEL_ACTIVE;
static TAG_KEY: &Tag = &sprites::KEYBOARD_KEY_ACTIVE;
static TAG_SPACE: &Tag = &sprites::KEYBOARD_SPACE_ACTIVE;

static FONT: &PrintableFont = &FONT_BLACK;
static FORMAT_FIELD: TextFormat = TextFormat {
    overflow: TextOverflow::Nothing,
    align: TextAlign::Left,
    clear: (112, 8),
};

fn key_row_col(chr: u8) -> (usize, usize) {
    for (row, keys) in KEYS.iter().enumerate() {
        if let Some(col) = keys.iter().position(|&key| key == chr) {
            return (row, col);
        }
    }
    (0, 0)
}

fn key_pos(chr: u8) -> Vector2D<i32> {
    let (row, col) = key_row_col(chr);
    POS_KEY_ROWS[row] + vec2(col as i32 * KEY_STEP, 0)
}

fn key_moved(dir: Direction, chr: u8) -> Cursor {
    let (row, col) = key_row_col(chr);
    let key = |row: usize, col: usize| Cursor::Key(KEYS[row][col]);
    match dir {
        Direction::Left if col > 0 => key(row, col - 1),
        Direction::Right if col + 1 < KEYS[row].len() => key(row, col + 1),
        Direction::Right if row == 0 || row == 1 => Cursor::Backspace,
        Direction::Up => match row {
            0 => Cursor::Key(chr),
            2 => key(1, col + 1),
            _ => key(row - 1, col),
        },
        Direction::Down => match row {
            0 => key(1, col),
            1 => key(2, col.saturating_sub(1)),
            2 => key(3, col),
            _ => Cursor::Space,
        },
        _ => Cursor::Key(chr),
    }
}

fn moved(cursor: Cursor, dir: Direction) -> Cursor {
    match (dir, cursor) {
        (Direction::Left, Cursor::Backspace) => Cursor::Key(b'0'),
        (Direction::Down, Cursor::Backspace) => Cursor::Submit,
        (Direction::Up, Cursor::Cancel) => Cursor::Key(b'Z'),
        (Direction::Right, Cursor::Cancel) => Cursor::Space,
        (Direction::Up, Cursor::Space) => Cursor::Key(b'B'),
        (Direction::Left, Cursor::Space) => Cursor::Cancel,
        (Direction::Right, Cursor::Space) => Cursor::Submit,
        (Direction::Up, Cursor::Submit) => Cursor::Backspace,
        (Direction::Left, Cursor::Submit) => Cursor::Space,
        (dir, Cursor::Key(chr)) => key_moved(dir, chr),
        _ => cursor,
    }
}

pub struct Keyboard {
    text: UserStr,
    cursor: Cursor,
    bg: RegularBackground,
    initial_update: bool,
    max_len: u8,
}

impl Keyboard {
    pub fn new(bg: &'static TileData, initial_text: UserStr, max_len: u8) -> Self {
        Self {
            text: initial_text,
            max_len,
            cursor: Cursor::Key(b'Q'),
            bg: create_filled_bg(bg, Priority::P2),
            initial_update: true,
        }
    }

    fn add_char(&mut self, chr: u8) -> bool {
        if self.text.len() < self.max_len as usize {
            self.text.push(chr);
            true
        } else {
            false
        }
    }

    fn del_char(&mut self) -> bool {
        self.text.pop().is_some()
    }

    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.bg.show(frame);
        match self.cursor {
            Cursor::Cancel => TAG_CANCEL.show_all(POS_CANCEL, frame),
            Cursor::Space => {
                TAG_SPACE.show(0, POS_SPACE, frame);
                for i in 1..=4 {
                    TAG_SPACE.show(1, POS_SPACE + vec2(32 * i, 0), frame);
                }
                TAG_SPACE.show(2, POS_SPACE + vec2(152, 0), frame);
            }
            Cursor::Submit => TAG_SAVE.show_all(POS_SUBMIT, frame),
            Cursor::Backspace => TAG_BACKSPACE.show(0, POS_BACKSPACE, frame),
            Cursor::Key(chr) => TAG_KEY.show(0, key_pos(chr), frame),
        }
    }

    pub fn update(
        &mut self,
        button_controller: &ButtonController,
        text_renderer: &mut TextRenderer,
        bg_text: &mut RegularBackground,
    ) -> Option<KeyboardResult> {
        let mut redraw = self.initial_update;
        if self.initial_update {
            self.initial_update = false;
            text_renderer.reset(false);
        }

        if let Some(dir) = Direction::from_recent_input(button_controller) {
            self.cursor = moved(self.cursor, dir);
        } else if button_controller.is_just_pressed(Button::A) {
            match self.cursor {
                Cursor::Cancel => return Some(KeyboardResult::Cancel),
                Cursor::Submit => return Some(KeyboardResult::NewValue(self.text.clone())),
                Cursor::Space => redraw |= self.add_char(b' '),
                Cursor::Backspace => redraw |= self.del_char(),
                Cursor::Key(chr) => redraw |= self.add_char(chr),
            }
        } else if button_controller.is_just_pressed(Button::B) {
            redraw |= self.del_char();
        } else if button_controller.is_just_pressed(Button::Start) {
            self.cursor = Cursor::Submit;
        }

        if redraw {
            text_renderer.draw_text(&self.text, FONT, bg_text, POS_FIELD, &FORMAT_FIELD);
        }
        None
    }
}
