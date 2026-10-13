// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 launchpad-Pro-Mod contributors

//! Seq95 step sequencer mode (Launchpad Pro Mk3). The sequencer core lives in
//! the `seq95` crate; this file connects it to the CoreFW surface, USB MIDI and
//! external flash. Playback keeps running while other modes are on screen.

use crate::app::apptrait::App;
use crate::app::events::{AftertouchEvent, MidiEvent, SurfaceEvent};
use crate::driver;
use crate::sys::led;
use crate::sys::midi::MidiPort;
use seq95::{LED_COUNT, MidiOut, PORT_USB_DAW, PORT_USB_MIDI, Seq95, Storage};

struct DriverOut;

impl MidiOut for DriverOut {
    fn send(&mut self, ports: u8, msg: &[u8]) {
        if ports & PORT_USB_MIDI != 0 {
            driver::send_midi(MidiPort::Midi, msg);
        }
        if ports & PORT_USB_DAW != 0 {
            driver::send_midi(MidiPort::Daw, msg);
        }
        // PORT_DIN: the Mk3 TRS MIDI output is not implemented in CoreFW yet.
    }
}

struct FlashStorage;

impl Storage for FlashStorage {
    fn slot_used(&mut self, slot: u8) -> bool {
        let mut header = [0xffu8; 8];
        driver::read_project_slot(slot, 0, &mut header) && seq95::looks_like_project(&header)
    }

    fn save(&mut self, slot: u8, data: &[u8]) -> bool {
        driver::write_project_slot(slot, data, seq95::looks_like_project)
    }

    fn load(&mut self, slot: u8, buf: &mut [u8]) -> bool {
        driver::read_project_slot(slot, 0, buf)
    }
}

pub struct Seq95App {
    seq: Seq95,
    visible: bool,
    shown: [u32; LED_COUNT],
    frame: [u32; LED_COUNT],
}

impl Seq95App {
    pub const fn new() -> Self {
        Self {
            seq: Seq95::new(),
            visible: false,
            shown: [u32::MAX; LED_COUNT],
            frame: [0; LED_COUNT],
        }
    }

    /// Runs the clock. Called every host tick whichever mode is on screen.
    pub fn background_tick(&mut self) {
        let now = embassy_time::Instant::now().as_micros();
        self.seq.tick(now, &mut DriverOut);
    }

    /// Clock, transport and transposition input. Returns true if the message
    /// must not reach the active mode (transposition notes).
    pub fn midi_in(&mut self, event: &MidiEvent) -> bool {
        let used = self
            .seq
            .midi_in([event.status, event.data1, event.data2], &mut DriverOut);
        used && event.status < 0xf0
    }

    fn draw(&mut self) {
        self.seq.render(&mut self.frame);
        for index in 1..LED_COUNT {
            // 9, 99 and 100 are not LEDs.
            if matches!(index, 9 | 99 | 100) {
                continue;
            }
            if self.frame[index] != self.shown[index] {
                self.shown[index] = self.frame[index];
                led::set(index as u8, self.frame[index]);
            }
        }
    }
}

impl App for Seq95App {
    fn on_enter(&mut self) {
        self.visible = true;
        self.shown = [u32::MAX; LED_COUNT];
        self.seq.enter();
        self.seq.engine.seed(embassy_time::Instant::now().as_ticks() as u32);
        self.draw();
    }

    fn on_exit(&mut self) {
        self.visible = false;
        self.seq.exit(&mut DriverOut);
    }

    fn on_surface(&mut self, event: SurfaceEvent) {
        self.seq.button(
            event.index,
            event.pressed,
            event.value,
            &mut DriverOut,
            &mut FlashStorage,
        );
    }

    fn on_midi(&mut self, _event: MidiEvent) {}

    fn on_aftertouch(&mut self, _event: AftertouchEvent) {}

    fn on_tick(&mut self) {
        if self.visible {
            self.draw();
        }
    }
}
