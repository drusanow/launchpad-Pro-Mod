// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 launchpad-Pro-Mod contributors

use super::*;
use std::collections::HashMap;
use std::vec::Vec;

#[derive(Default)]
struct Recorder {
    sent: Vec<(u8, Vec<u8>)>,
}

impl MidiOut for Recorder {
    fn send(&mut self, ports: u8, msg: &[u8]) {
        self.sent.push((ports, msg.to_vec()));
    }
}

impl Recorder {
    fn note_ons(&self) -> Vec<(u8, u8, u8)> {
        self.sent
            .iter()
            .filter(|(_, m)| m.len() == 3 && m[0] & 0xf0 == 0x90 && m[2] > 0)
            .map(|(_, m)| (m[0] & 0x0f, m[1], m[2]))
            .collect()
    }
    fn note_offs(&self) -> usize {
        self.sent
            .iter()
            .filter(|(_, m)| m.len() == 3 && m[0] & 0xf0 == 0x80)
            .count()
    }
}

#[derive(Default)]
struct MemStorage {
    slots: HashMap<u8, Vec<u8>>,
}

impl Storage for MemStorage {
    fn slot_used(&mut self, slot: u8) -> bool {
        self.slots.contains_key(&slot)
    }
    fn save(&mut self, slot: u8, data: &[u8]) -> bool {
        self.slots.insert(slot, data.to_vec());
        true
    }
    fn load(&mut self, slot: u8, buf: &mut [u8]) -> bool {
        match self.slots.get(&slot) {
            Some(d) => {
                buf[..d.len()].copy_from_slice(d);
                true
            }
            None => false,
        }
    }
}

/// Run the internal clock for `pulses` pulses at the project tempo.
fn run_pulses(seq: &mut Seq95, out: &mut Recorder, start_us: u64, pulses: u32) -> u64 {
    let per_pulse = 2_500_000 / seq.project.bpm as u64;
    let mut now = start_us;
    for _ in 0..pulses {
        now += per_pulse + 1;
        seq.tick(now, out);
    }
    now
}

fn press(seq: &mut Seq95, index: u8, out: &mut Recorder, storage: &mut MemStorage) {
    seq.button(index, true, 100, out, storage);
    seq.button(index, false, 0, out, storage);
}

#[test]
fn project_round_trips_through_wire_format() {
    let mut p = Project::new();
    p.bpm = 133;
    p.root = 7;
    p.scale = 10;
    p.transpose = -5;
    p.clock = ClockSource::External;
    p.send_clock = true;
    p.tracks[2].mel[17] = MelStep {
        on: true,
        muted: true,
        degree: 4,
        octave: 1,
        velocity: 2,
        length: 5,
        probability: 3,
    };
    p.tracks[0].drum[3][63].set_on(true);
    p.tracks[0].drum[3][63].set_probability(2);
    p.tracks[4].set_loop(6, 2);
    p.tracks[1].voice_mutes = 0x8001;

    let mut buf = vec![0u8; PROJECT_WIRE_SIZE];
    assert_eq!(p.encode(&mut buf), PROJECT_WIRE_SIZE);
    let q = Project::decode(&buf).unwrap();
    assert_eq!(p, q);
    assert_eq!(q.tracks[4].loop_start, 2);
    assert_eq!(q.tracks[4].loop_end, 6);
}

#[test]
fn corrupted_project_is_rejected() {
    let p = Project::new();
    let mut buf = vec![0u8; PROJECT_WIRE_SIZE];
    p.encode(&mut buf);
    buf[PROJECT_WIRE_SIZE - 1] ^= 1;
    assert_eq!(Project::decode(&buf), Err(DecodeError::BadChecksum));
    let erased = vec![0xffu8; PROJECT_WIRE_SIZE];
    assert_eq!(Project::decode(&erased), Err(DecodeError::BadMagic));
}

#[test]
fn project_fits_two_flash_sectors() {
    assert!(PROJECT_WIRE_SIZE <= 8192);
}

