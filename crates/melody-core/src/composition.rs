//! Multi-track score shared by the arranger, synthesizer and MIDI exporter.
use crate::Note;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Instrument {
    Piano,
    ElectricPiano,
    Marimba,
    Bell,
    Guitar,
    Strings,
    WarmPad,
    Flute,
    Organ,
    Brass,
    Bass,
    Drums,
}
impl Instrument {
    pub fn program(self) -> u8 {
        match self {
            Self::Piano => 0,
            Self::ElectricPiano => 4,
            Self::Marimba => 12,
            Self::Bell => 10,
            Self::Guitar => 24,
            Self::Strings => 48,
            Self::WarmPad => 89,
            Self::Flute => 73,
            Self::Organ => 16,
            Self::Brass => 61,
            Self::Bass => 33,
            Self::Drums => 0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub instrument: Instrument,
    pub channel: u8,
    pub gain: f32,
    pub pan: f32,
    pub notes: Vec<Note>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub start: f32,
    pub end: f32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MusicalIdentity {
    pub seed: String,
    pub character: String,
    pub mode: String,
    pub phrase_bars: u8,
    pub progression: Vec<i32>,
    pub space: f32,
}
fn default_meter() -> [u8; 2] {
    [4, 4]
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Composition {
    #[serde(default = "default_meter")]
    pub meter: [u8; 2],
    #[serde(default)]
    pub identity: Option<MusicalIdentity>,
    pub tempo_bpm: u32,
    pub key: String,
    pub duration_beats: f32,
    pub sections: Vec<Section>,
    pub tracks: Vec<Track>,
    /// Pitched notes flattened for older piano-roll readers; drums stay in tracks.
    pub notes: Vec<Note>,
}
impl Composition {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            (30..=240).contains(&self.tempo_bpm),
            "tempo must be 30..240 BPM"
        );
        anyhow::ensure!(
            self.duration_beats.is_finite()
                && self.duration_beats > 0.0
                && self.duration_beats * 60.0 / self.tempo_bpm as f32 <= 181.0,
            "invalid score duration"
        );
        anyhow::ensure!(
            !self.tracks.is_empty() && self.tracks.len() <= 16,
            "invalid track count"
        );
        anyhow::ensure!(
            (2..=7).contains(&self.meter[0]) && [4, 8].contains(&self.meter[1]),
            "invalid meter"
        );
        let mut channels = std::collections::HashSet::new();
        for track in &self.tracks {
            anyhow::ensure!(
                track.channel < 16 && channels.insert(track.channel),
                "tracks need unique MIDI channels"
            );
            anyhow::ensure!(
                (track.instrument == Instrument::Drums) == (track.channel == 9),
                "channel 10 is reserved for drums"
            );
            anyhow::ensure!(
                track.gain.is_finite()
                    && (0.0..=1.0).contains(&track.gain)
                    && track.pan.is_finite()
                    && (-1.0..=1.0).contains(&track.pan),
                "invalid mixer values"
            );
            anyhow::ensure!(
                track.notes.len() <= 20000
                    && track.notes.iter().all(|n| n.pitch <= 127
                        && n.velocity > 0
                        && n.velocity <= 127
                        && n.start.is_finite()
                        && n.end.is_finite()
                        && n.start >= 0.0
                        && n.end > n.start
                        && n.end <= self.duration_beats + 0.001),
                "invalid score note"
            );
        }
        Ok(())
    }
    pub fn to_mid_bytes(&self) -> anyhow::Result<Vec<u8>> {
        use midly::{
            Format, Header, MetaMessage as M, MidiMessage as MM, Smf, Timing, TrackEvent,
            TrackEventKind as K,
        };
        self.validate()?;
        let mut conductor = vec![
            TrackEvent {
                delta: 0.into(),
                kind: K::Meta(M::Tempo((60_000_000 / self.tempo_bpm).into())),
            },
            TrackEvent {
                delta: 0.into(),
                kind: K::Meta(M::TimeSignature(
                    self.meter[0],
                    self.meter[1].ilog2() as u8,
                    if self.meter[1] == 8 { 36 } else { 24 },
                    8,
                )),
            },
        ];
        let mut previous = 0;
        for section in &self.sections {
            let tick = (section.start * 480.0).round() as u32;
            conductor.push(TrackEvent {
                delta: tick.saturating_sub(previous).into(),
                kind: K::Meta(M::Marker(section.name.as_bytes())),
            });
            previous = tick;
        }
        let end_tick = (self.duration_beats * 480.0).round() as u32;
        conductor.push(TrackEvent {
            delta: end_tick.saturating_sub(previous).into(),
            kind: K::Meta(M::EndOfTrack),
        });
        let mut tracks = vec![conductor];
        for t in &self.tracks {
            let channel = t.channel.into();
            let mut track = vec![
                TrackEvent {
                    delta: 0.into(),
                    kind: K::Meta(M::TrackName(t.name.as_bytes())),
                },
                TrackEvent {
                    delta: 0.into(),
                    kind: K::Midi {
                        channel,
                        message: MM::ProgramChange {
                            program: t.instrument.program().into(),
                        },
                    },
                },
                TrackEvent {
                    delta: 0.into(),
                    kind: K::Midi {
                        channel,
                        message: MM::Controller {
                            controller: 7.into(),
                            value: ((t.gain * 127.0) as u8).into(),
                        },
                    },
                },
                TrackEvent {
                    delta: 0.into(),
                    kind: K::Midi {
                        channel,
                        message: MM::Controller {
                            controller: 10.into(),
                            value: (((t.pan + 1.0) * 63.5) as u8).into(),
                        },
                    },
                },
            ];
            let mut events = Vec::new();
            for n in &t.notes {
                events.push(((n.start * 480.0).round() as u32, true, n));
                events.push(((n.end * 480.0).round() as u32, false, n));
            }
            // Release a repeated pitch before retriggering it at the same tick.
            events.sort_by_key(|(tick, on, n)| (*tick, *on, n.pitch));
            let mut last = 0;
            for (tick, on, n) in events {
                let message = if on {
                    MM::NoteOn {
                        key: n.pitch.into(),
                        vel: n.velocity.into(),
                    }
                } else {
                    MM::NoteOff {
                        key: n.pitch.into(),
                        vel: 0.into(),
                    }
                };
                track.push(TrackEvent {
                    delta: (tick - last).into(),
                    kind: K::Midi { channel, message },
                });
                last = tick;
            }
            track.push(TrackEvent {
                delta: end_tick.saturating_sub(last).into(),
                kind: K::Meta(M::EndOfTrack),
            });
            tracks.push(track);
        }
        let smf = Smf {
            header: Header {
                format: Format::Parallel,
                timing: Timing::Metrical(480.into()),
            },
            tracks,
        };
        let mut bytes = Vec::new();
        smf.write(&mut bytes).map_err(|e| anyhow::anyhow!(e))?;
        Ok(bytes)
    }
}
