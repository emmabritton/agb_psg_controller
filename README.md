# AGB PSG Controller

Music and sound-effect player for the GBA's PSG (square, sweep, wave and noise channels).

- Songs (`.pmus`) and sound effects (`.psfx`) are written in a human-readable
  RON text format (see [`docs/format-spec.md`](docs/format-spec.md)) and parsed
  **at compile time** by `include_pmus!` / `include_psfx!` into ROM data — no
  runtime parsing, no allocation for playback same as the tracker with xm files.
- The player is stepped once per frame and uses no hardware timers, so it
  coexists with agb's mixer (timers 0/1) and flash storage.
- Sound effects temporarily steal their PSG channel from the music and hand it
  back when done.
- Tracker-style effects: arpeggio, pitch slides, tone portamento, vibrato,
  volume slides, note cut/delay, pattern jumps, tempo changes, duty, panning.

## Usage

```toml
eb_agb_psg_controller = "0.26.0"
```

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

## Tracker Program for GBA

Controls are generally explained on screen, except for:

- Press Start to hear the SFX/Song
- Press Select to swap instrument set in Song editor
- When deleting the B button must be held for 1s
- When filling the wave table, each arrow generates:
  - Left: Sine wave (approx)
  - Up: Sawtooth
  - Down: Triangle
  - Right: Random

[SFX Menu](https://raw.githubusercontent.com/emmabritton/agb_psg_controller/refs/heads/main/.github/ss_menu_sfx.png)
[Square SFX](https://raw.githubusercontent.com/emmabritton/agb_psg_controller/refs/heads/main/.github/ss_sfx_square.png)
[Wave SFX](https://raw.githubusercontent.com/emmabritton/agb_psg_controller/refs/heads/main/.github/ss_sfx_wave.png)

### Getting SFX from the tracker

Use the CLI tool to play and extract SFX from GBA .sav files

Playing an SFX from slot 0: 
`psg_cli play tracker.sav 0`

Saving an SFX to a file:
`psg_cli extract tracker.sav 0`
or save it as a .wav file
`psg_cli extract --wav tracker.sav 0`