// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 launchpad-Pro-Mod contributors

//! Playback: clock handling, step triggering, probability and note-off scheduling.

use crate::model::*;

/// Where the sequencer's MIDI goes. `ports` is a bitmask of `PORT_*`.
pub trait MidiOut {
    fn send(&mut self, ports: u8, msg: &[u8]);
}

/// Ports that receive MIDI clock / start / stop when the internal clock runs.
pub const CLOCK_PORTS: u8 = PORT_USB_MIDI | PORT_DIN;

const MAX_PENDING: usize = 96;
/// If the internal clock falls this many pulses behind (e.g. during a flash
/// write), skip ahead instead of firing a burst of notes.
const MAX_CATCH_UP_PULSES: u32 = 4;
const US_PER_MINUTE_PER_PPQN: u64 = 60_000_000 / 24;
const TAP_TIMEOUT_US: u64 = 2_000_000;

#[derive(Copy, Clone)]
struct PendingOff {
    ports: u8,
    channel: u8,
    note: u8,
    pulses_left: u16,
}

pub struct Engine {
    running: bool,
    /// Next pulse to be played (24 PPQN).
    pulse: u32,
    anchor_us: u64,
    anchor_pulse: u32,
    anchor_bpm: u16,
    playing_step: [Option<u8>; TRACKS],
    pending: [Option<PendingOff>; MAX_PENDING],
    rng: u32,
    taps: [u64; 4],
    tap_count: u8,
    now_us: u64,
}

impl Engine {
    pub const fn new() -> Self {
        Self {
            running: false,
            pulse: 0,
            anchor_us: 0,
            anchor_pulse: 0,
            anchor_bpm: 120,
            playing_step: [None; TRACKS],
            pending: [None; MAX_PENDING],
            rng: 0x1234_5678,
            taps: [0; 4],
            tap_count: 0,
            now_us: 0,
        }
    }

    pub fn running(&self) -> bool {
        self.running
    }

    /// Absolute step (0..STEPS) that the track last played, while running.
    pub fn playing_step(&self, track: usize) -> Option<u8> {
        if self.running {
            self.playing_step[track]
        } else {
            None
        }
    }

    /// True on the first half of every beat; used for the play button blink.
    pub fn beat_led(&self) -> bool {
        self.running && (self.pulse.wrapping_sub(1) % 24) < 12
    }

    pub fn seed(&mut self, seed: u32) {
        self.rng ^= seed;
        if self.rng == 0 {
            self.rng = 0x9e37_79b9;
        }
    }

    pub fn random(&mut self) -> u32 {
        // xorshift32
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        x
    }

    fn chance(&mut self, percent: u8) -> bool {
        percent >= 100 || (self.random() % 100) < percent as u32
    }

    // -- transport -----------------------------------------------------------

    pub fn start(&mut self, project: &Project, out: &mut impl MidiOut) {
        self.all_notes_off(out);
        self.running = true;
        self.pulse = 0;
        self.playing_step = [None; TRACKS];
        self.anchor_us = self.now_us;
        self.anchor_pulse = 0;
        self.anchor_bpm = project.bpm;
        if project.clock == ClockSource::Internal && project.send_clock {
            out.send(CLOCK_PORTS, &[0xfa]);
        }
    }

    pub fn stop(&mut self, project: &Project, out: &mut impl MidiOut) {
        if self.running && project.clock == ClockSource::Internal && project.send_clock {
            out.send(CLOCK_PORTS, &[0xfc]);
        }
        self.running = false;
        self.all_notes_off(out);
    }

    pub fn toggle(&mut self, project: &Project, out: &mut impl MidiOut) {
        if self.running {
            self.stop(project, out);
        } else {
            self.start(project, out);
        }
    }

    /// Call often (every millisecond) with a monotonic microsecond timestamp.
    pub fn tick(&mut self, now_us: u64, project: &Project, out: &mut impl MidiOut) {
        self.now_us = now_us;
        if !self.running || project.clock != ClockSource::Internal {
            return;
        }

        if project.bpm != self.anchor_bpm {
            // Re-anchor so a tempo change takes effect from the next pulse.
            self.anchor_us = self.pulse_time(self.pulse).min(now_us.max(self.anchor_us));
            self.anchor_pulse = self.pulse;
            self.anchor_bpm = project.bpm;
        }

        let mut fired = 0;
        while now_us >= self.pulse_time(self.pulse) {
            if fired >= MAX_CATCH_UP_PULSES {
                self.anchor_us = now_us;
                self.anchor_pulse = self.pulse;
                break;
            }
            if project.send_clock {
                out.send(CLOCK_PORTS, &[0xf8]);
            }
            self.play_pulse(project, out);
            fired += 1;
        }
    }

    fn pulse_time(&self, pulse: u32) -> u64 {
        let bpm = self.anchor_bpm.max(1) as u64;
        self.anchor_us
            + (pulse.wrapping_sub(self.anchor_pulse) as u64) * US_PER_MINUTE_PER_PPQN / bpm
    }

    /// Handles incoming MIDI that the sequencer cares about: clock, transport
    /// and live transposition notes. Returns true if the message was used.
    pub fn midi_in(
        &mut self,
        msg: [u8; 3],
        project: &mut Project,
        out: &mut impl MidiOut,
    ) -> bool {
        let status = msg[0];
        if project.clock == ClockSource::External {
            match status {
                0xfa => {
                    self.all_notes_off(out);
                    self.running = true;
                    self.pulse = 0;
                    self.playing_step = [None; TRACKS];
                    return true;
                }
                0xfb => {
                    self.running = true;
                    return true;
                }
                0xfc => {
                    self.running = false;
                    self.all_notes_off(out);
                    return true;
                }
                0xf8 => {
                    if self.running {
                        self.play_pulse(project, out);
                    }
                    return true;
                }
                _ => {}
            }
        }

        if status & 0xf0 == 0x90
            && msg[2] != 0
            && project.transpose_channel < 16
            && status & 0x0f == project.transpose_channel
        {
            project.transpose = (msg[1] as i16 - 60)
                .clamp(-(MAX_TRANSPOSE as i16), MAX_TRANSPOSE as i16)
                as i8;
            return true;
        }
        false
    }

