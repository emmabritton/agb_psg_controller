use agb::display::tiled::RegularBackground;
use agb::fixnum::vec2;
use gba_agb_font_renderer::prelude::{PrintableFont, TextRenderer};
use gba_agb_font_renderer::{TextAlign, TextFormat, TextOverflow};
use resources::FONT_WHITE;

static FONT: &PrintableFont = &FONT_WHITE;

static FORMAT_HELP: TextFormat = TextFormat {
    overflow: TextOverflow::Nothing,
    align: TextAlign::Left,
    clear: (240, 16),
};

static FORMAT_BUTTONS: TextFormat = TextFormat {
    overflow: TextOverflow::Nothing,
    align: TextAlign::Right(238),
    clear: (0, 0),
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TooltipText {
    pub help: &'static [u8],
    pub buttons: &'static [u8],
}

pub const BUTTONS_NUM: &[u8] = b"DEC: L | INC: R";
pub const BUTTONS_ARROW: &[u8] = b"TOGGLE: L";

pub fn draw_tooltip(
    text: TooltipText,
    text_renderer: &mut TextRenderer,
    bg: &mut RegularBackground,
) {
    text_renderer.draw_text(text.help, FONT, bg, vec2(1, 1), &FORMAT_HELP);
    text_renderer.draw_text(text.buttons, FONT, bg, vec2(1, 1), &FORMAT_BUTTONS);
}
