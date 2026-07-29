# PSG song and sound-effect format (`.pmus` / `.psfx`) — version 1

A song or sound effect is a single [RON](https://github.com/ron-rs/ron) document. Structural data
(settings, instruments, wave tables) is typed RON; pattern rows are compact tracker-style strings.
`//` line comments are allowed anywhere.

The two documents are different formats and use different extensions:

| Extension | Contents | Included with |
|---|---|---|
| `.pmus` | a song: patterns across all four channels, with an order list and looping | `include_pmus!` |
| `.psfx` | a sound effect: one channel, one list of rows, played once | `include_psfx!` |

Files are parsed at compile time; errors are reported as compile errors, including passing a file
to the wrong macro.

## What the PSG is

The GBA inherits the Game Boy's *Programmable Sound Generator*: four fixed-function channels that
you configure by writing registers. It cannot play samples (that is what agb's software mixer is
for) — instead each channel generates one specific kind of tone, and music is made by switching
notes and settings on those four voices, up to 60 times a second.

| Channel | Column         | Produces                                     | Key knobs                                         |
|---------|----------------|----------------------------------------------|---------------------------------------------------|
| 1       | square + sweep | a square wave                                | duty cycle, volume envelope, hardware pitch sweep |
| 2       | square         | a square wave                                | duty cycle, volume envelope                       |
| 3       | wave           | a looping 32-step, 4-bit waveform you supply | the wave table, 5 volume levels                   |
| 4       | noise          | pseudo-random noise (a shift register)       | 60 pitches, volume envelope, LFSR width           |

Things that surprise people coming from a sample tracker:

- **There are exactly four voices, and they are not interchangeable.** A bass line written for the
  wave channel cannot be moved to the noise channel. Column 3 of every pattern is *the* wave
  channel; if it is busy, there is no second one.
- **Volume is 4-bit.** Square and noise have 16 levels (0–15); the wave channel has five
  (mute/25/50/75/100%). Fades are chunky by nature.
- **Panning is one bit per side.** A channel can be left, right, both, or neither — there is no
  fine pan position.
- **Pitch is a period register, not a frequency.** The hardware takes an 11-bit number; the
  relationship to pitch is non-linear, which is why the pitch-slide effects behave differently high
  and low in the range (see [Effects](#effects)).
- **Square and noise have a hardware volume envelope** that runs on its own once a note starts —
  it costs nothing and is the cheapest way to get a decay. The wave channel has none.
- **Writing a volume to a square or noise channel does nothing until the channel is retriggered.**
  This one leaks into the format; see [Hardware notes](#hardware-notes-encoded-in-the-format).

## Song document (`.pmus`)

```ron
(
    version: 1,
    frames_per_tick: 2.5,       // frames (1/59.73 s) per tick; fractional allowed (8.8 fixed point)
    ticks_per_row: 6,           // initial speed; changeable mid-song with the Rxx effect
    order: [0, 1, 0],           // pattern play order (indices into `patterns`)
    loop_to: Some(1),           // order index to jump back to after the last entry; None = stop
    instruments: {
        "lead": Square(
            duty: D50,                  // D12_5 | D25 | D50 | D75 (default D50)
            envelope: (12, Down, 3),    // initial volume 0-15, Up|Down, step time 0-7 (0 = static)
            sweep: Some((3, Down, 2)),  // channel-1 column only: time 0-7, Up|Down, shift 0-7
            length: Some(32),           // optional length counter 1-64; None = play until note off
        ),
        "organ": Wave(table: "bass", volume: V100),  // V0 | V25 | V50 | V75 | V100
        "hat": Noise(envelope: (10, Down, 1), lfsr: Long),  // Short (7-bit) | Long (15-bit, default)
    },
    waves: {
        "bass": "0123456789ABCDEFFEDCBA9876543210",  // exactly 32 hex nibbles, played left to right
    },
    patterns: [
        [   // pattern 0; channels are: 1 square+sweep | 2 square | 3 wave | 4 noise
            "C-4 lead --- | --- ..  --- | C-3 organ --- | C-5 hat ---",
            "--- ..   A47 | D#4 lead V23 | --- ..    --- | --- ..  ---",
            "skip 12",  // 12 empty rows
        ],
    ],
)
```

## Sound effect document (`.psfx`)

Same shape, but with a `channel` field instead of `order`/`loop_to`, and `rows` (one cell each)
instead of `patterns`. An SFX plays once through its rows, then ends.

```ron
(
    version: 1,
    frames_per_tick: 1.0,
    ticks_per_row: 2,
    channel: Noise,             // SquareSweep | Square | Wave | Noise
    instruments: { "boom": Noise(envelope: (15, Down, 2), lfsr: Long) },
    rows: [
        "C-4 boom ---",
        "C-3 ..   D08",
        "off ..   ---",
    ],
)
```

While a sound effect plays it *steals* its channel from the music: the song keeps advancing
silently there and becomes audible again at its next note-on after the effect ends.

## Instruments

`instruments` is a name → definition map. A name is referenced from pattern cells; the definitions
are shared by every pattern in the document. Which columns an instrument may appear in is decided
by its kind:

| Kind                     | Valid columns | Notes                                            |
|--------------------------|---------------|--------------------------------------------------|
| `Square` without `sweep` | 1 or 2        |                                                  |
| `Square` with `sweep`    | 1 only        | the sweep unit exists only on hardware channel 1 |
| `Wave`                   | 3 only        |                                                  |
| `Noise`                  | 4 only        |                                                  |

Putting an instrument in the wrong column is a compile error.

**`envelope: (volume, direction, step)`** — the hardware volume envelope on square and noise.
`volume` 0–15 is the level the note starts at, `direction` is `Up` or `Down`, and `step` 0–7 is how
many 1/64ths of a second pass between each ±1 volume change. `step: 0` disables the envelope, so
the note holds at `volume` until something stops it. `(15, Down, 2)` is a note that starts loud and
fades to silence over 15 × 2/64 ≈ 0.47 s, for free, without any pattern data.

**`length: Some(n)`** (square only) — a hardware note-length counter in 1/256 s units, so
`length: Some(32)` is an eighth of a second. When it expires the channel silences itself. `None`
means the note plays until a note off, a note cut, or the envelope fading to 0.

**`sweep: Some((time, direction, shift))`** (column 1 only) — the hardware pitch sweep. Every
`time` × 1/128 s the hardware adjusts the period by 1/2^`shift` of itself, `Up` (rising pitch) or
`Down`. `time: 0` disables the sweep. Because the sweep unit rewrites the pitch register behind the
player's back, a sweep instrument cannot be combined with the `Uxx`/`Dxx`/`Txx` pitch effects — the
parser rejects that combination.

**`lfsr`** (noise only) — `Long` (15-bit) is normal hiss, good for snares and cymbals; `Short`
(7-bit) repeats after 127 steps, giving a buzzy, metallic, almost pitched tone.

**`volume: V0 … V100`** (wave only) — see below.

## Wave tables

The wave channel does not have a fixed waveform: it plays a **32-sample, 4-bit table that you
supply**, looping continuously at the note's frequency. That table is the instrument's timbre. It is
the only channel on the PSG that can sound like something other than a square or noise, so it is
usually where basses, plucks, organs and lead tones live.

### Writing a table

Each entry in `waves` is a name → **exactly 32 hexadecimal digits**. Each digit is one sample, an
amplitude from `0` (minimum) to `F` (maximum), and they are played **left to right**, then repeat.
Case does not matter.

```ron
waves: {
    "tri": "0123456789ABCDEFFEDCBA9876543210",
}
```

Reading that as a graph: the amplitude ramps 0 → F over the first 16 samples and back down over the
last 16 — a triangle wave. Some other useful starting points:

| Table                              | Sounds like                                                |
|------------------------------------|------------------------------------------------------------|
| `FFFFFFFFFFFFFFFF0000000000000000` | a 50% square (same tone as channels 1–2, but on channel 3) |
| `FFFFFFFF000000000000000000000000` | a 25% pulse — thinner, more nasal                          |
| `00112233445566778899AABBCCDDEEFF` | a sawtooth — bright and buzzy, good for basses and leads   |
| `0123456789ABCDEFFEDCBA9876543210` | a triangle — soft, flute/NES-bass-like                     |
| `89ACDEEFFFEEDCA97653211000112356` | an approximated sine — the mellowest tone available        |
| `0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F0F` | a square four octaves up (32 samples = 16 cycles) — harsh  |

Practical advice:

- **Only the shape matters, not where you start.** Rotating a table left or right changes the
  starting phase, not the timbre.
- **Repetition raises the pitch.** A shape that repeats *n* times inside the 32 samples sounds
  log2(*n*) octaves above the written note. That is a legitimate trick for getting bright timbres,
  but it means an accidentally doubled pattern plays an octave high.
- **Centre the table around 7–8.** The output is unipolar, so a table sitting mostly near `0` or
  `F` wastes headroom and adds a DC step (an audible click) when the note starts.
- **Four bits is 16 levels.** Smooth shapes are approximate; sines and triangles quantise audibly,
  which is part of the sound.
- Internally each *pair* of digits becomes one byte of wave RAM, the left digit being the high
  nibble (which plays first). That only matters if you are generating these files from a tool.

### Using a table

A wave table is not playable on its own — it is referenced by a `Wave` instrument, which also
carries the channel's volume:

```ron
instruments: {
    "bass": Wave(table: "tri", volume: V100),
    "pad":  Wave(table: "tri", volume: V50),   // same timbre, quieter
},
```

`volume` is one of `V0` (mute), `V25`, `V50`, `V75`, `V100` — the hardware's only output levels for
this channel. There is no wave envelope, so a volume of `V0` is silence rather than a fade-in.

Two or more instruments may share one table, and one instrument may be used in as many cells as you
like.

### Limits and behaviour worth knowing

- **Only one table is resident at a time.** The hardware has a single 16-byte wave RAM. The player
  uploads a table when a note triggers whose instrument uses a different one than is currently
  loaded; it writes to the inactive bank and flips, so there is no glitch, but alternating between
  two tables on consecutive notes means an upload on every note. It is cheap (8 halfword writes),
  not free.
- **A wave-channel sound effect overwrites wave RAM.** When the effect ends, the music's table is
  re-uploaded at its next note-on — so the music's wave voice stays silent until then, rather than
  playing the effect's timbre.
- **The wave channel plays an octave below the square channels for the same period value.** The
  player compensates by looking notes up an octave higher, so a `C-4` in column 3 sounds at the
  same pitch as a `C-4` in column 1. The cost is the top octave: wave notes may not exceed `B-8`.
- **Volume changes are free here.** Unlike square and noise, writing a new wave volume takes effect
  immediately, so `Mxx` and `Sxx` fades on column 3 do not retrigger the note or click.
- **`waves` may hold up to 255 tables**, and unused tables are still embedded in the ROM.

## Patterns

### The vocabulary

A song is not one long list of notes. It is built from three layers:

- A **row** is one moment in time across all four channels: what each channel should do now.
- A **pattern** is a block of rows — the format's unit of reuse, typically a bar or four. Patterns
  are stored in the `patterns` list and referred to by their index: the first is pattern 0.
- The **order** is the sequence in which patterns are played: `order: [0, 1, 0, 2]` plays pattern
  0, then 1, then 0 again, then 2. This is how a chorus is written once and played three times.
  Positions in the order list are also 0-based, so that example has order positions 0–3.

When the last order entry finishes, `loop_to: Some(n)` restarts from order position `n`
(`Some(0)` = loop the whole song); `loop_to: None` stops playback and silences the channels.

```
order:      [ 0,      1,      0,      2   ]        loop_to: Some(1)
              │       │       │       │             ↑
patterns:  pattern 0  │   pattern 0   │             └── after pattern 2, continue here
                  pattern 1       pattern 2
```

Each pattern may be a different length (1–256 rows). A document may hold up to 256 patterns.

### How fast it plays

Three units stack up:

- a **frame** is one video frame, ≈ 1/59.73 s — the player is stepped once per frame and uses no
  hardware timers;
- a **tick** is `frames_per_tick` frames — this is the resolution at which continuous effects like
  vibrato and slides update. Fractional values are allowed (`2.5`) and accumulate exactly;
- a **row** is `ticks_per_row` ticks.

So `frames_per_tick: 2.5` with `ticks_per_row: 6` gives 15 frames per row ≈ 0.251 s per row — at
four rows to the beat, 60 BPM. Both values can be changed mid-song with the `Fxx` and `Rxx`
effects.

Within a row, **tick 0 is when the row happens**: notes start, instruments change, and the
one-shot part of each effect is applied. Ticks 1 and up run the continuous part of the effect
(arpeggio steps, slide increments, vibrato). A row with `ticks_per_row: 1` therefore gets no
continuous effects at all.

Hardware is written once per frame, at the end of it. With `frames_per_tick` below 1 several ticks
run inside one frame, and only the state they leave behind is played — a note started and cut
within the same frame is never heard. Ticks stay in time either way; it is only the audible
resolution that is capped at one change per frame.

### Row strings

A row is 4 cells separated by `|` — one per channel, always in the fixed order square+sweep,
square, wave, noise (an SFX row has 1 cell and no `|`). Whitespace between tokens is free, so
columns can be padded for readability. A cell is always three whitespace-separated tokens, even
when empty:

```
C#4 lead A37
 │   │    └──── effect:     arpeggio, alternating +3 and +7 semitones
 │   └───────── instrument: the instrument named "lead"
 └───────────── note:       C sharp, octave 4

--- ..  ---
 │   │   └───── empty: no effect on this row
 │   └───────── same instrument as this channel used last
 └───────────── empty: nothing happens to this channel on this row
```

The two placeholders are not the same idea, and neither of them means "repeat":

- **`---` is an empty cell** — it says nothing happens here. It is not an instruction to sustain.
  A note left ringing through a row of `---` keeps sounding because nothing touched the channel;
  an effect, by contrast, *stops* at an empty cell, because effects last exactly one row. Same
  token, opposite result — the difference comes from what the field means, not from `---`.
- **`..` is a value, not a blank** — "the instrument this channel last named". Every row resolves
  to some instrument, so there is no empty instrument cell; `..` is the format's only carry-over
  token. It is also not interchangeable with retyping the name: naming an instrument resets the
  channel's volume, `..` preserves it (see [What carries over](#what-carries-over-between-rows)).

The differing widths are inherited from classic trackers, where the note field is three characters
and the instrument field two — which is also why columns line up in a monospaced editor.

- **note**: `C-2` … `B-9` (sharps as `C#4`; no flats), `---` = empty, `off` = note off.
  Semitone index 1–96, C-2 lowest. Constraints per channel:
  - wave column: max `B-8` (the wave channel plays an octave lower; the player offsets internally)
  - noise column: max `B-6` (indexes the 60-entry noise pitch table; higher = brighter noise)
- **instrument**: an instrument name, or `..` = the one this channel used last.
- **effect**: `---` = empty, or a letter followed by two hex digits (see [Effects](#effects)).

A line consisting of `skip N` (N ≥ 1) stands in for N rows that are empty on all four channels. It
is purely shorthand — those rows still take time and still count toward the 256-row limit.

```ron
[
    "C-4 lead --- | --- ..  --- | C-3 organ --- | C-5 hat ---",
    "skip 3",       // three empty rows: the notes above simply keep ringing
    "off ..   --- | --- ..  --- | --- ..    --- | C-5 hat ---",
]
```

### What carries over between rows

A pattern is a list of *changes*, not a list of what is sounding. Nothing needs to be repeated:

- **A note keeps playing** until something ends it: an `off` in that column, a note cut (`Cxx`), a
  new note, the instrument's hardware envelope reaching 0, or its `length` counter expiring.
- **The instrument is remembered per channel.** `..` reuses the last instrument named in that
  column — including across pattern and order boundaries.
- **Naming an instrument resets that channel's volume** to the instrument's initial envelope volume
  (or the wave instrument's `volume`). Using `..` keeps the channel's current volume, so a fade
  built with `Sxx` survives further notes only if those notes use `..`.
- **Effects do not carry over.** An effect applies to the row it is written on and stops at the
  next row. A four-row fade needs `Sxx` on all four rows; a vibrato that should last a whole note
  needs `Vxy` repeated on every row of that note.
- **Panning, duty and volume changes persist** until changed again — they are channel state, not
  effects.
- **Volume and duty changes on a silent channel are remembered, not played.** `Mxx`, `Sxx` and
  `Wxx` before a channel's first note, or after an `off`/`Cxx`, only update the stored value; it is
  applied when that channel next plays a note. They never restart a note that has already ended, so
  presetting a volume at the top of a pattern is safe.

One consequence worth planning for: when a vibrato stops, the pitch is left wherever the vibrato
wave happened to be, until the next note-on resets it. Either carry the vibrato through to the next
note or accept a slight detune on the tail.

### Jumping around

Two effects change what plays next; both take effect at the *end* of the row they appear on, so the
rest of that row plays normally.

- `Bxx` — **position jump**: continue at order position `xx` (hex), row 0. The target is checked at
  compile time against the length of `order`.
- `Kxx` — **pattern break**: finish this pattern now and continue at row `xx` of the *next* order
  entry. If that pattern is shorter than `xx`, playback starts at its last row.

Use at most one jump per row: if several channels in the same row carry a jump, only one of them
takes effect.

Neither is allowed in a sound effect (there is no order list to jump within) — the parser rejects
them.

### A worked example

```ron
(
    version: 1,
    frames_per_tick: 2.5,       // \ 4 ticks × 2.5 frames = 10 frames per row,
    ticks_per_row: 4,           // / ≈ 0.167 s — 90 BPM at four rows to the beat
    order: [0, 0, 1],
    loop_to: Some(0),           // after pattern 1, go back to the start and repeat forever
    instruments: {
        "lead": Square(duty: D50, envelope: (13, Down, 2)),   // plucky: fades over ~0.4 s
        "bass": Wave(table: "tri", volume: V100),
        "hat":  Noise(envelope: (8, Down, 1)),                // short tick
    },
    waves: { "tri": "0123456789ABCDEFFEDCBA9876543210" },
    patterns: [
        [   //  ch1 (square+sweep)  ch2 (square)      ch3 (wave)        ch4 (noise)
            "--- ..   ---     |    C-5 lead ---  |   C-3 bass ---  |   C-5 hat ---",
            "--- ..   ---     |    --- ..   ---  |   --- ..   ---  |   --- ..  ---",
            "--- ..   ---     |    E-5 ..   ---  |   --- ..   ---  |   C-5 ..  ---",
            "--- ..   ---     |    --- ..   ---  |   --- ..   ---  |   --- ..  ---",
            "--- ..   ---     |    G-5 ..   V23  |   G-2 ..   ---  |   C-5 ..  ---",
            "skip 2",
            "--- ..   ---     |    off ..   ---  |   off ..   ---  |   --- ..  ---",
        ],
        [   // same idea, but ends early and jumps back rather than playing all 8 rows
            "--- ..   ---     |    A-5 lead ---  |   A-2 bass ---  |   C-5 hat ---",
            "--- ..   ---     |    --- ..   ---  |   --- ..   ---  |   --- ..  ---",
            "--- ..   ---     |    off ..   K00  |   off ..   ---  |   --- ..  ---",
            "skip 5",         // never reached: K00 ends the pattern on the row above
        ],
    ],
)
```

Reading pattern 0 row by row: row 0 starts a lead note, a bass note and a hat, all with named
instruments (so all three reset to their instruments' volumes). Row 1 is empty in the file, but
all three notes are still sounding. Row 2 plays a new lead note and a new hat using `..` — same
instruments as before. Row 4 adds `V23` to the lead: a vibrato at speed 2, depth 3, for that row
only. Rows 5–6 are the `skip 2`. Row 7 releases the lead and bass; the hat's envelope has long
since faded it out on its own.

## Effects

One effect per cell. The parameter is always two hexadecimal digits, and always required.

| Effect | Meaning |
|---|---|
| `Axy` | Arpeggio: rotate base note, +x, +y semitones each tick |
| `Uxx` / `Dxx` | Pitch slide up / down by xx period units per tick |
| `Txx` | Tone portamento: slide toward this cell's note at xx period units per tick |
| `Vxy` | Vibrato: speed x, depth y |
| `Sxx` | Volume slide: signed byte added to volume each row (square/noise: retriggers, see below) |
| `Cxx` | Note cut: silence the channel at tick xx |
| `Qxx` | Note delay: trigger this cell's note at tick xx instead of tick 0 |
| `Bxx` | Position jump: continue at order index xx after this row |
| `Kxx` | Pattern break: continue at row xx of the next order entry after this row |
| `Rxx` | Set ticks per row (1-31) |
| `Fxx` | Set frames per tick, 4.4 fixed point (e.g. `F28` = 2.5) |
| `Wxx` | Set square duty 0-3 (12.5/25/50/75%) |
| `Pxx` | Panning: bit 1 = left on, bit 0 = right on (`P03` = centre, `P02` = left only) |
| `Mxx` | Set volume: 0-15 on square/noise, 0-4 on wave (0 mute, 1 = 25% … 4 = 100%) |

**Pitch effects.** `Axy` cycles the pitch every tick — base note on ticks 0, 3, 6…, +x on ticks 1,
4…, +y on ticks 2, 5… — the classic chiptune substitute for a chord; on the noise channel it steps
the noise pitch instead. `Uxx`, `Dxx` and `Txx` operate on the raw 11-bit period register, not on
semitones, so the same `xx` is a wide interval low in the range and a very small one high up;
expect to tune slide values by ear per octave. `Txx` slides toward the note written in the same
cell without retriggering it, so a legato line is `C-4 lead ---` then `E-4 .. T08`. `Vxy` moves the
pitch around the note along a 32-step sine: `x` is how many steps per tick (speed) and `y` scales
the swing to at most ±2×`y` period units. Period-based effects (`Uxx`, `Dxx`, `Txx`, `Vxy`) are
rejected on the noise column, and all three slides are rejected on a sweep instrument.

**Volume.** `Mxx` sets the level outright; `Sxx` adds a *signed* value once per row — `S01` is +1
per row, `SFF` is −1, `SFE` is −2 — clamped to the channel's range. Both must be repeated on every
row of a fade. On square and noise both retrigger the channel (see below); on wave they are silent
and free. A volume equal to the channel's current one is a no-op — in particular, it does not
retrigger, so `Mxx` cannot be used to restart a note the hardware envelope has faded out.

**Timing and shape.** `Rxx` and `Fxx` change the tempo from that row onward, and are hex like every
other parameter (`R0C` = 12 ticks per row). `Cxx` silences the channel at tick `xx` of the row
(`C00` = immediately), which is how you get notes shorter than a row. `Qxx` does the opposite,
holding this cell's note back until tick `xx`; if `xx` is not less than `ticks_per_row` the row ends
first and the note never sounds.

**Channel settings.** `Wxx` changes the square duty cycle mid-note (it retriggers, since duty shares
a register with the envelope) and is only valid on the square columns. `Pxx` sets the channel's
left/right enables and persists until changed; `P00` mutes the channel on both sides.

Runtime limits for sound effects: `Pxx` (panning) inside an SFX is ignored in
version 1 — the music's panning stays in control of `SOUNDCNT_L`.

## Hardware notes encoded in the format

- **Square/noise volume quirk**: the GBA only applies envelope-register (volume) writes when a
  channel is retriggered. `Mxx`, `Sxx` and `Wxx` on square/noise therefore retrigger the channel,
  resetting its phase (a small click) — but only while a note is actually sounding; on a silent
  channel they are just stored for the next note-on. Sustained fades are better authored with
  hardware envelopes (`envelope: (v, Down, step)` with step > 0), which cost nothing.
- **Sweep vs pitch slides**: an instrument with `sweep` must not be combined with `Uxx`/`Dxx`/`Txx`
  on the same note — the hardware sweep unit rewrites the frequency internally and the two fight.
  The parser rejects this combination, but only within a single pattern: a pattern that inherits a
  sweep instrument from an earlier pattern through `..` is not checked, because file order is not
  play order. Repeat the instrument name in that pattern to get the check.
- Wave tables are 32 4-bit samples; the high nibble of each byte pair plays first.
- Only one wave table is in wave RAM at a time; switching tables costs an upload at the next
  note-on, and a wave-channel SFX forces the music's table to be re-uploaded when it ends.

## Limits

| Thing | Limit |
|---|---|
| `frames_per_tick` | > 0 and ≤ 255 (8.8 fixed point) |
| `ticks_per_row` | 1–31 |
| patterns per document | 256 |
| rows per pattern (including `skip`ped rows) | 256 |
| instruments | 255 |
| wave tables | 255 |
| notes | `C-2`–`B-9`; `B-8` max on wave, `B-6` max on noise |

## Versioning

`version` is required and must be `1`. Parsers reject other versions. Future revisions bump the
version and document changes here.
