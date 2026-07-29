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
    let mut player = Player::new();
    player.play_song(&SONG);

    loop {
        // on input: player.play_sfx(&JUMP);
        player.frame(); // once per frame
        // ...display frame commit...
    }
}
```

A complete example with button-triggered SFX is in
`eb_agb_psg_controller/examples/basic.rs` — run it with
`cd eb_agb_psg_controller && cargo run --example basic` (requires
[mgba](https://mgba.io) as the cargo runner, already configured).

## Runtime-built songs and effects (`dynamic` feature)

`Track` and `Sfx` are plain structs whose slices are `Cow<'static, [_]>`, so
they can also be built at runtime — for an on-device editor, say. The normal
`play_song`/`play_sfx` borrow their data for the player's lifetime, which a
struct that owns both cannot satisfy. Enabling the `dynamic` feature adds
`Rc`-taking counterparts:

```toml
eb_agb_psg_controller = { version = "0.25", features = ["dynamic"] }
```

```rust
struct Tracker {
    player: Player<'static>,   // owns everything it plays
    song: Rc<Track>,
    sfx: Rc<Sfx>,
}

self.player.play_sfx_shared(Rc::clone(&self.sfx));
self.player.play_song_shared(Rc::clone(&self.song));

// Editing is a rebuild; the old data stays alive until playback releases it.
self.sfx = Rc::new(editor.build());
```

The feature is off by default and purely additive: without it the player holds
a bare `&T` exactly as before, so the `include_pmus!`/`include_psfx!` path keeps
its zero-allocation playback.

Hand-built data skips the `.pmus`/`.psfx` parser's validation, so a few
invariants become the caller's job: `frames_per_tick` must be greater than zero
(zero spins forever in the frame step), `ticks_per_row` at least 1, `rows` /
`pattern_data` non-empty, notes in `1..=96`, and wave-channel notes no higher
than 84 (the player adds a 12-semitone offset before the period lookup).

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
