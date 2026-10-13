// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 launchpad-Pro-Mod contributors

//! Seq95: a Launchpad95-inspired hardware step sequencer for the Launchpad Pro Mk3.
//!
//! The crate is hardware independent. A host feeds it button presses, incoming
//! MIDI and a microsecond clock, and gives it somewhere to send MIDI, draw LEDs
//! and store projects. Button and LED indexes use the Launchpad Pro Mk3
//! programmer-mode layout (grid 11..88, edges 1..8, 10..80, 19..89, 90..98, 101..108).

#![cfg_attr(not(test), no_std)]

pub mod engine;
pub mod model;
mod ui;

pub use engine::{Engine, MidiOut};
pub use model::*;
pub use ui::{Lane, LED_COUNT};

/// Persistent project slots.
pub const PROJECT_SLOTS: u8 = 8;

/// Project storage provided by the host. Slots are `0..PROJECT_SLOTS`.
pub trait Storage {
    /// True if the slot holds something that looks like a saved project.
    fn slot_used(&mut self, slot: u8) -> bool;
    /// Writes `data` to the slot. Returns false if the write failed or was refused.
    fn save(&mut self, slot: u8, data: &[u8]) -> bool;
    /// Fills `buf` from the slot. Returns false if nothing could be read.
    fn load(&mut self, slot: u8, buf: &mut [u8]) -> bool;
}

pub struct Seq95 {
    pub project: Project,
    pub engine: Engine,
    ui: ui::Ui,
    scratch: [u8; PROJECT_WIRE_SIZE],
}

impl Seq95 {
    pub const fn new() -> Self {
        Self {
            project: Project::new(),
            engine: Engine::new(),
            ui: ui::Ui::new(),
            scratch: [0; PROJECT_WIRE_SIZE],
        }
    }

    /// Call every millisecond or so, also while the sequencer is not on screen.
    pub fn tick(&mut self, now_us: u64, out: &mut impl MidiOut) {
        self.ui.now_us = now_us;
        self.engine.tick(now_us, &self.project, out);
    }

    /// Feed incoming MIDI (clock, start/stop, transposition notes).
    /// Returns true if the sequencer consumed the message.
    pub fn midi_in(&mut self, msg: [u8; 3], out: &mut impl MidiOut) -> bool {
        self.engine.midi_in(msg, &mut self.project, out)
    }

    /// Called when the sequencer screen is shown.
    pub fn enter(&mut self) {
        self.ui.release_all();
    }

    /// Called when another mode takes over the surface. Playback continues.
    pub fn exit(&mut self, out: &mut impl MidiOut) {
        self.ui.release_all();
        self.ui.stop_preview(&self.project, out);
    }

    pub fn button(
        &mut self,
        index: u8,
        pressed: bool,
        velocity: u8,
        out: &mut impl MidiOut,
        storage: &mut impl Storage,
    ) {
        ui::button(self, index, pressed, velocity, out, storage);
    }

    /// Draws the whole surface into `frame` (RGB 0xRRGGBB per LED index).
    pub fn render(&self, frame: &mut [u32; LED_COUNT]) {
        ui::render(self, frame);
    }

    pub fn save_project(&mut self, slot: u8, storage: &mut impl Storage) -> bool {
        let len = self.project.encode(&mut self.scratch);
        storage.save(slot, &self.scratch[..len])
    }

    pub fn load_project(&mut self, slot: u8, storage: &mut impl Storage, out: &mut impl MidiOut) -> bool {
        if !storage.load(slot, &mut self.scratch) {
            return false;
        }
        match Project::decode(&self.scratch) {
            Ok(project) => {
                self.engine.all_notes_off(out);
                self.project = project;
                true
            }
            Err(_) => false,
        }
    }

    pub fn current_track(&self) -> usize {
        self.ui.track
    }
}

impl Default for Seq95 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
