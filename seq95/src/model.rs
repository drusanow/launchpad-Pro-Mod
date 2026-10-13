// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 launchpad-Pro-Mod contributors

//! Project data: everything that is saved to a project slot.

pub const TRACKS: usize = 5;
pub const STEPS: usize = 64;
pub const BLOCK_STEPS: usize = 8;
pub const BLOCKS: usize = STEPS / BLOCK_STEPS;
pub const VOICES: usize = 16;
/// Number of values in each per-step lane (one per editable grid row).
pub const LEVELS: u8 = 7;

/// Output port bits.
pub const PORT_USB_MIDI: u8 = 1 << 0;
pub const PORT_USB_DAW: u8 = 1 << 1;
pub const PORT_DIN: u8 = 1 << 2;

pub const VELOCITY_LEVELS: [u8; LEVELS as usize] = [16, 32, 48, 64, 85, 106, 127];
pub const PROBABILITY_LEVELS: [u8; LEVELS as usize] = [12, 25, 37, 50, 62, 75, 100];
/// Gate length in quarter steps.
pub const LENGTH_QUARTERS: [u16; LEVELS as usize] = [1, 2, 3, 4, 8, 16, 32];
/// Octave lane: level 3 is "no offset".
pub const OCTAVE_CENTER: u8 = 3;

/// Step rates in MIDI clock pulses (24 PPQN): 1/4, 1/4T, 1/8, 1/8T, 1/16, 1/16T, 1/32, 1/32T.
pub const RATE_PULSES: [u8; 8] = [24, 16, 12, 8, 6, 4, 3, 2];
pub const DEFAULT_RATE: u8 = 4;

pub const SCALES: [&[u8]; 16] = [
    &[0, 2, 4, 5, 7, 9, 11],        // Major
    &[0, 2, 3, 5, 7, 8, 10],        // Natural minor
    &[0, 2, 3, 5, 7, 9, 10],        // Dorian
    &[0, 1, 3, 5, 7, 8, 10],        // Phrygian
    &[0, 2, 4, 6, 7, 9, 11],        // Lydian
    &[0, 2, 4, 5, 7, 9, 10],        // Mixolydian
    &[0, 1, 3, 5, 6, 8, 10],        // Locrian
    &[0, 2, 3, 5, 7, 8, 11],        // Harmonic minor
    &[0, 2, 3, 5, 7, 9, 11],        // Melodic minor
    &[0, 2, 4, 7, 9],               // Major pentatonic
    &[0, 3, 5, 7, 10],              // Minor pentatonic
    &[0, 3, 5, 6, 7, 10],           // Blues
    &[0, 2, 4, 6, 8, 10],           // Whole tone
    &[0, 2, 3, 5, 6, 8, 9, 11],     // Diminished (whole-half)
    &[0, 2, 3, 6, 7, 8, 11],        // Hungarian minor
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], // Chromatic
];

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TrackKind {
    Drum = 0,
    Melodic = 1,
}

/// One step of a melodic track. Lanes are stored as levels `0..LEVELS`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct MelStep {
    pub on: bool,
    pub muted: bool,
    pub degree: u8,
    pub octave: u8,
    pub velocity: u8,
    pub length: u8,
    pub probability: u8,
}

impl MelStep {
    pub const EMPTY: Self = Self {
        on: false,
        muted: false,
        degree: 0,
        octave: OCTAVE_CENTER,
        velocity: 6,
        length: 3,
        probability: 6,
    };
}

/// One drum hit, packed: bit7 on, bit6 muted, bits 3..5 probability level, bits 0..2 velocity level.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Hit(pub u8);

impl Hit {
    pub const EMPTY: Self = Self(6 | (6 << 3));

