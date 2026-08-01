use crate::scenes::sfx_editor::channels::starter_instrument;
use crate::scenes::sfx_editor::wave;
use crate::scenes::{SceneAction, SceneCtx, clicked};
use crate::sfx_doc::SfxDocument;
use crate::sfx_template::SfxTemplate;
use crate::sound_controller::SoundEffect;
use agb::display::object::Sprite;
use agb::display::tiled::RegularBackground;
use agb::display::{GraphicsFrame, Priority};
use agb::fixnum::{Vector2D, vec2};
use agb::input::Button;
use agb::rng::RandomNumberGenerator;
use agb_eb_ext::backgrounds::create_filled_bg;
use agb_eb_ext::direction::Direction;
use agb_eb_ext::gfx::ShowSprite;
use eb_agb_psg_controller::SfxChannel;
use resources::{bg, sprites};

enum ButtonAction {
    Template(SfxTemplate),
    Blank(SfxChannel),
}

struct MenuButton {
    x: i32,
    y: i32,
    action: ButtonAction,
}

const fn btn(x: i32, y: i32, template: SfxTemplate) -> MenuButton {
    MenuButton {
        x,
        y,
        action: ButtonAction::Template(template),
    }
}

const fn blank(x: i32, y: i32, channel: SfxChannel) -> MenuButton {
    MenuButton {
        x,
        y,
        action: ButtonAction::Blank(channel),
    }
}

#[rustfmt::skip]
const ROWS: &[&[MenuButton]] = &[
    &[btn(12, 41, SfxTemplate::Click), btn(88, 41, SfxTemplate::Positive), btn(164, 41, SfxTemplate::Negative)],
    &[btn(12, 59, SfxTemplate::Beep), btn(88, 59, SfxTemplate::Laser), btn(164, 59, SfxTemplate::Explosion)],
    &[btn(12, 77, SfxTemplate::Teleport), btn(88, 77, SfxTemplate::Powerup), btn(164, 77, SfxTemplate::Shoot)],
    &[btn(12, 95, SfxTemplate::Hurt), btn(88, 95, SfxTemplate::Magic), btn(164, 95, SfxTemplate::Alarm)],
    &[btn(12, 113, SfxTemplate::Pickup), btn(88, 113, SfxTemplate::Jump), btn(164, 113, SfxTemplate::Footstep)],
    &[blank(10, 141, SfxChannel::Square), blank(67, 141, SfxChannel::SquareSweep), blank(125, 141, SfxChannel::Wave), blank(182, 141, SfxChannel::Noise)],
];

impl MenuButton {
    fn centre_x(&self) -> i32 {
        self.x
            + match self.action {
                ButtonAction::Template(_) => 31,
                ButtonAction::Blank(_) => 23,
            }
    }

    fn sprite(&self) -> &'static Sprite {
        match self.action {
            ButtonAction::Template(_) => sprites::SFX_MENU_BUTTON_LARGE.sprite(0),
            ButtonAction::Blank(_) => sprites::SFX_MENU_BUTTON_SMALL.sprite(0),
        }
    }

    fn build_doc(&self, rng: &mut RandomNumberGenerator) -> (SfxDocument, Option<SfxTemplate>) {
        match self.action {
            ButtonAction::Template(template) => (template.to_doc(rng, None), Some(template)),
            ButtonAction::Blank(channel) => {
                let mut doc = SfxDocument::new();
                doc.set_channel(channel);
                if channel == SfxChannel::Wave {
                    // A wave instrument is invalid while there are no wave
                    // tables, so the wave editor's seeding adds both.
                    wave::ensure_seeded(&mut doc);
                } else {
                    doc.add_instrument(starter_instrument(channel));
                }
                (doc, None)
            }
        }
    }
}

/// Index of the button in `row` whose centre is nearest `x` (first wins ties).
fn nearest_col(row: &[MenuButton], x: i32) -> usize {
    let mut best = 0;
    let mut best_dist = i32::MAX;
    for (i, button) in row.iter().enumerate() {
        let dist = (button.centre_x() - x).abs();
        if dist < best_dist {
            best = i;
            best_dist = dist;
        }
    }
    best
}

pub struct SfxMenuScene {
    bg: RegularBackground,
    cursor: Vector2D<usize>,
}

impl SfxMenuScene {
    pub fn new() -> Self {
        Self {
            bg: create_filled_bg(&bg::sfx_menu, Priority::P3),
            cursor: Vector2D::new(0, 0),
        }
    }

    pub fn show(&self, frame: &mut GraphicsFrame) {
        self.bg.show(frame);
        let button = &ROWS[self.cursor.y][self.cursor.x];
        button.sprite().show(vec2(button.x, button.y), frame);
    }

    pub fn update(&mut self, ctx: &mut SceneCtx) -> Option<SceneAction> {
        // A/B first: a select pressed on the same frame as a direction (or
        // during key repeat) must not be swallowed by the cursor move.
        if clicked(ctx, Button::A, SoundEffect::CursorSelect) {
            let mut rng = ctx.seed_gen.create_rng();
            let (doc, template) = ROWS[self.cursor.y][self.cursor.x].build_doc(&mut rng);
            return Some(SceneAction::OpenSfx(doc, template, None));
        } else if clicked(ctx, Button::B, SoundEffect::CursorCancel) {
            return Some(SceneAction::OpenMenu);
        } else if let Some(dir) = Direction::from_recent_input(ctx.button_controller) {
            let (col, row) = (self.cursor.x, self.cursor.y);
            let moved = match dir {
                Direction::Up | Direction::Down => {
                    let new_row = if dir == Direction::Up {
                        row.checked_sub(1)
                    } else {
                        (row + 1 < ROWS.len()).then_some(row + 1)
                    };
                    new_row.map(|r| (nearest_col(ROWS[r], ROWS[row][col].centre_x()), r))
                }
                Direction::Left => (col > 0).then(|| (col - 1, row)),
                Direction::Right => (col + 1 < ROWS[row].len()).then(|| (col + 1, row)),
            };
            if let Some((col, row)) = moved {
                self.cursor = Vector2D::new(col, row);
                ctx.sound_controller.play_sfx(SoundEffect::CursorMove);
            }
        }
        None
    }
}
