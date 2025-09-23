
use serde::{Deserialize, Serialize};

/// MIDI <-> frequency helpers
pub fn hz_to_midi(hz: f32) -> f32 { 69.0 + 12.0 * (hz / 440.0).log2() }
pub fn midi_to_hz(m: f32) -> f32 { 440.0 * 2f32.powf((m - 69.0) / 12.0) }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note { pub pitch: u8, pub start: f32, pub end: f32, pub velocity: u8 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonophonicMidi { pub notes: Vec<Note>, pub tempo_bpm: u32 }

impl MonophonicMidi {
    pub fn new(tempo_bpm: u32) -> Self { Self { notes: Vec::new(), tempo_bpm } }
    pub fn push(&mut self, pitch: u8, start: f32, end: f32, vel: u8) {
        self.notes.push(Note { pitch, start, end, velocity: vel });
    }

    /// Serialize to SMF bytes (single track), simple delta timing.
    pub fn to_mid_bytes(&self) -> anyhow::Result<Vec<u8>> {
        use midly::{
            Smf, Header, Format, Timing, TrackEvent, TrackEventKind, MetaMessage, MidiMessage,
            num::{u4, u7}
        };
        let ppq: u16 = 480;
        let micros_per_quarter = (60_000_000u32 / self.tempo_bpm) as u32;

        // sortăm evenimentele
        let mut evs: Vec<(f32, bool, &Note)> = Vec::new();
        for n in &self.notes {
            evs.push((n.start, true, n));
            evs.push((n.end, false, n));
        }
        evs.sort_by(|a,b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut track: Vec<TrackEvent> = Vec::new();
        // tempo (u24)
        track.push(TrackEvent {
            delta: 0.into(),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(micros_per_quarter.into())),
        });

        // delte de timp
        let mut last_tick: u32 = 0;
        for (t_sec, is_on, n) in evs {
            let tick = (t_sec.max(0.0) * ppq as f32) as u32;
            let delta = tick.saturating_sub(last_tick);
            last_tick = tick;
            let kind = if is_on {
                TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOn {
                        key: u7::new(n.pitch.min(127)),
                        vel: u7::new(n.velocity.min(127)),
                    },
                }
            } else {
                TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOff {
                        key: u7::new(n.pitch.min(127)),
                        vel: u7::new(0),
                    },
                }
            };
            track.push(TrackEvent { delta: delta.into(), kind });
        }

        let smf = Smf {
            header: Header {
                format: Format::SingleTrack,
                timing: Timing::Metrical(ppq.into()), // u15
            },
            tracks: vec![track],
        };

        let mut buf = Vec::new();
        smf.write(&mut buf).map_err(|e| anyhow::anyhow!(e))?;
        Ok(buf)
    }
}

/// Simple scale machinery
#[derive(Copy, Clone, Debug)]
pub enum ScaleKind { Major, Minor }

/// Return semitone steps for diatonic degrees 0..6 for the given scale
pub fn scale_steps(scale: ScaleKind) -> [i32;7] {
    match scale {
        ScaleKind::Major => [0,2,4,5,7,9,11],
        ScaleKind::Minor => [0,2,3,5,7,8,10], // natural minor
    }
}

/// Map (root MIDI, diatonic degree index possibly >6) to absolute MIDI pitch, across octaves
pub fn degree_to_midi(root: i32, degree: i32, scale: ScaleKind) -> i32 {
    let steps = scale_steps(scale);
    let octave = degree.div_euclid(7);
    let idx = degree.rem_euclid(7) as usize;
    root + steps[idx] + 12 * octave
}