#[test]
fn melodic_track_plays_scale_notes_with_transpose() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let t = &mut seq.project.tracks[1];
    t.mel[0].on = true;
    t.mel[0].degree = 2; // E in C major
    t.mel[1].on = true;
    t.mel[1].degree = 0;
    t.mel[1].octave = OCTAVE_CENTER + 1;
    seq.project.transpose = 2;

    seq.engine.start(&seq.project, &mut out);
    run_pulses(&mut seq, &mut out, 0, 12);
    let ons = out.note_ons();
    // base octave 3 -> C3 = 48
    assert_eq!(ons, vec![(0, 48 + 4 + 2, 127), (0, 48 + 12 + 2, 127)]);
}

#[test]
fn drum_track_plays_each_voice_on_its_note() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    seq.project.tracks[0].drum[0][0].set_on(true);
    seq.project.tracks[0].drum[2][0].set_on(true);
    seq.project.tracks[0].drum[1][1].set_on(true);
    seq.project.tracks[0].voice_mutes = 1 << 2;

    seq.engine.start(&seq.project, &mut out);
    run_pulses(&mut seq, &mut out, 0, 12);
    assert_eq!(out.note_ons(), vec![(9, 36, 127), (9, 37, 127)]);
}

#[test]
fn notes_are_released_after_gate_and_on_stop() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let t = &mut seq.project.tracks[1];
    t.mel[0].on = true;
    t.mel[0].length = 6; // 8 steps
    seq.engine.start(&seq.project, &mut out);
    run_pulses(&mut seq, &mut out, 0, 3);
    assert_eq!(seq.engine.sounding_notes(), 1);
    seq.engine.stop(&seq.project, &mut out);
    assert_eq!(seq.engine.sounding_notes(), 0);
    assert_eq!(out.note_offs(), 1);

    let mut out = Recorder::default();
    seq.project.tracks[1].mel[0].length = 3; // one step = 6 pulses
    seq.engine.start(&seq.project, &mut out);
    run_pulses(&mut seq, &mut out, 1_000_000, 7);
    assert_eq!(out.note_offs(), 1);
    assert_eq!(seq.engine.sounding_notes(), 0);
}

#[test]
fn loop_range_wraps_playback() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let t = &mut seq.project.tracks[1];
    t.set_loop(1, 1);
    t.mel[8].on = true;
    seq.engine.start(&seq.project, &mut out);
    // 2 loops of 8 sixteenth steps
    run_pulses(&mut seq, &mut out, 0, 16 * 6 - 1);
    assert_eq!(out.note_ons().len(), 2);
    assert!(seq.engine.playing_step(1).unwrap() >= 8);
}

#[test]
fn probability_thins_out_hits() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let t = &mut seq.project.tracks[0];
    t.set_loop(0, 7);
    for step in 0..STEPS {
        t.drum[0][step].set_on(true);
        t.drum[0][step].set_probability(3); // 50 %
    }
    seq.engine.start(&seq.project, &mut out);
    run_pulses(&mut seq, &mut out, 0, 6 * 64 * 4);
    let n = out.note_ons().len();
    assert!(n > 64 && n < 192, "{n} hits out of 256");
}

#[test]
fn mute_and_solo() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    seq.project.tracks[0].drum[0][0].set_on(true);
    seq.project.tracks[1].mel[0].on = true;
    seq.project.tracks[2].mel[0].on = true;
    seq.project.tracks[2].solo = true;
    seq.engine.start(&seq.project, &mut out);
    run_pulses(&mut seq, &mut out, 0, 2);
    assert_eq!(out.note_ons().len(), 1);
    assert_eq!(out.note_ons()[0].0, 1);
}

#[test]
fn external_clock_drives_playback() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    seq.project.clock = ClockSource::External;
    seq.project.tracks[1].mel[0].on = true;
    seq.project.tracks[1].mel[1].on = true;

    // Internal ticks do nothing in external mode.
    seq.engine.start(&seq.project, &mut out);
    run_pulses(&mut seq, &mut out, 0, 50);
    assert!(out.note_ons().is_empty());

    assert!(seq.midi_in([0xfa, 0, 0], &mut out));
    for _ in 0..7 {
        seq.midi_in([0xf8, 0, 0], &mut out);
    }
    assert_eq!(out.note_ons().len(), 2);
    seq.midi_in([0xfc, 0, 0], &mut out);
    assert!(!seq.engine.running());
}

