# Seq95 – step sequencer for the Launchpad Pro Mk3

Seq95 is a Launchpad95-style hardware step sequencer that runs on the Launchpad
itself. Unlike the original Launchpad95 (an Ableton Live remote script that edits
Live's clips), Seq95 needs no computer software: it has its own clock and sends
MIDI notes straight out of the Launchpad.

- 5 independent tracks, each drum or melodic, each on its own MIDI channel
- up to 64 steps per track, with a loop range and its own step rate
  (1/4 … 1/32, including triplets)
- per-step note probability, velocity, gate length, octave and mute
- live transposition (arrow buttons, an on-grid keyboard, or incoming MIDI notes)
- 16 scales with a selectable root note
- internal clock with tap tempo, or follow external MIDI clock; optional MIDI clock out
- 8 project slots, saved separately from everything else
- keeps playing in the background when you switch back to another mode

> **Status.** Seq95 currently ships inside CoreFW (this repository's custom
> firmware), which replaces Novation's firmware while it is installed. The
> version that keeps Novation's own firmware (including its 4-track sequencer)
> and adds Seq95 on top is planned but not built yet. See
> [Keeping Novation's firmware](#keeping-novations-firmware) for what it needs.
> None of this has been tested on hardware yet.

## Entering and leaving

Press **Custom** to switch to Seq95 from any mode (Performance, Programmer,
DAW). Press **Custom** again to return to the mode you came from. Playback
keeps running while Seq95 is not on screen.

## Layout

Button names follow the labels printed on the Launchpad Pro Mk3.

```
 Shift  Left  Right  Session  Note  Chord  Custom  Seq  Projects
┌─────┬─────────────────────────────────────────────┬──────────┐
│ Up  │                                             │ Scale    │
│ Down│                                             │ Prob.    │
│Clear│                                             │ Rate     │
│ Dup │              8 x 8 grid                     │ Random   │
│Quant│       (drum or melodic editor,              │ Length   │
│Fixed│        or an overlay, see below)            │ Octave   │
│Play │                                             │ Velocity │
│ Rec │                                             │ Notes    │
└─────┴─────────────────────────────────────────────┴──────────┘
        Rec Arm  Mute  Solo  Volume  Pan  Sends  Device  Stop Clip
        Track 1  Track 2  Track 3  Track 4  Track 5    -    -    -
```

### Buttons available in both track types

| Button | Action |
| --- | --- |
| **Track 1–5** (bottom row) | Select a track |
| **Mute** (hold) + Track | Mute / unmute the track |
| **Solo** (hold) + Track | Solo / unsolo the track |
| **Clear** (hold) + Track | Erase all steps of the track |
| **Play** | Start / stop. With external clock it arms and waits for clock |
| **Left / Right** | Previous / next page of steps |
| **Shift** + Left / Right | Tempo −1 / +1 BPM |
| **Up / Down** | Transpose melodic tracks ±1 semitone |
| **Shift** + Up / Down | Transpose ±1 octave |
| **Record** (hold) | Transposition keyboard on the grid (see below) |
| **Duplicate** | Copy the loop after itself and double the loop length |
| **Quantise** or **Rate** | Cycle the track's step rate |
| **Fixed Length** (hold) + page pad | Set the loop end |
| **Clear** (hold) + page pad | Erase that page (8 steps) |
| **Shift** (hold) | Track and global settings (see below) |
| **Scale** | Scale and root note overlay |
| **Projects** | Project load/save overlay |
| **Random** | Randomise the current lane (melodic) or voice (drum) within the loop |

**Pages and loops.** Each track has 8 pages of 8 steps (64 steps). The page
pads show the loop range dimly, the page you're viewing brightly, and the
playing page in white. Tap a page to view it. Hold one page and tap another to
set the loop to that range.

### Melodic tracks (Launchpad95 "melodic step sequencer")

The top 7 rows are 8 steps (columns) by 7 values (rows). The bottom grid row
holds the page pads. The right column picks what the 7 rows edit:

| Right column | Rows mean | Tap a pad to… |
| --- | --- | --- |
| **Notes** | 7 scale degrees, root at the bottom | turn the step on at that note; tap the lit pad again to turn it off |
| **Octave** | −3 … +3 octaves (middle row = 0) | set the octave |
| **Velocity** | 7 levels, 16 … 127 | set the velocity |
| **Length** | ¼, ½, ¾, 1, 2, 4, 8 steps | set the gate length |
| **Probability** | 12 %, 25 %, 37 %, 50 %, 62 %, 75 %, 100 % | set how likely the step plays |

Hold **Mute** and tap a step to mute it (shown red). Hold **Clear** and tap a
step to reset it. Notes follow the project scale and root, plus the live
transposition.

### Drum tracks (Launchpad95 "drum step sequencer")

```
 rows 8-5:  32 steps of the selected voice (two views of 32 = 64 steps)
 rows 4-1:  left 4x4 = 16 drum voices   right top 2 rows = 8 page pads
```

- Tap a **voice pad** to select it. It also plays the sound, using the pad's velocity.
- Tap a **step** to toggle a hit. New hits take their velocity from how hard you hit the pad.
- Hold **Velocity** (right column) and tap a step to step its velocity down.
- Hold **Probability** and tap a step to step its probability down. Steps under 100 % show in cyan.
- Hold **Mute** and tap a step to mute it, or tap a voice to mute that voice.
- Hold **Clear** and tap a step or voice to erase it.

Voices play chromatically from the track's base note: 36 (C1) by default,
which matches General MIDI drums.

### Settings overlay (hold Shift)

| Row | Pads |
| --- | --- |
| 8–7 | MIDI channel 1–16 of the current track |
| 6 | 1 = Drum, 2 = Melodic · 4 = output on USB MIDI port, 5 = output on USB DAW port, 6 = TRS MIDI out (not available yet) |
| 5 | Step rate: 1/4, 1/4T, 1/8, 1/8T, 1/16, 1/16T, 1/32, 1/32T |
| 4 | 1 = internal clock, 2 = external clock · 4 = send MIDI clock · 8 = transpose from MIDI channel 16 on/off |
| 3 | Melodic: root octave 1–8 · Drum: base note C0 / C1 / C2 / C3 |
| 2 | 1 = tap tempo · 5 = −10 BPM, 6 = −1, 7 = +1, 8 = +10 |
| 1 | Tempo presets 80 … 150 BPM |

### Scale overlay (Scale button)

Rows 8–7 are a piano for the root note. Rows 5–4 pick one of 16 scales: Major,
Minor, Dorian, Phrygian, Lydian, Mixolydian, Locrian, Harmonic minor, Melodic
minor, Major pentatonic, Minor pentatonic, Blues, Whole tone, Diminished,
Hungarian minor, Chromatic.

### Live transposition

- **Up / Down** (with Shift for octaves) while playing.
- Hold **Record**: the grid becomes three one-octave piano keyboards. Rows 1–2
  give 0 … +12, rows 3–4 give −12 … 0, rows 5–6 give +12 … +24. The top-right
  pad resets to 0.
- Play notes into the Launchpad's USB MIDI port on **channel 16**. Middle C (60)
  means no transposition. These notes are consumed by Seq95 and not passed to
  other modes. Turn this off in the settings overlay.

Transposition applies to melodic tracks only; drums never transpose.

### Projects (Projects button)

| Row | Pads |
| --- | --- |
| 8 | Load slot 1–8. Green = saved, white = loaded |
| 7 | Save to slot 1–8 |
| 5, pad 8 | New, empty project |

A pad flashes white when the action worked and red when it failed. Projects
hold every track, step, tempo, scale and clock setting.

Saving takes a moment, and the sequencer pauses briefly while flash is
written. Projects live in a dedicated area of the Launchpad's 16 MB flash chip
(offset `0xF00000`). Seq95 refuses to overwrite any part of it that contains
data it didn't write itself.

## MIDI output

Each track sends notes on its own channel. The defaults are track 1 = drums on
channel 10, and tracks 2–5 = melodic on channels 1–4. Output goes to the
Launchpad's USB **MIDI** port (the second USB port, "PRO MK3 (MIDI)"). It can
also go to the DAW port per track.

The 3.5 mm TRS MIDI out isn't driven by CoreFW yet, because its pins on the Mk3
haven't been identified. The setting is already stored per track, so it will
start working once the output is implemented.

## Keeping Novation's firmware

The requested end state is Novation's own firmware, with its Session, Note,
Chord, Custom and 4-track Sequencer modes, plus Seq95 on the Custom button.
That can only be done by patching Novation's firmware image, which is how
CoreFW started (`launchpad-injection-cfw`; see this repository's history before
commit `1cb2e3e`). That project already found, for stock firmware **v385**:

- an app-loop call site to hook a tick function into
- the incoming-MIDI handler
- the button/pad event mailbox (`0x2004fa6c`)
- `set LED` / `push LEDs`, `send MIDI` (port mask), and the NAFFS file system
  (open/read/write/seek/delete)
- a free area at `0x08076000`–`0x08080000` for injected code

Its hooks replaced Novation's main event loop (the "Protest" patch NOPs the
loop body), so Novation's modes stopped working. Keeping them needs one more
piece of reverse engineering in the v385 binary: the point where the stock
loop dispatches button events. A hook there would pass events to Novation's
code normally, and divert them to Seq95 while Seq95 is active. LEDs need the
same treatment, so Seq95's drawing wins while it is on screen.

The `seq95` crate is written for that port. It is `no_std`, has no hardware
dependencies and needs about 16 KB of RAM. It can be compiled as a static
library and linked into the injected image. Going through Novation's firmware
would also give TRS MIDI out for free, because the stock send-MIDI function
takes a port mask.

What's needed to finish that version:

1. The stock `launchpadpromk3-firmware-385.syx` (from Novation Components).
   It can't be committed here, and this build environment can't download it.
2. Reverse engineering the v385 event dispatch and LED ownership, then a
   new patch set.
3. Testing on hardware. Patching a binary blind can't be verified any other way.
