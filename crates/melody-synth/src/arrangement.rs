//! Source-conditioned phrase composition, with independent harmony, meter and groove.
use anyhow::Result;
use melody_core::{
    Composition, Instrument as I, MonophonicMidi, MusicalIdentity, Note, ScaleKind, Section, Track,
};
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
fn choice(seed: u64, stream: u64, n: usize) -> usize {
    (mix(seed ^ stream.wrapping_mul(0x517cc1b727220a95)) % n as u64) as usize
}
fn fraction(seed: u64, n: u64) -> f32 {
    (mix(seed ^ n) >> 40) as f32 / 16777215.0
}
fn track(name: &str, instrument: I, channel: u8, gain: f32, pan: f32) -> Track {
    Track {
        name: name.into(),
        instrument,
        channel,
        gain,
        pan,
        notes: Vec::new(),
    }
}
fn push(t: &mut Track, pitch: i32, start: f32, length: f32, velocity: u8, total: f32) {
    let end = (start + length).min(total);
    if start >= 0.0 && end > start {
        t.notes.push(Note {
            pitch: pitch.clamp(0, 127) as u8,
            start,
            end,
            velocity: velocity.clamp(1, 127),
        });
    }
}
fn degree(root: i32, d: i32, steps: &[i32; 7]) -> i32 {
    root + steps[d.rem_euclid(7) as usize] + 12 * d.div_euclid(7)
}
fn fitted(mut pitch: i32, lo: i32, hi: i32) -> i32 {
    while pitch < lo {
        pitch += 12;
    }
    while pitch > hi {
        pitch -= 12;
    }
    pitch
}
/// A rotation of a maximally-even pulse distribution (Euclidean rhythm).
fn pulse(i: usize, slots: usize, hits: usize, rotation: usize) -> bool {
    let i = (i + rotation) % slots;
    ((i + 1) * hits) / slots != (i * hits) / slots
}
fn source_degree(pitch: u8, root: i32, scale: ScaleKind) -> i32 {
    let steps = match scale {
        ScaleKind::Major => [0, 2, 4, 5, 7, 9, 11],
        ScaleKind::Minor => [0, 2, 3, 5, 7, 8, 10],
    };
    (-14..=21)
        .min_by_key(|d| (degree(root, *d, &steps) - pitch as i32).abs())
        .unwrap()
}
pub fn arrange(
    source: &MonophonicMidi,
    scale: ScaleKind,
    root: i32,
    energy: f32,
    swing: f32,
) -> Result<Composition> {
    let seed = source.notes.iter().fold(0xcbf29ce484222325u64, |s, n| {
        mix(s ^ n.pitch as u64 ^ ((n.start.to_bits() as u64) << 16) ^ n.end.to_bits() as u64)
    });
    arrange_seeded(source, scale, root, energy, swing, seed)
}
pub fn arrange_seeded(
    source: &MonophonicMidi,
    scale: ScaleKind,
    root: i32,
    energy: f32,
    swing: f32,
    seed: u64,
) -> Result<Composition> {
    anyhow::ensure!(
        !source.notes.is_empty() && source.tempo_bpm > 0,
        "empty source score"
    );
    let source_end = source.notes.iter().map(|n| n.end).fold(0.0, f32::max);
    let seconds = source_end * 60.0 / source.tempo_bpm as f32;
    let profile = choice(seed, 1, 7);
    let names = [
        "Glass",
        "Broken",
        "Waltz",
        "Tidal",
        "Drive",
        "Suspended",
        "Chamber",
    ];
    let meters = [[4, 4], [4, 4], [3, 4], [6, 8], [4, 4], [5, 4], [4, 4]];
    let meter = meters[profile];
    let bar_beats = meter[0] as f32 * 4.0 / meter[1] as f32;
    let multipliers = [0.63, 0.95, 0.82, 0.88, 1.18, 0.74, 0.92];
    let tempo = ((source.tempo_bpm as f32 * multipliers[profile]) + fraction(seed, 3) * 18.0 - 9.0)
        .round()
        .clamp(52.0, 170.0) as u32;
    let total = seconds * tempo as f32 / 60.0;
    let bars = (total / bar_beats).ceil() as usize;
    let positive = matches!(scale, ScaleKind::Major);
    let modes = if positive {
        [
            [0, 2, 4, 5, 7, 9, 11],
            [0, 2, 4, 6, 7, 9, 11],
            [0, 2, 4, 5, 7, 9, 10],
        ]
    } else {
        [
            [0, 2, 3, 5, 7, 8, 10],
            [0, 2, 3, 5, 7, 9, 10],
            [0, 1, 3, 5, 7, 8, 10],
        ]
    };
    let mode_i = choice(seed, 4, 3);
    let steps = modes[mode_i];
    let mode_name = if positive {
        ["major", "lydian", "mixolydian"][mode_i]
    } else {
        ["minor", "dorian", "phrygian"][mode_i]
    };
    let source_root = root;
    let root = 48 + (root + choice(seed, 5, 12) as i32).rem_euclid(12);
    let leads = match profile {
        0 => [I::Bell, I::ElectricPiano, I::Flute],
        1 => [I::ElectricPiano, I::Marimba, I::Organ],
        2 => [I::Piano, I::Guitar, I::Flute],
        3 => [I::Guitar, I::Flute, I::Marimba],
        4 => [I::Brass, I::Organ, I::ElectricPiano],
        5 => [I::Flute, I::Bell, I::Strings],
        _ => [I::Piano, I::Flute, I::Strings],
    };
    let mut tracks = vec![
        track("Lead", leads[choice(seed, 6, 3)], 0, 0.78, -0.13),
        track(
            "Harmony",
            if profile == 4 {
                I::Organ
            } else if choice(seed, 7, 2) == 0 {
                I::Strings
            } else {
                I::WarmPad
            },
            1,
            if profile == 0 { 0.43 } else { 0.32 },
            0.34,
        ),
        track("Bass", I::Bass, 2, 0.62, 0.0),
        track(
            "Arpeggio",
            if choice(seed, 8, 2) == 0 {
                I::Guitar
            } else {
                I::Marimba
            },
            3,
            0.34,
            -0.52,
        ),
        track(
            "Countermelody",
            if choice(seed, 9, 2) == 0 {
                I::Flute
            } else {
                I::Bell
            },
            4,
            0.37,
            0.48,
        ),
        track("Keys", I::ElectricPiano, 5, 0.31, -0.24),
        track(
            "Percussion",
            I::Drums,
            9,
            if profile == 4 { 0.68 } else { 0.47 },
            0.0,
        ),
    ];
    let phrase_bars = [2, 3, 4][choice(seed, 10, 3)];
    let mut progression = vec![0i32];
    let plen = 3 + choice(seed, 11, 6);
    for j in 1..plen {
        let prev = *progression.last().unwrap();
        let options: &[i32] = match prev {
            0 => &[2, 3, 4, 5],
            2 => &[3, 5, 1],
            3 => &[0, 1, 4, 5],
            4 => &[0, 5, 6],
            5 => &[1, 3, 4, 6],
            _ => &[0, 3, 4],
        };
        progression.push(options[choice(seed, 100 + j as u64, options.len())]);
    }
    let forms: &[&str] = match choice(seed, 12, 3) {
        0 => &["Prelude", "Theme", "Response", "Return", "Coda"],
        1 => &["Opening", "Build", "Peak", "Afterglow"],
        _ => &["Exposition", "Interlude", "Development", "Resolution"],
    };
    let mut sections = Vec::new();
    for (i, name) in forms.iter().enumerate() {
        let start = (bars * i / forms.len()) as f32 * bar_beats;
        let end = ((bars * (i + 1) / forms.len()) as f32 * bar_beats).min(total);
        if end > start {
            sections.push(Section {
                name: (*name).into(),
                start,
                end,
            });
        }
    }
    let motif_len = 7 + choice(seed, 13, 10);
    let mut motif = Vec::new();
    let mut cur = 0;
    for i in 0..motif_len {
        let n = &source.notes[(i * source.notes.len() / motif_len).min(source.notes.len() - 1)];
        let raw = source_degree(n.pitch, source_root, scale).rem_euclid(7);
        let delta = (raw - cur).clamp(-3, 3);
        cur = (cur + delta).clamp(-3, 10);
        motif.push(cur);
    }
    let subdivision = if profile == 0 || profile == 2 || profile == 6 {
        0.5
    } else {
        0.25
    };
    let slots = (bar_beats / subdivision).round() as usize;
    let melody_hits = match profile {
        0 => 2 + choice(seed, 14, 3),
        2 => 3 + choice(seed, 14, 3),
        4 => 7 + choice(seed, 14, 5),
        _ => 4 + choice(seed, 14, 5),
    }
    .min(slots);
    let rotation = choice(seed, 15, slots);
    let bass_hits = 2 + choice(seed, 16, 3);
    let arp_hits = 3 + choice(seed, 17, slots.saturating_sub(3).max(1));
    let chord_span = if profile == 0 || profile == 5 {
        2
    } else {
        1 + choice(seed, 18, 2)
    };
    let mut voicing = [root, root + 4, root + 7, root + 11];
    for bar in 0..bars {
        let b = bar as f32 * bar_beats;
        let progress = b / total;
        let phrase = bar / phrase_bars;
        let sparse = progress < 0.12 || progress > 0.88 || (profile != 4 && phrase % 4 == 2);
        let level = (if sparse { 0.7 } else { 0.94 }) * (0.85 + energy.clamp(0.0, 1.0) * 0.15);
        let chord = if bar + 1 == bars {
            0
        } else {
            progression[(bar / chord_span) % progression.len()]
        };
        let voices = if profile == 0 || choice(seed, 19, 2) == 0 {
            4
        } else {
            3
        };
        for voice in 0..voices {
            let p = degree(root, chord + voice as i32 * 2, &steps);
            let pitch = (-3..=3)
                .map(|oct| p + oct * 12)
                .filter(|p| (46..=78).contains(p))
                .min_by_key(|p| (p - voicing[voice]).abs())
                .unwrap_or(p);
            voicing[voice] = pitch;
            if bar % chord_span == 0 || bar + 1 == bars {
                push(
                    &mut tracks[1],
                    pitch,
                    b,
                    bar_beats * chord_span.min(bars.saturating_sub(1 + bar).max(1)) as f32 * 0.97,
                    (67.0 * level) as u8,
                    total,
                );
            }
            if profile == 2 || profile == 6 || (!sparse && profile == 1) {
                for beat in 0..bar_beats.ceil() as usize {
                    if beat % 2 == 0 || profile == 2 {
                        push(
                            &mut tracks[5],
                            pitch,
                            b + beat as f32 + if profile == 2 { 0.0 } else { 0.25 },
                            0.48,
                            (61.0 * level) as u8,
                            total,
                        );
                    }
                }
            }
        }
        let mut onsets: Vec<f32> = (0..slots)
            .filter(|i| {
                pulse(
                    *i,
                    slots,
                    if sparse {
                        melody_hits.saturating_sub(1).max(1)
                    } else {
                        melody_hits
                    },
                    rotation + phrase % 3,
                )
            })
            .map(|i| i as f32 * subdivision)
            .collect();
        if onsets.is_empty() {
            onsets.push(0.0);
        }
        for (j, &offset) in onsets.iter().enumerate() {
            let source_index = (bar * source.notes.len() / bars.max(1)
                + j * source.notes.len() / slots.max(1))
                % source.notes.len();
            let src = &source.notes[source_index];
            let mut d = motif[(j + bar % phrase_bars * 2) % motif.len()];
            if phrase % 3 == 1 {
                d = 4 - d;
            }
            if phrase % 4 == 3 {
                d += 2;
            }
            d += chord;
            if (offset % 1.0) < 0.01 {
                d = chord + [0, 2, 4][choice(seed, 300 + source_index as u64, 3)];
            }
            if bar + 1 == bars && j + 1 == onsets.len() {
                d = 0;
            }
            let pitch = fitted(degree(root + 12, d, &steps), 60, 91);
            let next = onsets.get(j + 1).copied().unwrap_or(bar_beats);
            let dur = (next - offset) * (0.62 + fraction(seed, 500 + source_index as u64) * 0.31);
            let shift = if j % 2 == 1 {
                (swing + if profile == 1 { 0.12 } else { 0.0 }).min(0.3) * subdivision * 0.35
            } else {
                0.0
            };
            let timing = (fraction(seed, 800 + bar as u64 * 31 + j as u64) - 0.5) * 0.018;
            push(
                &mut tracks[0],
                pitch,
                (b + offset + shift + timing).max(0.0),
                dur.max(0.08),
                ((72.0 + src.velocity as f32 * 0.2) * level) as u8,
                total,
            );
            if (profile == 6 || (!sparse && phrase % 2 == 1)) && j % 3 == 1 {
                push(
                    &mut tracks[4],
                    fitted(
                        degree(root + 12, chord + 4 - d.rem_euclid(5), &steps),
                        62,
                        89,
                    ),
                    b + offset + subdivision * 0.5,
                    dur * 0.75,
                    58,
                    total,
                );
            }
        }
        let bass_slots = if profile == 2 {
            3
        } else {
            (bar_beats * 2.0).round() as usize
        };
        for j in 0..bass_slots {
            if j == 0
                || (!sparse
                    && pulse(
                        j,
                        bass_slots,
                        bass_hits.min(bass_slots),
                        choice(seed, 20, bass_slots),
                    ))
            {
                let d = chord
                    + if profile == 2 && j > 0 {
                        4
                    } else if j + 1 == bass_slots {
                        choice(seed, bar as u64 + 1000, 5) as i32
                    } else {
                        0
                    };
                push(
                    &mut tracks[2],
                    fitted(degree(root - 12, d, &steps), 31, 55),
                    b + j as f32 * bar_beats / bass_slots as f32,
                    if profile == 0 {
                        bar_beats * 0.8
                    } else {
                        bar_beats / bass_slots as f32 * 0.86
                    },
                    (87.0 * level) as u8,
                    total,
                );
            }
        }
        if profile != 5 || bar % 2 == 0 {
            for j in 0..slots {
                if pulse(
                    j,
                    slots,
                    if sparse { arp_hits / 2 + 1 } else { arp_hits }.min(slots),
                    choice(seed, 21, slots),
                ) {
                    let order = match profile {
                        2 => [0, 2, 4, 2, 4, 6],
                        3 => [0, 4, 2, 6, 4, 2],
                        _ => [0, 2, 4, 6, 2, 4],
                    };
                    let d = chord + order[(j + phrase) % order.len()];
                    push(
                        &mut tracks[3],
                        fitted(degree(root, d, &steps), 48, 83),
                        b + j as f32 * subdivision,
                        subdivision * 0.72,
                        (58.0 * level) as u8,
                        total,
                    );
                }
            }
        }
        if profile != 0 && profile != 5 && (!sparse || bar > 0) {
            let drum_slots = (bar_beats * 4.0).round() as usize;
            let kick_hits = if profile == 4 {
                4
            } else {
                2 + choice(seed, 22, 3)
            }
            .min(drum_slots);
            let hat_hits = (drum_slots / 2 + choice(seed, 23, drum_slots / 2)).min(drum_slots);
            for j in 0..drum_slots {
                let beat = b + j as f32 * 0.25;
                if pulse(j, drum_slots, hat_hits, choice(seed, 24, drum_slots)) {
                    push(
                        &mut tracks[6],
                        if j + 2 >= drum_slots && phrase % 2 == 1 {
                            46
                        } else {
                            42
                        },
                        beat,
                        0.12,
                        if j % 4 == 0 { 61 } else { 38 },
                        total,
                    );
                }
                if j == 0 || pulse(j, drum_slots, kick_hits, choice(seed, 25, drum_slots)) {
                    push(&mut tracks[6], 36, beat, 0.18, 95, total);
                }
                let snare = if meter == [3, 4] {
                    j == 4 || j == 10
                } else if meter == [6, 8] {
                    j == 6
                } else {
                    j % 8 == 4
                };
                if snare {
                    push(&mut tracks[6], 38, beat, 0.18, 84, total);
                    if profile == 4 {
                        push(&mut tracks[6], 39, beat + 0.015, 0.12, 48, total);
                    }
                }
                if bar % phrase_bars + 1 == phrase_bars
                    && j > drum_slots.saturating_sub(4)
                    && !sparse
                {
                    push(
                        &mut tracks[6],
                        45 + ((j + choice(seed, 26, 3)) % 4) as i32,
                        beat,
                        0.14,
                        65 + (j % 4) as u8 * 5,
                        total,
                    );
                }
            }
        }
    }
    tracks.retain(|t| !t.notes.is_empty());
    let notes = tracks
        .iter()
        .filter(|t| t.instrument != I::Drums)
        .flat_map(|t| t.notes.clone())
        .collect();
    let key_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let score = Composition {
        meter,
        identity: Some(MusicalIdentity {
            seed: seed.to_string(),
            character: names[profile].into(),
            mode: mode_name.into(),
            phrase_bars: phrase_bars as u8,
            progression,
            space: if profile == 0 || profile == 5 {
                0.13
            } else {
                0.045 + fraction(seed, 27) * 0.045
            },
        }),
        tempo_bpm: tempo,
        key: format!("{} {}", key_names[root.rem_euclid(12) as usize], mode_name),
        duration_beats: total,
        sections,
        tracks,
        notes,
    };
    score.validate()?;
    Ok(score)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> MonophonicMidi {
        let mut s = MonophonicMidi::new(120);
        for i in 0..64 {
            s.push(60 + (i % 7) as u8, i as f32 * 0.5, (i + 1) as f32 * 0.5, 85);
        }
        s
    }
    #[test]
    fn midi_tracks_and_duration_follow_the_actual_score() {
        let s = arrange_seeded(&source(), ScaleKind::Major, 60, 0.8, 0.15, 42).unwrap();
        let bytes = s.to_mid_bytes().unwrap();
        let midi = midly::Smf::parse(&bytes).unwrap();
        assert_eq!(midi.tracks.len(), s.tracks.len() + 1);
        assert!((s.duration_beats * 60.0 / s.tempo_bpm as f32 - 16.0).abs() < 0.001);
        for t in midi.tracks {
            assert_eq!(
                t.iter().map(|e| e.delta.as_int()).sum::<u32>(),
                (s.duration_beats * 480.0).round() as u32
            );
        }
    }
    #[test]
    fn source_identities_vary_pitch_intervals_rhythm_and_meter() {
        let mut intervals = std::collections::HashSet::new();
        let mut rhythms = std::collections::HashSet::new();
        let mut meters = std::collections::HashSet::new();
        for seed in 0..16 {
            let s = arrange_seeded(&source(), ScaleKind::Major, 60, 0.7, 0.1, mix(seed)).unwrap();
            let lead = &s.tracks[0].notes;
            intervals.insert(
                lead.windows(2)
                    .take(20)
                    .map(|w| w[1].pitch as i16 - w[0].pitch as i16)
                    .collect::<Vec<_>>(),
            );
            rhythms.insert(
                lead.windows(2)
                    .take(20)
                    .map(|w| ((w[1].start - w[0].start) * 100.0).round() as i32)
                    .collect::<Vec<_>>(),
            );
            meters.insert(s.meter);
        }
        assert!(intervals.len() >= 14);
        assert!(rhythms.len() >= 14);
        assert!(meters.len() >= 3);
    }
    #[test]
    fn euclidean_pulses_are_evenly_spaced() {
        let on: Vec<_> = (0..16).filter(|i| pulse(*i, 16, 5, 0)).collect();
        assert_eq!(on.len(), 5);
        let mut gaps = Vec::new();
        for i in 0..on.len() {
            gaps.push((on[(i + 1) % on.len()] + 16 - on[i]) % 16);
        }
        assert!(gaps.iter().max().unwrap() - gaps.iter().min().unwrap() <= 1);
    }
}