#[test]
fn incoming_notes_transpose_on_the_transpose_channel() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    assert!(seq.midi_in([0x9f, 63, 100], &mut out));
    assert_eq!(seq.project.transpose, 3);
    assert!(!seq.midi_in([0x90, 50, 100], &mut out));
    assert_eq!(seq.project.transpose, 3);
}

#[test]
fn internal_clock_sends_midi_clock_when_enabled() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    seq.project.send_clock = true;
    seq.engine.start(&seq.project, &mut out);
    // The first pulse plays at the start time, so 23 pulse periods give 24 clocks.
    run_pulses(&mut seq, &mut out, 0, 23);
    let clocks = out.sent.iter().filter(|(_, m)| m == &vec![0xf8]).count();
    assert_eq!(clocks, 24);
    assert_eq!(out.sent[0].1, vec![0xfa]);
}

#[test]
fn clock_does_not_burst_after_a_stall() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    seq.project.send_clock = true;
    seq.engine.start(&seq.project, &mut out);
    seq.tick(10_000_000, &mut out);
    let clocks = out.sent.iter().filter(|(_, m)| m == &vec![0xf8]).count();
    assert!(clocks <= 4);
}

#[test]
fn surface_editing_melodic_steps() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let mut st = MemStorage::default();
    press(&mut seq, 2, &mut out, &mut st); // select track 2 (melodic)
    press(&mut seq, 53, &mut out, &mut st); // row 5 -> degree 3, step 2
    let s = seq.project.tracks[1].mel[2];
    assert!(s.on);
    assert_eq!(s.degree, 3);
    press(&mut seq, 53, &mut out, &mut st);
    assert!(!seq.project.tracks[1].mel[2].on);

    press(&mut seq, 29, &mut out, &mut st); // velocity lane
    press(&mut seq, 31, &mut out, &mut st); // level 1, step 0
    assert_eq!(seq.project.tracks[1].mel[0].velocity, 1);

    // page selector: view block 3, then hold 3 + press 5 sets loop 3..5
    seq.button(14, true, 100, &mut out, &mut st);
    seq.button(16, true, 100, &mut out, &mut st);
    seq.button(16, false, 0, &mut out, &mut st);
    seq.button(14, false, 0, &mut out, &mut st);
    assert_eq!((seq.project.tracks[1].loop_start, seq.project.tracks[1].loop_end), (3, 5));
    press(&mut seq, 19, &mut out, &mut st); // back to the notes lane
    press(&mut seq, 88, &mut out, &mut st); // row 8 -> degree 6, step 3*8+7
    assert!(seq.project.tracks[1].mel[31].on);
}

#[test]
fn surface_editing_drum_steps_and_voices() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let mut st = MemStorage::default();
    // Voice pad 2 (row 1, col 2) selects voice 1 and previews note 37.
    press(&mut seq, 12, &mut out, &mut st);
    assert_eq!(out.note_ons(), vec![(9, 37, 100)]);
    assert_eq!(out.note_offs(), 1);
    // Top-left step pad = step 0.
    seq.button(81, true, 40, &mut out, &mut st);
    let hit = seq.project.tracks[0].drum[1][0];
    assert!(hit.on());
    assert_eq!(hit.velocity(), 2);
    // Probability hold cycles down from 100%.
    seq.button(79, true, 100, &mut out, &mut st);
    press(&mut seq, 81, &mut out, &mut st);
    seq.button(79, false, 0, &mut out, &mut st);
    assert_eq!(seq.project.tracks[0].drum[1][0].probability(), LEVELS - 2);
    // Row 5 col 8 = step 31.
    press(&mut seq, 58, &mut out, &mut st);
    assert!(seq.project.tracks[0].drum[1][31].on());
}

