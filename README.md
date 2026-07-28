# AGB PSG Controller

Music and sound-effect player for the Game Boy Advance's Programmable Sound
Generator (the four "Game Boy" channels: square with sweep, square, wave,
noise), built to pair with [agb](https://github.com/agbrs/agb) the same way
`agb-tracker` pairs with agb's software mixer.

- Songs (`.pmus`) and sound effects (`.psfx`) are written in a human-readable
  RON text format (see [`docs/format-spec.md`](docs/format-spec.md)) and parsed
  **at compile time** by `include_pmus!` / `include_psfx!` into ROM data — no
  runtime parsing, no allocation for playback.
- The player is stepped once per frame and uses **no hardware timers**, so it
  coexists with agb's mixer (timers 0/1) and flash storage.
- Sound effects temporarily steal their PSG channel from the music and hand it
  back when done.
- Tracker-style effects: arpeggio, pitch slides, tone portamento, vibrato,
  volume slides, note cut/delay, pattern jumps, tempo changes, duty, panning.

## Usage

```rust
use eb_agb_psg_controller::{include_pmus, include_psfx, Player, Sfx, Track};

static SONG: Track = include_pmus!("assets/song.pmus");
static JUMP: Sfx = include_psfx!("assets/jump.psfx");

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    let mut player = Player::new(&SONG);

    loop {
        // on input: player.play_sfx(&JUMP);
        player.frame(); // once per frame
        // ...display frame commit...
    }
}
```

For sound effects without music, use `Player::sfx_only()` — a song can be
started (or replaced) at any time with `player.play_song(&SONG)` and stopped
with `player.stop_song()`.

A complete example with button-triggered SFX is in
`eb_agb_psg_controller/examples/basic.rs` — run it with
`cd eb_agb_psg_controller && cargo run --example basic` (requires
[mgba](https://mgba.io) as the cargo runner, already configured).

## Song format at a glance

```ron
(
    version: 1,
    frames_per_tick: 2.5,
    ticks_per_row: 4,
    order: [0, 1],
    loop_to: Some(0),
    instruments: {
        "lead": Square(duty: D50, envelope: (13, Down, 2)),
        "bass": Wave(table: "tri", volume: V100),
        "hat":  Noise(envelope: (8, Down, 1)),
    },
    waves: { "tri": "159D159D26AE26AE37BF37BF48C048C0" },
    patterns: [
        [ // square+sweep | square | wave | noise
            "--- ..  --- | C-5 lead --- | C-3 bass --- | C-5 hat ---",
            "E-4 lead A47 | --- ..  --- | --- ..  --- | --- ..  ---",
            "skip 12",
        ],
    ],
)
```

## Workspace

| Crate                   | Purpose                                                                                                                                       |
|-------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------|
| `eb_agb_psg_controller` | The runtime player (`no_std`, GBA)                                                                                                            |
| `eb_agb_psg_macros`     | `include_pmus!` / `include_psfx!` proc-macros                                                                                                 |
| `eb_agb_psg_interop`    | Shared ROM data model + song lowering/validation                                                                                              |
| `eb_agb_psg_format`     | The `.pmus` / `.psfx` file structs + RON read/write — depend on this alone to build song tools (editors, converters) with no GBA dependencies |

Host-side tests (format round-trips, pitch tables) run from the workspace
root: `cargo test -p eb_agb_psg_interop --features parse`. GBA tests run
from inside `eb_agb_psg_controller/` under mgba: `cargo test`.