    /// Records a tap and returns the new tempo once there are enough taps.
    pub fn tap(&mut self) -> Option<u16> {
        let now = self.now_us;
        if self.tap_count > 0 && now.saturating_sub(self.taps[0]) > TAP_TIMEOUT_US {
            self.tap_count = 0;
        }
        self.taps.copy_within(0..3, 1);
        self.taps[0] = now;
        self.tap_count = (self.tap_count + 1).min(4);
        if self.tap_count < 2 {
            return None;
        }
        let span = self.taps[0] - self.taps[self.tap_count as usize - 1];
        let intervals = self.tap_count as u64 - 1;
        if span == 0 {
            return None;
        }
        let bpm = 60_000_000 * intervals / span;
        Some((bpm as u16).clamp(MIN_BPM, MAX_BPM))
    }

    // -- playback ------------------------------------------------------------

    fn play_pulse(&mut self, project: &Project, out: &mut impl MidiOut) {
        let pulse = self.pulse;
        self.pulse = self.pulse.wrapping_add(1);

        self.advance_note_offs(out);

        for index in 0..TRACKS {
            let track = &project.tracks[index];
            let rate = RATE_PULSES[(track.rate as usize).min(RATE_PULSES.len() - 1)] as u32;
            if pulse % rate != 0 {
                continue;
            }
            let count = (pulse / rate) as usize;
            let step = track.loop_first_step() + count % track.loop_len();
            self.playing_step[index] = Some(step as u8);

            if !project.track_audible(index) {
                continue;
            }
            self.trigger_step(project, index, step, rate as u16, out);
        }
    }

    fn trigger_step(
        &mut self,
        project: &Project,
        index: usize,
        step: usize,
        rate: u16,
        out: &mut impl MidiOut,
    ) {
        let track = &project.tracks[index];
        match track.kind {
            TrackKind::Melodic => {
                let s = track.mel[step];
                if !s.on || s.muted {
                    return;
                }
                if !self.chance(PROBABILITY_LEVELS[s.probability as usize]) {
                    return;
                }
                let Some(note) = project.melodic_note(track, &s) else {
                    return;
                };
                let gate = gate_pulses(rate, s.length);
                self.note_on(
                    track.ports,
                    track.channel,
                    note,
                    VELOCITY_LEVELS[s.velocity as usize],
                    gate,
                    out,
                );
            }
            TrackKind::Drum => {
                for voice in 0..VOICES {
                    let hit = track.drum[voice][step];
                    if !hit.on() || hit.muted() || track.voice_mutes & (1 << voice) != 0 {
                        continue;
                    }
                    if !self.chance(PROBABILITY_LEVELS[hit.probability() as usize]) {
                        continue;
                    }
                    let note = track.drum_base as usize + voice;
                    if note > 127 {
                        continue;
                    }
                    let gate = gate_pulses(rate, 3);
                    self.note_on(
                        track.ports,
                        track.channel,
                        note as u8,
                        VELOCITY_LEVELS[hit.velocity() as usize],
                        gate,
                        out,
                    );
                }
            }
        }
    }

    fn note_on(
        &mut self,
        ports: u8,
        channel: u8,
        note: u8,
        velocity: u8,
        gate: u16,
        out: &mut impl MidiOut,
    ) {
        // Retrigger: close a still-sounding copy of the same note first.
        for slot in self.pending.iter_mut() {
            if let Some(p) = slot {
                if p.ports == ports && p.channel == channel && p.note == note {
                    out.send(p.ports, &[0x80 | p.channel, p.note, 0]);
                    *slot = None;
                }
            }
        }

        let free = self.pending.iter().position(|slot| slot.is_none());
        let Some(free) = free else {
            // No room to track the note-off: don't start a note we can't stop.
            return;
        };
        out.send(ports, &[0x90 | (channel & 0x0f), note & 0x7f, velocity.clamp(1, 127)]);
        self.pending[free] = Some(PendingOff {
            ports,
            channel: channel & 0x0f,
            note: note & 0x7f,
            pulses_left: gate.max(1),
        });
    }

    fn advance_note_offs(&mut self, out: &mut impl MidiOut) {
        for slot in self.pending.iter_mut() {
            if let Some(p) = slot {
                p.pulses_left = p.pulses_left.saturating_sub(1);
                if p.pulses_left == 0 {
                    out.send(p.ports, &[0x80 | p.channel, p.note, 0]);
                    *slot = None;
                }
            }
        }
    }

    pub fn all_notes_off(&mut self, out: &mut impl MidiOut) {
        for slot in self.pending.iter_mut() {
            if let Some(p) = slot.take() {
                out.send(p.ports, &[0x80 | p.channel, p.note, 0]);
            }
        }
    }

    pub fn sounding_notes(&self) -> usize {
        self.pending.iter().filter(|slot| slot.is_some()).count()
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

pub fn gate_pulses(rate_pulses: u16, length_level: u8) -> u16 {
    let quarters = LENGTH_QUARTERS[(length_level as usize).min(LENGTH_QUARTERS.len() - 1)];
    ((rate_pulses * quarters) / 4).max(1)
}
