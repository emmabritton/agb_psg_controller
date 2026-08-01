use agb::fixnum::{Vector2D, vec2};

pub const TABLE_ORIGIN: Vector2D<i32> = vec2(27, 38);
pub const ROWS_ORIGIN: Vector2D<i32> = vec2(27, 107);
pub const CELL_WIDTH: u16 = 14;
pub const COL_STRIDE: i32 = CELL_WIDTH as i32 + 1;
pub const VISIBLE_COLUMNS: u8 = 14;
pub const CURSOR_X_NUDGE: i32 = -1;
pub const ARROW_X_NUDGE: i32 = 4;

pub const POS_NAME: Vector2D<i32> = vec2(27, 19);
pub const POS_FRAMES: Vector2D<i32> = vec2(142, 19);
pub const POS_TICKS: Vector2D<i32> = vec2(192, 19);