    pub fn on(self) -> bool {
        self.0 & 0x80 != 0
    }
    pub fn muted(self) -> bool {
        self.0 & 0x40 != 0
    }
    pub fn velocity(self) -> u8 {
        (self.0 & 0x07).min(LEVELS - 1)
    }
    pub fn probability(self) -> u8 {
        ((self.0 >> 3) & 0x07).min(LEVELS - 1)
    }
    pub fn set_on(&mut self, on: bool) {
        self.0 = (self.0 & !0x80) | if on { 0x80 } else { 0 };
    }
    pub fn set_muted(&mut self, muted: bool) {
        self.0 = (self.0 & !0x40) | if muted { 0x40 } else { 0 };
    }
    pub fn set_velocity(&mut self, level: u8) {
        self.0 = (self.0 & !0x07) | level.min(LEVELS - 1);
    }
    pub fn set_probability(&mut self, level: u8) {
        self.0 = (self.0 & !0x38) | (level.min(LEVELS - 1) << 3);
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct Track {
    pub kind: TrackKind,
    /// MIDI channel 0..15.
    pub channel: u8,
    /// Bitmask of `PORT_*`.
    pub ports: u8,
    pub muted: bool,
    pub solo: bool,
    /// Index into `RATE_PULSES`.
    pub rate: u8,
    /// Inclusive loop range in blocks of `BLOCK_STEPS`.
    pub loop_start: u8,
    pub loop_end: u8,
    /// Melodic: octave of the root (MIDI note = 12 * (base_octave + 1) + root).
    pub base_octave: u8,
    /// Drum: MIDI note of voice 0; voices are chromatic upwards.
    pub drum_base: u8,
    /// Drum: one bit per muted voice.
    pub voice_mutes: u16,
    pub mel: [MelStep; STEPS],
    pub drum: [[Hit; STEPS]; VOICES],
}

impl Track {
    pub const fn new(kind: TrackKind, channel: u8) -> Self {
        Self {
            kind,
            channel,
            ports: PORT_USB_MIDI | PORT_DIN,
            muted: false,
            solo: false,
            rate: DEFAULT_RATE,
            loop_start: 0,
            loop_end: 1,
            base_octave: 3,
            drum_base: 36,
            voice_mutes: 0,
            mel: [MelStep::EMPTY; STEPS],
            drum: [[Hit::EMPTY; STEPS]; VOICES],
        }
    }

    pub fn loop_first_step(&self) -> usize {
        self.loop_start as usize * BLOCK_STEPS
    }

    pub fn loop_len(&self) -> usize {
        (self.loop_end as usize + 1 - self.loop_start as usize) * BLOCK_STEPS
    }

    pub fn set_loop(&mut self, a: u8, b: u8) {
        let (s, e) = if a <= b { (a, b) } else { (b, a) };
        self.loop_start = s.min(BLOCKS as u8 - 1);
        self.loop_end = e.min(BLOCKS as u8 - 1);
    }

    pub fn clear_steps(&mut self) {
        self.mel = [MelStep::EMPTY; STEPS];
        self.drum = [[Hit::EMPTY; STEPS]; VOICES];
    }

    /// Copies the current loop directly after itself and doubles the loop
    /// length (as far as 64 steps allow).
    pub fn duplicate_loop(&mut self) {
        let blocks = self.loop_end - self.loop_start + 1;
        let new_end = (self.loop_end + blocks).min(BLOCKS as u8 - 1);
        if new_end == self.loop_end {
            return;
        }
        let src = self.loop_first_step();
        let len = self.loop_len();
        let dst = (self.loop_end as usize + 1) * BLOCK_STEPS;
        let copy = len.min(STEPS - dst);
        for i in 0..copy {
            self.mel[dst + i] = self.mel[src + i];
            for voice in 0..VOICES {
                self.drum[voice][dst + i] = self.drum[voice][src + i];
            }
        }
        self.loop_end = new_end;
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ClockSource {
    Internal = 0,
    External = 1,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Project {
    /// Tempo in BPM (internal clock).
    pub bpm: u16,
    pub root: u8,
    pub scale: u8,
    /// Live transposition in semitones, applied to melodic tracks.
    pub transpose: i8,
    pub clock: ClockSource,
    pub send_clock: bool,
    /// Channel (0..15) whose incoming notes transpose melodic tracks, 0xFF = off.
    pub transpose_channel: u8,
    pub tracks: [Track; TRACKS],
}

impl Project {
    pub const fn new() -> Self {
        Self {
            bpm: 120,
            root: 0,
            scale: 0,
            transpose: 0,
            clock: ClockSource::Internal,
            send_clock: false,
            transpose_channel: 15,
            tracks: [
                Track::new(TrackKind::Drum, 9),
                Track::new(TrackKind::Melodic, 0),
                Track::new(TrackKind::Melodic, 1),
                Track::new(TrackKind::Melodic, 2),
                Track::new(TrackKind::Melodic, 3),
            ],
        }
    }

    pub fn scale_notes(&self) -> &'static [u8] {
        SCALES[(self.scale as usize) % SCALES.len()]
    }

    /// Semitone offset of a scale degree (degrees past the scale wrap into the next octave).
    pub fn degree_offset(&self, degree: u8) -> i16 {
        let scale = self.scale_notes();
        let n = scale.len() as u8;
        scale[(degree % n) as usize] as i16 + 12 * (degree / n) as i16
    }

    pub fn melodic_note(&self, track: &Track, step: &MelStep) -> Option<u8> {
        let note = 12 * (track.base_octave as i16 + 1)
            + self.root as i16
            + self.degree_offset(step.degree)
            + 12 * (step.octave as i16 - OCTAVE_CENTER as i16)
            + self.transpose as i16;
        if (0..=127).contains(&note) {
            Some(note as u8)
        } else {
            None
        }
    }

    pub fn any_solo(&self) -> bool {
        self.tracks.iter().any(|t| t.solo)
    }

    pub fn track_audible(&self, index: usize) -> bool {
        let track = &self.tracks[index];
        if self.any_solo() {
            track.solo
        } else {
            !track.muted
        }
    }
}

impl Default for Project {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

const MAGIC: [u8; 6] = *b"SEQ95P";
const FORMAT_VERSION: u8 = 1;
const HEADER_SIZE: usize = MAGIC.len() + 1 + 4;
const GLOBAL_SIZE: usize = 16;
const TRACK_HEADER_SIZE: usize = 16;
const MEL_STEP_SIZE: usize = 6;
const TRACK_SIZE: usize = TRACK_HEADER_SIZE + STEPS * MEL_STEP_SIZE + VOICES * STEPS;

/// Size of an encoded project in bytes.
pub const PROJECT_WIRE_SIZE: usize = HEADER_SIZE + GLOBAL_SIZE + TRACKS * TRACK_SIZE;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DecodeError {
    TooShort,
    BadMagic,
    BadVersion,
    BadChecksum,
}

impl Project {
    pub fn encode(&self, out: &mut [u8]) -> usize {
        assert!(out.len() >= PROJECT_WIRE_SIZE);
        let out = &mut out[..PROJECT_WIRE_SIZE];
        out.fill(0);
        out[..MAGIC.len()].copy_from_slice(&MAGIC);
        out[MAGIC.len()] = FORMAT_VERSION;

        let body = &mut out[HEADER_SIZE..];
        let g = &mut body[..GLOBAL_SIZE];
        g[0..2].copy_from_slice(&self.bpm.to_le_bytes());
        g[2] = self.root;
        g[3] = self.scale;
        g[4] = self.transpose as u8;
        g[5] = self.clock as u8;
        g[6] = self.send_clock as u8;
        g[7] = self.transpose_channel;

        for (i, track) in self.tracks.iter().enumerate() {
            let start = GLOBAL_SIZE + i * TRACK_SIZE;
            encode_track(track, &mut body[start..start + TRACK_SIZE]);
        }

        let crc = crc32(&out[HEADER_SIZE..]);
        out[MAGIC.len() + 1..HEADER_SIZE].copy_from_slice(&crc.to_le_bytes());
        PROJECT_WIRE_SIZE
    }

    pub fn decode(input: &[u8]) -> Result<Self, DecodeError> {
        if input.len() < PROJECT_WIRE_SIZE {
            return Err(DecodeError::TooShort);
        }
        let input = &input[..PROJECT_WIRE_SIZE];
        if input[..MAGIC.len()] != MAGIC {
            return Err(DecodeError::BadMagic);
        }
        if input[MAGIC.len()] != FORMAT_VERSION {
            return Err(DecodeError::BadVersion);
        }
        let stored = u32::from_le_bytes([
            input[MAGIC.len() + 1],
            input[MAGIC.len() + 2],
            input[MAGIC.len() + 3],
            input[MAGIC.len() + 4],
        ]);
        if stored != crc32(&input[HEADER_SIZE..]) {
            return Err(DecodeError::BadChecksum);
        }

        let body = &input[HEADER_SIZE..];
        let g = &body[..GLOBAL_SIZE];
        let mut project = Project::new();
        project.bpm = u16::from_le_bytes([g[0], g[1]]).clamp(MIN_BPM, MAX_BPM);
        project.root = g[2] % 12;
        project.scale = g[3] % SCALES.len() as u8;
        project.transpose = (g[4] as i8).clamp(-MAX_TRANSPOSE, MAX_TRANSPOSE);
        project.clock = if g[5] == 1 {
            ClockSource::External
        } else {
            ClockSource::Internal
        };
        project.send_clock = g[6] != 0;
        project.transpose_channel = if g[7] < 16 { g[7] } else { 0xff };

        for (i, track) in project.tracks.iter_mut().enumerate() {
            let start = GLOBAL_SIZE + i * TRACK_SIZE;
            decode_track(track, &body[start..start + TRACK_SIZE]);
        }
        Ok(project)
    }
}

pub const MIN_BPM: u16 = 30;
pub const MAX_BPM: u16 = 300;
pub const MAX_TRANSPOSE: i8 = 36;

fn encode_track(track: &Track, out: &mut [u8]) {
    let h = &mut out[..TRACK_HEADER_SIZE];
    h[0] = track.kind as u8;
    h[1] = track.channel;
    h[2] = track.ports;
    h[3] = track.muted as u8 | (track.solo as u8) << 1;
    h[4] = track.rate;
    h[5] = track.loop_start;
    h[6] = track.loop_end;
    h[7] = track.base_octave;
    h[8] = track.drum_base;
    h[9..11].copy_from_slice(&track.voice_mutes.to_le_bytes());

    let mel = &mut out[TRACK_HEADER_SIZE..TRACK_HEADER_SIZE + STEPS * MEL_STEP_SIZE];
    for (step, chunk) in track.mel.iter().zip(mel.chunks_exact_mut(MEL_STEP_SIZE)) {
        chunk[0] = step.on as u8 | (step.muted as u8) << 1;
        chunk[1] = step.degree;
        chunk[2] = step.octave;
        chunk[3] = step.velocity;
        chunk[4] = step.length;
        chunk[5] = step.probability;
    }

    let drum = &mut out[TRACK_HEADER_SIZE + STEPS * MEL_STEP_SIZE..];
    for (voice, chunk) in track.drum.iter().zip(drum.chunks_exact_mut(STEPS)) {
        for (hit, byte) in voice.iter().zip(chunk.iter_mut()) {
            *byte = hit.0;
        }
    }
}

fn decode_track(track: &mut Track, input: &[u8]) {
    let h = &input[..TRACK_HEADER_SIZE];
    track.kind = if h[0] == 0 {
        TrackKind::Drum
    } else {
        TrackKind::Melodic
    };
    track.channel = h[1] & 0x0f;
    track.ports = h[2] & (PORT_USB_MIDI | PORT_USB_DAW | PORT_DIN);
    track.muted = h[3] & 1 != 0;
    track.solo = h[3] & 2 != 0;
    track.rate = h[4].min(RATE_PULSES.len() as u8 - 1);
    track.set_loop(h[5], h[6]);
    track.base_octave = h[7].min(8);
    track.drum_base = h[8].min(127 - VOICES as u8 + 1);
    track.voice_mutes = u16::from_le_bytes([h[9], h[10]]);

    let mel = &input[TRACK_HEADER_SIZE..TRACK_HEADER_SIZE + STEPS * MEL_STEP_SIZE];
    for (step, chunk) in track.mel.iter_mut().zip(mel.chunks_exact(MEL_STEP_SIZE)) {
        step.on = chunk[0] & 1 != 0;
        step.muted = chunk[0] & 2 != 0;
        step.degree = chunk[1].min(LEVELS - 1);
        step.octave = chunk[2].min(LEVELS - 1);
        step.velocity = chunk[3].min(LEVELS - 1);
        step.length = chunk[4].min(LEVELS - 1);
        step.probability = chunk[5].min(LEVELS - 1);
    }

    let drum = &input[TRACK_HEADER_SIZE + STEPS * MEL_STEP_SIZE..];
    for (voice, chunk) in track.drum.iter_mut().zip(drum.chunks_exact(STEPS)) {
        for (hit, byte) in voice.iter_mut().zip(chunk.iter()) {
            *hit = Hit(*byte);
        }
    }
}

/// True if `bytes` starts with a Seq95 project header (any format version).
pub fn looks_like_project(bytes: &[u8]) -> bool {
    bytes.len() >= MAGIC.len() && bytes[..MAGIC.len()] == MAGIC
}

pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= *byte as u32;
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}