#[test]
fn transpose_controls() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let mut st = MemStorage::default();
    press(&mut seq, 80, &mut out, &mut st);
    assert_eq!(seq.project.transpose, 1);
    seq.button(90, true, 100, &mut out, &mut st);
    press(&mut seq, 70, &mut out, &mut st);
    seq.button(90, false, 0, &mut out, &mut st);
    assert_eq!(seq.project.transpose, -11);
    // Hold record and play the transpose keyboard: row 2 col 3 = D#.
    seq.button(10, true, 100, &mut out, &mut st);
    press(&mut seq, 23, &mut out, &mut st);
    assert_eq!(seq.project.transpose, 3);
    press(&mut seq, 31, &mut out, &mut st);
    assert_eq!(seq.project.transpose, -12);
    seq.button(10, false, 0, &mut out, &mut st);
}

#[test]
fn projects_save_and_load() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let mut st = MemStorage::default();
    seq.project.bpm = 99;
    seq.project.tracks[3].mel[5].on = true;
    press(&mut seq, 98, &mut out, &mut st); // open projects
    press(&mut seq, 73, &mut out, &mut st); // save slot 3
    assert!(st.slots.contains_key(&2));
    press(&mut seq, 58, &mut out, &mut st); // new project
    assert_eq!(seq.project.bpm, 120);
    press(&mut seq, 83, &mut out, &mut st); // load slot 3
    assert_eq!(seq.project.bpm, 99);
    assert!(seq.project.tracks[3].mel[5].on);
    press(&mut seq, 84, &mut out, &mut st); // load empty slot fails
    assert_eq!(seq.project.bpm, 99);
}

#[test]
fn settings_overlay() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let mut st = MemStorage::default();
    seq.button(90, true, 100, &mut out, &mut st);
    press(&mut seq, 72, &mut out, &mut st); // row 7 col 2 -> MIDI channel 10 (index 9)
    press(&mut seq, 62, &mut out, &mut st); // melodic
    press(&mut seq, 53, &mut out, &mut st); // rate 1/8
    press(&mut seq, 42, &mut out, &mut st); // external clock
    press(&mut seq, 15, &mut out, &mut st); // 120 bpm preset
    seq.button(90, false, 0, &mut out, &mut st);
    let t = &seq.project.tracks[0];
    assert_eq!(t.channel, 9);
    assert_eq!(t.kind, TrackKind::Melodic);
    assert_eq!(t.rate, 2);
    assert_eq!(seq.project.clock, ClockSource::External);
    assert_eq!(seq.project.bpm, 120);
}

#[test]
fn render_covers_every_led_without_panicking() {
    let mut seq = Seq95::new();
    let mut out = Recorder::default();
    let mut st = MemStorage::default();
    let mut frame = [0u32; LED_COUNT];
    for track in 1..=5 {
        press(&mut seq, track, &mut out, &mut st);
        seq.render(&mut frame);
        for overlay in [89u8, 98] {
            press(&mut seq, overlay, &mut out, &mut st);
            seq.render(&mut frame);
            press(&mut seq, overlay, &mut out, &mut st);
        }
        seq.button(90, true, 1, &mut out, &mut st);
        seq.render(&mut frame);
        seq.button(90, false, 0, &mut out, &mut st);
        seq.button(10, true, 1, &mut out, &mut st);
        seq.render(&mut frame);
        seq.button(10, false, 0, &mut out, &mut st);
    }
    // Every grid pad and button reacts without panicking.
    for index in 0..LED_COUNT as u8 {
        seq.button(index, true, 64, &mut out, &mut st);
        seq.button(index, false, 0, &mut out, &mut st);
    }
    seq.render(&mut frame);
}

#[test]
fn duplicate_extends_loop() {
    let mut t = Track::new(TrackKind::Melodic, 0);
    t.mel[3].on = true;
    t.duplicate_loop();
    assert_eq!(t.loop_end, 3);
    assert!(t.mel[19].on);
}
