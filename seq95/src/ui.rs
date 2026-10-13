// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 launchpad-Pro-Mod contributors

//! Surface layout, button handling and LED rendering.
//!
//! See `docs/seq95.md` for the user-facing description of every control.

use crate::engine::MidiOut;
use crate::model::*;
use crate::{PROJECT_SLOTS, Seq95, Storage};

pub const LED_COUNT: usize = 110;

// Launchpad Pro Mk3 programmer-mode indexes.
const SHIFT: u8 = 90;
const LEFT: u8 = 91;
const RIGHT: u8 = 92;
const PROJECTS: u8 = 98;
const UP: u8 = 80;
const DOWN: u8 = 70;
const CLEAR: u8 = 60;
const DUPLICATE: u8 = 50;
const QUANTISE: u8 = 40;
const FIXED_LENGTH: u8 = 30;
const PLAY: u8 = 20;
const RECORD: u8 = 10;
const MUTE: u8 = 102;
const SOLO: u8 = 103;
const CUSTOM: u8 = 96;

// Right column (scene buttons), top to bottom.
const SCENE_SCALE: u8 = 89;
const SCENE_PROBABILITY: u8 = 79;
const SCENE_RATE: u8 = 69;
const SCENE_RANDOM: u8 = 59;
const SCENE_LENGTH: u8 = 49;
const SCENE_OCTAVE: u8 = 39;
const SCENE_VELOCITY: u8 = 29;
const SCENE_NOTES: u8 = 19;

const TRACK_COLOURS: [u32; TRACKS] = [0xff2020, 0x20ff40, 0x2070ff, 0xffa000, 0xc030ff];
const WHITE: u32 = 0xffffff;
const PLAYHEAD: u32 = 0x303030;
const RED: u32 = 0xff0000;
const AMBER: u32 = 0xff8000;
const CYAN: u32 = 0x00c0ff;
const MAGENTA: u32 = 0xff00a0;
const YELLOW: u32 = 0xffe000;
const RATE_COLOURS: [u32; 8] = [
    0x0040ff, 0x6020ff, 0x00c0ff, 0x40a0ff, 0x00ff60, 0x80ff40, 0xffc000, 0xff8040,
];
const MESSAGE_US: u64 = 600_000;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Lane {
    Notes,
    Octave,
    Velocity,
    Length,
    Probability,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum Overlay {
    None,
    Scale,
    Projects,
}

pub(crate) struct Ui {
    pub(crate) track: usize,
    pub(crate) now_us: u64,
    lane: Lane,
    view_block: [u8; TRACKS],
    voice: [u8; TRACKS],
    overlay: Overlay,
    shift: bool,
    record: bool,
    clear: bool,
    mute: bool,
    solo: bool,
    fixed_length: bool,
    hold_velocity: bool,
    hold_probability: bool,
    held_block: Option<u8>,
    preview: Option<(u8, u8, u8)>,
    current_slot: Option<u8>,
    slot_used: [bool; PROJECT_SLOTS as usize],
    message: Option<(u8, u32, u64)>,
}

impl Ui {
    pub(crate) const fn new() -> Self {
        Self {
            track: 0,
            now_us: 0,
            lane: Lane::Notes,
            view_block: [0; TRACKS],
            voice: [0; TRACKS],
            overlay: Overlay::None,
            shift: false,
            record: false,
            clear: false,
            mute: false,
            solo: false,
            fixed_length: false,
            hold_velocity: false,
            hold_probability: false,
            held_block: None,
            preview: None,
            current_slot: None,
            slot_used: [false; PROJECT_SLOTS as usize],
            message: None,
        }
    }

    pub(crate) fn release_all(&mut self) {
        self.shift = false;
        self.record = false;
        self.clear = false;
        self.mute = false;
        self.solo = false;
        self.fixed_length = false;
        self.hold_velocity = false;
        self.hold_probability = false;
        self.held_block = None;
    }

    pub(crate) fn stop_preview(&mut self, project: &Project, out: &mut impl MidiOut) {
        if let Some((track, channel, note)) = self.preview.take() {
            out.send(project.tracks[track as usize].ports, &[0x80 | channel, note, 0]);
        }
    }

    fn flash_message(&mut self, index: u8, colour: u32) {
        self.message = Some((index, colour, self.now_us + MESSAGE_US));
    }
}

fn grid(row: u8, col: u8) -> u8 {
    row * 10 + col
}

/// Row and column (1..8) of a main grid pad.
fn grid_pos(index: u8) -> Option<(u8, u8)> {
    let row = index / 10;
    let col = index % 10;
    if (1..=8).contains(&row) && (1..=8).contains(&col) {
        Some((row, col))
    } else {
        None
    }
}

fn dim(colour: u32, num: u32, den: u32) -> u32 {
    let r = ((colour >> 16) & 0xff) * num / den;
    let g = ((colour >> 8) & 0xff) * num / den;
    let b = (colour & 0xff) * num / den;
    (r << 16) | (g << 8) | b
}

fn level_from_velocity(velocity: u8) -> u8 {
    if velocity == 0 {
        LEVELS - 1
    } else {
        ((velocity as u16 * LEVELS as u16) / 128).min(LEVELS as u16 - 1) as u8
    }
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

pub(crate) fn button(
    seq: &mut Seq95,
    index: u8,
    pressed: bool,
    velocity: u8,
    out: &mut impl MidiOut,
    storage: &mut impl Storage,
) {
    let kind = seq.project.tracks[seq.ui.track].kind;

    // Hold modifiers.
    match index {
        SHIFT => {
            seq.ui.shift = pressed;
            return;
        }
        RECORD => {
            seq.ui.record = pressed;
            return;
        }
        CLEAR => {
            seq.ui.clear = pressed;
            return;
        }
        MUTE => {
            seq.ui.mute = pressed;
            return;
        }
        SOLO => {
            seq.ui.solo = pressed;
            return;
        }
        FIXED_LENGTH => {
            seq.ui.fixed_length = pressed;
            return;
        }
        SCENE_VELOCITY if kind == TrackKind::Drum => {
            seq.ui.hold_velocity = pressed;
            return;
        }
        SCENE_PROBABILITY if kind == TrackKind::Drum => {
            seq.ui.hold_probability = pressed;
            return;
        }
        _ => {}
    }

    if let Some((row, col)) = grid_pos(index) {
        grid_event(seq, row, col, pressed, velocity, out, storage);
        return;
    }

    if !pressed {
        return;
    }

    let ui = &mut seq.ui;
    let project = &mut seq.project;
    let track = &mut project.tracks[ui.track];

    match index {
        PLAY => seq.engine.toggle(project, out),
        LEFT | RIGHT => {
            let up = index == RIGHT;
            if ui.shift {
                let bpm = if up { project.bpm + 1 } else { project.bpm.saturating_sub(1) };
                project.bpm = bpm.clamp(MIN_BPM, MAX_BPM);
            } else {
                let block = &mut ui.view_block[ui.track];
                let stride = if track.kind == TrackKind::Drum { 4 } else { 1 };
                *block = if up {
                    (*block + stride).min(BLOCKS as u8 - 1)
                } else {
                    block.saturating_sub(stride)
                };
            }
        }
        UP | DOWN => {
            let amount: i16 = if ui.shift { 12 } else { 1 };
            let delta = if index == UP { amount } else { -amount };
            project.transpose = (project.transpose as i16 + delta)
                .clamp(-(MAX_TRANSPOSE as i16), MAX_TRANSPOSE as i16) as i8;
        }
        DUPLICATE => track.duplicate_loop(),
        QUANTISE | SCENE_RATE => track.rate = (track.rate + 1) % RATE_PULSES.len() as u8,
        PROJECTS => {
            ui.overlay = if ui.overlay == Overlay::Projects {
                Overlay::None
            } else {
                for slot in 0..PROJECT_SLOTS {
                    ui.slot_used[slot as usize] = storage.slot_used(slot);
                }
                Overlay::Projects
            };
        }
        SCENE_SCALE => {
            ui.overlay = if ui.overlay == Overlay::Scale {
                Overlay::None
            } else {
                Overlay::Scale
            };
        }
        SCENE_RANDOM => randomise(seq),
        SCENE_NOTES if track.kind == TrackKind::Melodic => ui.lane = Lane::Notes,
        SCENE_OCTAVE if track.kind == TrackKind::Melodic => ui.lane = Lane::Octave,
        SCENE_VELOCITY if track.kind == TrackKind::Melodic => ui.lane = Lane::Velocity,
        SCENE_LENGTH if track.kind == TrackKind::Melodic => ui.lane = Lane::Length,
        SCENE_PROBABILITY if track.kind == TrackKind::Melodic => ui.lane = Lane::Probability,
        1..=5 => {
            let selected = (index - 1) as usize;
            if ui.mute {
                project.tracks[selected].muted = !project.tracks[selected].muted;
            } else if ui.solo {
                project.tracks[selected].solo = !project.tracks[selected].solo;
            } else if ui.clear {
                project.tracks[selected].clear_steps();
            } else {
                ui.stop_preview(project, out);
                ui.track = selected;
                ui.held_block = None;
            }
        }
        _ => {}
    }
}

fn grid_event(
    seq: &mut Seq95,
    row: u8,
    col: u8,
    pressed: bool,
    velocity: u8,
    out: &mut impl MidiOut,
    storage: &mut impl Storage,
) {
    if seq.ui.shift {
        if pressed {
            settings_press(seq, row, col);
        }
        return;
    }
    if seq.ui.record {
        if pressed {
            transpose_press(seq, row, col);
        }
        return;
    }
    match seq.ui.overlay {
        Overlay::Scale => {
            if pressed {
                scale_press(seq, row, col);
            }
            return;
        }
        Overlay::Projects => {
            if pressed {
                projects_press(seq, row, col, out, storage);
            }
            return;
        }
        Overlay::None => {}
    }

    match seq.project.tracks[seq.ui.track].kind {
        TrackKind::Melodic => melodic_press(seq, row, col, pressed),
        TrackKind::Drum => drum_press(seq, row, col, pressed, velocity, out),
    }
}

fn block_press(seq: &mut Seq95, block: u8, pressed: bool) {
    let ui = &mut seq.ui;
    let track = &mut seq.project.tracks[ui.track];
    if !pressed {
        if ui.held_block == Some(block) {
            ui.held_block = None;
        }
        return;
    }
    if ui.fixed_length {
        let start = track.loop_start;
        track.set_loop(start.min(block), block.max(start));
    } else if ui.clear {
        for step in block as usize * BLOCK_STEPS..(block as usize + 1) * BLOCK_STEPS {
            track.mel[step] = MelStep::EMPTY;
            for voice in track.drum.iter_mut() {
                voice[step] = Hit::EMPTY;
            }
        }
    } else if let Some(first) = ui.held_block {
        track.set_loop(first, block);
    } else {
        ui.held_block = Some(block);
        ui.view_block[ui.track] = block;
    }
}

fn melodic_press(seq: &mut Seq95, row: u8, col: u8, pressed: bool) {
    if row == 1 {
        block_press(seq, col - 1, pressed);
        return;
    }
    if !pressed {
        return;
    }
    let ui = &seq.ui;
    let level = row - 2;
    let step_index = ui.view_block[ui.track] as usize * BLOCK_STEPS + (col - 1) as usize;
    let lane = ui.lane;
    let (clear, mute) = (ui.clear, ui.mute);
    let step = &mut seq.project.tracks[ui.track].mel[step_index];

    if clear {
        *step = MelStep::EMPTY;
        return;
    }
    if mute {
        step.muted = !step.muted;
        return;
    }
    match lane {
        Lane::Notes => {
            if step.on && step.degree == level {
                step.on = false;
            } else {
                step.on = true;
                step.degree = level;
            }
        }
        Lane::Octave => step.octave = level,
        Lane::Velocity => step.velocity = level,
        Lane::Length => step.length = level,
        Lane::Probability => step.probability = level,
    }
}

fn drum_press(seq: &mut Seq95, row: u8, col: u8, pressed: bool, velocity: u8, out: &mut impl MidiOut) {
    let t = seq.ui.track;
    if row >= 5 {
        if !pressed {
            return;
        }
        let half = (seq.ui.view_block[t] / 4) as usize;
        let step = half * 32 + (8 - row) as usize * BLOCK_STEPS + (col - 1) as usize;
        let voice = seq.ui.voice[t] as usize;
        let ui = &seq.ui;
        let hit = &mut seq.project.tracks[t].drum[voice][step];
        if ui.clear {
            *hit = Hit::EMPTY;
        } else if ui.mute {
            let muted = hit.muted();
            hit.set_muted(!muted);
        } else if ui.hold_velocity {
            let level = if hit.on() { hit.velocity() } else { 0 };
            hit.set_velocity(if level == 0 { LEVELS - 1 } else { level - 1 });
            hit.set_on(true);
        } else if ui.hold_probability {
            let level = if hit.on() { hit.probability() } else { 0 };
            hit.set_probability(if level == 0 { LEVELS - 1 } else { level - 1 });
            hit.set_on(true);
        } else if hit.on() {
            *hit = Hit::EMPTY;
        } else {
            *hit = Hit::EMPTY;
            hit.set_velocity(level_from_velocity(velocity));
            hit.set_on(true);
        }
        return;
    }

    if col >= 5 {
        if row >= 3 {
            let block = (4 - row) * 4 + (col - 5);
            block_press(seq, block, pressed);
        }
        return;
    }

    // Voice pads.
    let voice = (row - 1) * 4 + (col - 1);
    if !pressed {
        if let Some((track, _, note)) = seq.ui.preview {
            let track_ref = &seq.project.tracks[track as usize];
            if track as usize == t && note == track_ref.drum_base.saturating_add(voice) {
                seq.ui.stop_preview(&seq.project, out);
            }
        }
        return;
    }
    let ui = &mut seq.ui;
    let track = &mut seq.project.tracks[t];
    if ui.mute {
        track.voice_mutes ^= 1 << voice;
        return;
    }
    if ui.clear {
        track.drum[voice as usize] = [Hit::EMPTY; STEPS];
        return;
    }
    ui.voice[t] = voice;
    let note = track.drum_base as u16 + voice as u16;
    if note <= 127 {
        ui.stop_preview(&seq.project, out);
        let track = &seq.project.tracks[t];
        let vel = if velocity == 0 { 100 } else { velocity };
        out.send(track.ports, &[0x90 | track.channel, note as u8, vel]);
        seq.ui.preview = Some((t as u8, track.channel, note as u8));
    }
}

fn settings_press(seq: &mut Seq95, row: u8, col: u8) {
    let project = &mut seq.project;
    let track = &mut project.tracks[seq.ui.track];
    match row {
        8 => track.channel = col - 1,
        7 => track.channel = col + 7,
        6 => match col {
            1 => track.kind = TrackKind::Drum,
            2 => track.kind = TrackKind::Melodic,
            4 => track.ports ^= PORT_USB_MIDI,
            5 => track.ports ^= PORT_USB_DAW,
            6 => track.ports ^= PORT_DIN,
            _ => {}
        },
        5 => track.rate = col - 1,
        4 => match col {
            1 => project.clock = ClockSource::Internal,
            2 => project.clock = ClockSource::External,
            4 => project.send_clock = !project.send_clock,
            8 => {
                project.transpose_channel = if project.transpose_channel < 16 { 0xff } else { 15 };
            }
            _ => {}
        },
        3 => match track.kind {
            TrackKind::Melodic => track.base_octave = col,
            TrackKind::Drum => {
                if col <= 4 {
                    track.drum_base = 24 + 12 * (col - 1);
                }
            }
        },
        2 => match col {
            1 => {
                if let Some(bpm) = seq.engine.tap() {
                    project.bpm = bpm;
                }
            }
            5 => project.bpm = project.bpm.saturating_sub(10).max(MIN_BPM),
            6 => project.bpm = project.bpm.saturating_sub(1).max(MIN_BPM),
            7 => project.bpm = (project.bpm + 1).min(MAX_BPM),
            8 => project.bpm = (project.bpm + 10).min(MAX_BPM),
            _ => {}
        },
        1 => project.bpm = 70 + 10 * col as u16,
        _ => {}
    }
}

/// Piano layout used by the scale and transposition overlays: white keys on
/// the lower row, black keys above. Returns the semitone (0..=12).
fn piano_key(upper: bool, col: u8) -> Option<u8> {
    if upper {
        match col {
            2 => Some(1),
            3 => Some(3),
            5 => Some(6),
            6 => Some(8),
            7 => Some(10),
            _ => None,
        }
    } else {
        Some([0, 2, 4, 5, 7, 9, 11, 12][(col - 1) as usize])
    }
}

fn scale_press(seq: &mut Seq95, row: u8, col: u8) {
    let project = &mut seq.project;
    match row {
        8 | 7 => {
            if let Some(key) = piano_key(row == 8, col) {
                project.root = key % 12;
            }
        }
        5 => project.scale = col - 1,
        4 => project.scale = col + 7,
        _ => {}
    }
}

fn transpose_press(seq: &mut Seq95, row: u8, col: u8) {
    let project = &mut seq.project;
    if row == 8 && col == 8 {
        project.transpose = 0;
        return;
    }
    let octave: i8 = match row {
        5 | 6 => 12,
        3 | 4 => -12,
        1 | 2 => 0,
        _ => return,
    };
    if let Some(key) = piano_key(row % 2 == 0, col) {
        project.transpose = octave + key as i8;
    }
}

fn projects_press(
    seq: &mut Seq95,
    row: u8,
    col: u8,
    out: &mut impl MidiOut,
    storage: &mut impl Storage,
) {
    let slot = col - 1;
    match row {
        8 => {
            if seq.load_project(slot, storage, out) {
                seq.ui.current_slot = Some(slot);
                seq.ui.flash_message(grid(8, col), WHITE);
            } else {
                seq.ui.flash_message(grid(8, col), RED);
            }
        }
        7 => {
            if seq.save_project(slot, storage) {
                seq.ui.current_slot = Some(slot);
                seq.ui.slot_used[slot as usize] = true;
                seq.ui.flash_message(grid(7, col), WHITE);
            } else {
                seq.ui.flash_message(grid(7, col), RED);
            }
        }
        5 if col == 8 => {
            seq.engine.all_notes_off(out);
            seq.project = Project::new();
            seq.ui.current_slot = None;
            seq.ui.flash_message(grid(5, 8), WHITE);
        }
        _ => {}
    }
}

fn randomise(seq: &mut Seq95) {
    let t = seq.ui.track;
    let lane = seq.ui.lane;
    let voice = seq.ui.voice[t] as usize;
    let first = seq.project.tracks[t].loop_first_step();
    let len = seq.project.tracks[t].loop_len();
    for step in first..first + len {
        let r = seq.engine.random();
        let track = &mut seq.project.tracks[t];
        match track.kind {
            TrackKind::Drum => {
                let hit = &mut track.drum[voice][step];
                *hit = Hit::EMPTY;
                if r % 4 == 0 {
                    hit.set_velocity(3 + ((r >> 8) % 4) as u8);
                    hit.set_on(true);
                }
            }
            TrackKind::Melodic => {
                let s = &mut track.mel[step];
                let level = ((r >> 8) % LEVELS as u32) as u8;
                match lane {
                    Lane::Notes => {
                        s.on = r % 2 == 0;
                        s.degree = level;
                    }
                    Lane::Octave => s.octave = level,
                    Lane::Velocity => s.velocity = level,
                    Lane::Length => s.length = level,
                    Lane::Probability => s.probability = level,
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

pub(crate) fn render(seq: &Seq95, frame: &mut [u32; LED_COUNT]) {
    frame.fill(0);
    let ui = &seq.ui;
    let project = &seq.project;
    let t = ui.track;
    let track = &project.tracks[t];
    let colour = TRACK_COLOURS[t];

    // Grid.
    if ui.shift {
        render_settings(seq, frame);
    } else if ui.record {
        render_transpose(seq, frame);
    } else {
        match ui.overlay {
            Overlay::Scale => render_scale(seq, frame),
            Overlay::Projects => render_projects(seq, frame),
            Overlay::None => match track.kind {
                TrackKind::Melodic => render_melodic(seq, frame),
                TrackKind::Drum => render_drum(seq, frame),
            },
        }
    }

    // Track buttons.
    for i in 0..TRACKS {
        let tr = &project.tracks[i];
        frame[i + 1] = if i == t {
            TRACK_COLOURS[i]
        } else if !project.track_audible(i) {
            0x100000
        } else {
            dim(TRACK_COLOURS[i], 1, 8)
        };
        if tr.solo {
            frame[i + 1] = if i == t { YELLOW } else { dim(YELLOW, 1, 4) };
        }
    }

    // Left column.
    frame[UP as usize] = if project.transpose > 0 { CYAN } else { dim(CYAN, 1, 8) };
    frame[DOWN as usize] = if project.transpose < 0 { CYAN } else { dim(CYAN, 1, 8) };
    frame[CLEAR as usize] = if ui.clear { WHITE } else { 0x181818 };
    frame[DUPLICATE as usize] = 0x181818;
    frame[QUANTISE as usize] = RATE_COLOURS[track.rate as usize % 8];
    frame[FIXED_LENGTH as usize] = if ui.fixed_length { WHITE } else { 0x181818 };
    frame[PLAY as usize] = if seq.engine.beat_led() {
        0x00ff00
    } else if seq.engine.running() {
        0x004000
    } else if project.clock == ClockSource::External {
        0x000830
    } else {
        0x001000
    };
    frame[RECORD as usize] = if ui.record { RED } else { 0x200000 };

    // Top row.
    frame[SHIFT as usize] = if ui.shift { WHITE } else { 0x181818 };
    frame[LEFT as usize] = 0x181818;
    frame[RIGHT as usize] = 0x181818;
    frame[CUSTOM as usize] = MAGENTA;
    frame[PROJECTS as usize] = if ui.overlay == Overlay::Projects { WHITE } else { 0x181818 };

    // Bottom function row.
    frame[MUTE as usize] = if ui.mute { RED } else { 0x200000 };
    frame[SOLO as usize] = if ui.solo { YELLOW } else { 0x201c00 };

    // Right column.
    frame[SCENE_SCALE as usize] = if ui.overlay == Overlay::Scale { AMBER } else { dim(AMBER, 1, 6) };
    frame[SCENE_RATE as usize] = RATE_COLOURS[track.rate as usize % 8];
    frame[SCENE_RANDOM as usize] = dim(MAGENTA, 1, 4);
    match track.kind {
        TrackKind::Melodic => {
            for (index, lane) in [
                (SCENE_NOTES, Lane::Notes),
                (SCENE_VELOCITY, Lane::Velocity),
                (SCENE_OCTAVE, Lane::Octave),
                (SCENE_LENGTH, Lane::Length),
                (SCENE_PROBABILITY, Lane::Probability),
            ] {
                frame[index as usize] = if ui.lane == lane { colour } else { dim(colour, 1, 8) };
            }
        }
        TrackKind::Drum => {
            frame[SCENE_VELOCITY as usize] = if ui.hold_velocity { WHITE } else { dim(colour, 1, 8) };
            frame[SCENE_PROBABILITY as usize] =
                if ui.hold_probability { WHITE } else { dim(CYAN, 1, 8) };
        }
    }

    if let Some((index, colour, until)) = ui.message {
        if ui.now_us < until {
            frame[index as usize] = colour;
        }
    }
}

fn block_colour(seq: &Seq95, block: u8, viewed: bool) -> u32 {
    let t = seq.ui.track;
    let track = &seq.project.tracks[t];
    let colour = TRACK_COLOURS[t];
    let playing = seq
        .engine
        .playing_step(t)
        .map(|s| s as usize / BLOCK_STEPS == block as usize)
        .unwrap_or(false);
    if playing {
        WHITE
    } else if viewed {
        colour
    } else if block >= track.loop_start && block <= track.loop_end {
        dim(colour, 1, 6)
    } else {
        0x040404
    }
}

fn render_melodic(seq: &Seq95, frame: &mut [u32; LED_COUNT]) {
    let t = seq.ui.track;
    let track = &seq.project.tracks[t];
    let colour = TRACK_COLOURS[t];
    let view = seq.ui.view_block[t];
    let playing = seq.engine.playing_step(t);
    let scale_len = seq.project.scale_notes().len() as u8;

    for block in 0..BLOCKS as u8 {
        frame[grid(1, block + 1) as usize] = block_colour(seq, block, block == view);
    }

    for col in 1..=8u8 {
        let index = view as usize * BLOCK_STEPS + (col - 1) as usize;
        let step = &track.mel[index];
        let in_loop = index >= track.loop_first_step()
            && index < track.loop_first_step() + track.loop_len();
        let note_colour = if step.muted { RED } else { colour };
        let on_playhead = playing == Some(index as u8);

        for level in 0..LEVELS {
            let lit = step.on
                && match seq.ui.lane {
                    Lane::Notes => step.degree == level,
                    Lane::Octave => step.octave == level,
                    Lane::Velocity => level <= step.velocity,
                    Lane::Length => level <= step.length,
                    Lane::Probability => level <= step.probability,
                };
            let top = match seq.ui.lane {
                Lane::Notes => step.degree,
                Lane::Octave => step.octave,
                Lane::Velocity => step.velocity,
                Lane::Length => step.length,
                Lane::Probability => step.probability,
            };
            let mut c = if lit {
                if level == top { note_colour } else { dim(note_colour, 1, 4) }
            } else {
                let marker = match seq.ui.lane {
                    Lane::Notes => level % scale_len == 0,
                    Lane::Octave => level == OCTAVE_CENTER,
                    _ => false,
                };
                if marker && in_loop {
                    0x0a0500
                } else {
                    0
                }
            };
            if on_playhead {
                c = if lit { WHITE } else { PLAYHEAD };
            }
            frame[grid(level + 2, col) as usize] = c;
        }
    }
}

fn render_drum(seq: &Seq95, frame: &mut [u32; LED_COUNT]) {
    let t = seq.ui.track;
    let track = &seq.project.tracks[t];
    let colour = TRACK_COLOURS[t];
    let half = (seq.ui.view_block[t] / 4) as usize;
    let voice = seq.ui.voice[t] as usize;
    let playing = seq.engine.playing_step(t);
    let first = track.loop_first_step();
    let end = first + track.loop_len();

    for row in 5..=8u8 {
        for col in 1..=8u8 {
            let step = half * 32 + (8 - row) as usize * BLOCK_STEPS + (col - 1) as usize;
            let hit = track.drum[voice][step];
            let mut c = if hit.on() {
                let base = if hit.muted() {
                    RED
                } else if hit.probability() < LEVELS - 1 {
                    CYAN
                } else {
                    colour
                };
                dim(base, hit.velocity() as u32 + 2, LEVELS as u32 + 1)
            } else if step >= first && step < end {
                if step % 4 == 0 { 0x0a0a0a } else { 0x020202 }
            } else {
                0
            };
            if playing == Some(step as u8) {
                c = if hit.on() { WHITE } else { PLAYHEAD };
            }
            frame[grid(row, col) as usize] = c;
        }
    }

    for v in 0..VOICES as u8 {
        let row = v / 4 + 1;
        let col = v % 4 + 1;
        let used = (first..end).any(|s| track.drum[v as usize][s].on());
        let muted = track.voice_mutes & (1 << v) != 0;
        let firing = playing
            .map(|s| track.drum[v as usize][s as usize].on())
            .unwrap_or(false);
        frame[grid(row, col) as usize] = if muted {
            0x200000
        } else if firing && !muted {
            WHITE
        } else if v as usize == voice {
            colour
        } else if used {
            dim(colour, 1, 4)
        } else {
            dim(colour, 1, 16)
        };
    }

    for block in 0..BLOCKS as u8 {
        let row = 4 - block / 4;
        let col = 5 + block % 4;
        let viewed = block as usize / 4 == half;
        frame[grid(row, col) as usize] = block_colour(seq, block, viewed);
    }
}

fn render_settings(seq: &Seq95, frame: &mut [u32; LED_COUNT]) {
    let project = &seq.project;
    let t = seq.ui.track;
    let track = &project.tracks[t];
    let colour = TRACK_COLOURS[t];

    for ch in 0..16u8 {
        let row = 8 - ch / 8;
        let col = ch % 8 + 1;
        frame[grid(row, col) as usize] = if track.channel == ch { colour } else { 0x101010 };
    }
    frame[grid(6, 1) as usize] = if track.kind == TrackKind::Drum { RED } else { 0x200000 };
    frame[grid(6, 2) as usize] = if track.kind == TrackKind::Melodic { 0x00ff00 } else { 0x002000 };
    for (col, bit) in [(4, PORT_USB_MIDI), (5, PORT_USB_DAW), (6, PORT_DIN)] {
        frame[grid(6, col) as usize] = if track.ports & bit != 0 { 0x2060ff } else { 0x040818 };
    }
    for rate in 0..8u8 {
        frame[grid(5, rate + 1) as usize] = if track.rate == rate {
            RATE_COLOURS[rate as usize]
        } else {
            dim(RATE_COLOURS[rate as usize], 1, 10)
        };
    }
    let internal = project.clock == ClockSource::Internal;
    frame[grid(4, 1) as usize] = if internal { 0x00ff00 } else { 0x002000 };
    frame[grid(4, 2) as usize] = if internal { 0x000820 } else { 0x0040ff };
    frame[grid(4, 4) as usize] = if project.send_clock { AMBER } else { dim(AMBER, 1, 10) };
    frame[grid(4, 8) as usize] = if project.transpose_channel < 16 { CYAN } else { dim(CYAN, 1, 10) };
    match track.kind {
        TrackKind::Melodic => {
            for col in 1..=8u8 {
                frame[grid(3, col) as usize] = if track.base_octave == col { colour } else { 0x101010 };
            }
        }
        TrackKind::Drum => {
            for col in 1..=4u8 {
                let base = 24 + 12 * (col - 1);
                frame[grid(3, col) as usize] = if track.drum_base == base { colour } else { 0x101010 };
            }
        }
    }
    frame[grid(2, 1) as usize] = if seq.engine.beat_led() { WHITE } else { 0x303030 };
    frame[grid(2, 5) as usize] = 0x400000;
    frame[grid(2, 6) as usize] = 0x100000;
    frame[grid(2, 7) as usize] = 0x001000;
    frame[grid(2, 8) as usize] = 0x004000;
    for col in 1..=8u8 {
        let preset = 70 + 10 * col as u16;
        frame[grid(1, col) as usize] = if project.bpm == preset { WHITE } else { 0x080808 };
    }
}

fn render_piano(frame: &mut [u32; LED_COUNT], lower_row: u8, selected: Option<u8>, colour: u32) {
    for col in 1..=8u8 {
        for upper in [false, true] {
            if let Some(key) = piano_key(upper, col) {
                let row = if upper { lower_row + 1 } else { lower_row };
                frame[grid(row, col) as usize] = if selected == Some(key) {
                    colour
                } else if upper {
                    0x0c0418
                } else {
                    0x141414
                };
            }
        }
    }
}

fn render_scale(seq: &Seq95, frame: &mut [u32; LED_COUNT]) {
    let project = &seq.project;
    render_piano(frame, 7, Some(project.root), AMBER);
    for scale in 0..16u8 {
        let row = 5 - scale / 8;
        let col = scale % 8 + 1;
        frame[grid(row, col) as usize] = if project.scale == scale { AMBER } else { 0x100800 };
    }
}

fn render_transpose(seq: &Seq95, frame: &mut [u32; LED_COUNT]) {
    let tr = seq.project.transpose;
    for (lower_row, octave) in [(1u8, 0i8), (3, -12), (5, 12)] {
        let key = tr - octave;
        let selected = if (0..=12).contains(&key) { Some(key as u8) } else { None };
        render_piano(frame, lower_row, selected, CYAN);
    }
    frame[grid(8, 8) as usize] = if tr == 0 { WHITE } else { 0x202020 };
}

fn render_projects(seq: &Seq95, frame: &mut [u32; LED_COUNT]) {
    for slot in 0..PROJECT_SLOTS {
        let current = seq.ui.current_slot == Some(slot);
        let used = seq.ui.slot_used[slot as usize];
        frame[grid(8, slot + 1) as usize] = if current {
            WHITE
        } else if used {
            0x00a000
        } else {
            0x040404
        };
        frame[grid(7, slot + 1) as usize] = if current { 0xff2000 } else { 0x200400 };
    }
    frame[grid(5, 8) as usize] = 0x202020;
}
