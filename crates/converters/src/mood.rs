//! Mood is a creative interpretation of a source, never a claim about a person's feelings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mood {
    Luminous,
    Tender,
    Restless,
    Wistful,
    Nocturnal,
    Serene,
}
#[derive(Clone, Debug)]
pub struct Atmosphere {
    pub valence: f32,
    pub energy: f32,
    pub complexity: f32,
    pub hue: f32,
    pub seed: u64,
    pub contour: Vec<f32>,
    pub rising: bool,
}
pub fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(1469598103934665603u64, |s, b| {
        (s ^ *b as u64).wrapping_mul(1099511628211)
    })
}
pub fn mixed(seed: u64) -> u64 {
    let mut x = seed.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
impl Atmosphere {
    pub fn mood(&self) -> Mood {
        if self.energy > 0.72 {
            if self.valence > 0.2 {
                Mood::Luminous
            } else {
                Mood::Restless
            }
        } else if self.valence > 0.33 {
            if self.energy > 0.38 {
                Mood::Luminous
            } else {
                Mood::Tender
            }
        } else if self.valence < -0.3 {
            if self.energy > 0.18 {
                Mood::Wistful
            } else {
                Mood::Nocturnal
            }
        } else {
            Mood::Serene
        }
    }
    pub fn variant(&self, variation: u32) -> Self {
        let mut value = self.clone();
        if variation > 0 {
            value.seed = mixed(self.seed ^ variation as u64);
        }
        value
    }
}
impl Mood {
    pub fn name(self) -> &'static str {
        match self {
            Self::Luminous => "Luminous",
            Self::Tender => "Tender",
            Self::Restless => "Restless",
            Self::Wistful => "Wistful",
            Self::Nocturnal => "Nocturnal",
            Self::Serene => "Serene",
        }
    }
}

pub fn text_atmosphere(text: &str) -> Atmosphere {
    let f = text_features::analyze_text(text).expect("text feature analysis is infallible");
    let mut polarity = 0.0f32;
    let mut hits = 0.0f32;
    let mut negated = 0usize;
    let mut intensity = 1.0f32;
    let positive = [
        "love",
        "hope",
        "joy",
        "happy",
        "happiness",
        "bright",
        "warm",
        "delight",
        "beautiful",
        "wonder",
        "peace",
        "calm",
        "gentle",
        "grateful",
        "alive",
        "dance",
        "laugh",
        "sunshine",
        "dream",
        "safe",
        "bucurie",
        "iubire",
        "iubesc",
        "fericit",
        "fericire",
        "speranta",
        "speranță",
        "lumina",
        "lumină",
        "liniste",
        "liniște",
    ];
    let negative = [
        "miss",
        "missing",
        "empty",
        "absence",
        "goodbye",
        "ache",
        "aching",
        "sorrow",
        "mourn",
        "gone",
        "numb",
        "loneliness",
        "heartbroken",
        "sad",
        "sadness",
        "grief",
        "loss",
        "lost",
        "lonely",
        "alone",
        "fear",
        "afraid",
        "rage",
        "angry",
        "hate",
        "hurt",
        "pain",
        "dark",
        "cold",
        "despair",
        "broken",
        "cry",
        "tears",
        "anxious",
        "trist",
        "tristete",
        "tristețe",
        "durere",
        "frica",
        "frică",
        "furie",
        "singur",
        "singuratate",
        "singurătate",
    ];
    let mut energetic = 0.0;
    for word in text.split_whitespace() {
        let word = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        if ["not", "never", "no", "nu", "fara", "fără"].contains(&word.as_str()) {
            negated = 3;
            continue;
        }
        if ["very", "so", "deeply", "extremely", "foarte", "mult"].contains(&word.as_str()) {
            intensity = 1.6;
            continue;
        }
        let sign = if positive.contains(&word.as_str()) {
            1.0
        } else if negative.contains(&word.as_str()) {
            -1.0
        } else {
            0.0
        };
        if sign != 0.0 {
            polarity += sign * intensity * if negated > 0 { -0.8 } else { 1.0 };
            hits += 1.0;
            intensity = 1.0;
        }
        if [
            "rage", "dance", "storm", "fire", "shout", "wild", "rush", "thunder", "furie",
            "furtuna", "furtună",
        ]
        .contains(&word.as_str())
        {
            energetic += 1.0;
        }
        negated = negated.saturating_sub(1);
    }
    let valence = if hits > 0.0 {
        (polarity / (hits + 0.8)).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    let energy = (0.25
        + text.matches('!').count() as f32 * 0.1
        + energetic * 0.17
        + f.punctuation_ratio * 0.5)
        .clamp(0.0, 1.0);
    let seed = hash(text.as_bytes());
    let contour = text
        .split_whitespace()
        .take(128)
        .map(|w| {
            ((hash(w.as_bytes()) % 1000) as f32 / 1000.0 + w.chars().count().min(16) as f32 / 16.0)
                * 0.5
        })
        .collect();
    Atmosphere {
        valence,
        energy,
        complexity: f.ttr,
        hue: ((seed % 50) as f32 - 25.0),
        seed,
        contour,
        rising: valence >= 0.0,
    }
}

pub fn writing(a: &Atmosphere, prose: bool, source: &str) -> (String, String) {
    let mood = a.mood();
    let choose = |values: &[&str], offset: u64| {
        values[(mixed(a.seed ^ offset) % values.len() as u64) as usize].to_string()
    };
    let (titles, openings, hearts, turns, closings): (&[&str], &[&str], &[&str], &[&str], &[&str]) =
        match mood {
            Mood::Luminous => (
                &[
                    "The world opens again",
                    "Everything still becoming",
                    "A place for the light",
                    "The courage of morning",
                    "Room for another dawn",
                    "Somewhere the world begins",
                    "What the light remembers",
                    "Everything we can still become",
                ],
                &[
                    "Something opens before it has a name.",
                    "The light arrives without asking permission.",
                    "For a moment, the world remembers how to begin.",
                ],
                &[
                    "There is warmth here that refuses to stay still.",
                    "Even the smallest distance feels like an invitation.",
                    "What was closed yesterday is learning to unfold.",
                ],
                &[
                    "I think of all the things we have not yet dared to love.",
                    "Nothing is promised, and still there is room for hope.",
                    "Perhaps joy is this: a little more room to breathe.",
                ],
                &[
                    "Let the day find us willing.",
                    "There is still so much sky left in us.",
                    "We are not finished becoming.",
                ],
            ),
            Mood::Tender => (
                &[
                    "The shape of staying",
                    "A gentler kind of brave",
                    "Held in the quiet",
                    "Where warmth remains",
                    "The quiet art of being held",
                    "Close enough to hear",
                    "A place beside the silence",
                    "What gentleness can carry",
                ],
                &[
                    "Some things ask to be held, not understood.",
                    "A small warmth gathers at the edge of silence.",
                    "The world lowers its voice, and I come closer.",
                ],
                &[
                    "Nothing here needs to prove that it belongs.",
                    "Softness survives where force would have broken.",
                    "There is a kind of courage in remaining gentle.",
                ],
                &[
                    "I think of the words we keep for someone coming home.",
                    "For once, being here is enough.",
                    "Let the unfinished things rest beside us.",
                ],
                &[
                    "Stay a little. There is no hurry.",
                    "What matters is still within reach.",
                    "We can begin again without raising our voices.",
                ],
            ),
            Mood::Restless => (
                &[
                    "Before the sky breaks",
                    "Too alive to be still",
                    "A pulse against the walls",
                    "Everything at once",
                    "The edge of becoming",
                    "A sky that will not stay still",
                    "When the walls give way",
                    "The unfinished storm",
                ],
                &[
                    "Something is moving faster than its own shadow.",
                    "The air feels charged with things not yet spoken.",
                    "There is a heartbeat here that will not stand aside.",
                ],
                &[
                    "Every edge wants to become a doorway.",
                    "The quiet cannot hold all that is gathering.",
                    "I can almost feel the distance catching fire.",
                ],
                &[
                    "Is it fear, or the first fierce moment of freedom?",
                    "What if the breaking is also a beginning?",
                    "No one has promised a safe way through.",
                ],
                &[
                    "We move anyway.",
                    "Let it change us, but let us keep our names.",
                    "Somewhere beyond the noise, a new shape is waiting.",
                ],
            ),
            Mood::Wistful => (
                &[
                    "What the distance keeps",
                    "An almost forgotten warmth",
                    "The color of missing you",
                    "After the last goodbye",
                    "Where your absence gathers",
                    "The warmth of what is gone",
                    "A little farther from home",
                    "What we could not keep",
                ],
                &[
                    "There is a space here where something used to be.",
                    "The world continues, a little farther away.",
                    "Some silences have the shape of a name.",
                ],
                &[
                    "I reach for the warmth and find its memory instead.",
                    "Even the beautiful things seem to be passing through.",
                    "What remains is not empty; it is full of what has gone.",
                ],
                &[
                    "Perhaps longing is love with nowhere familiar to stand.",
                    "We carry more than we can explain.",
                    "I do not know whether to hold on or let the distance speak.",
                ],
                &[
                    "Still, a little light refuses to leave.",
                    "What we loved has changed the way we see.",
                    "Somewhere, the tenderness remains.",
                ],
            ),
            Mood::Nocturnal => (
                &[
                    "The night holds its breath",
                    "A room inside the dark",
                    "What silence shelters",
                    "Until the world returns",
                    "A small shelter from morning",
                    "The hours without a name",
                    "Beneath the unspoken",
                    "A light kept to ourselves",
                ],
                &[
                    "The dark does not answer, but it stays.",
                    "Everything has drawn a little closer to itself.",
                    "The world is a low voice behind a closed door.",
                ],
                &[
                    "There are thoughts that only arrive when nothing asks us to speak.",
                    "Even the distance seems to have put down its weight.",
                    "Something fragile is learning how to be alone.",
                ],
                &[
                    "I cannot tell whether this is an ending or a place to rest.",
                    "Perhaps we do not need to solve the night.",
                    "For now, let the unanswered things remain unanswered.",
                ],
                &[
                    "Morning can wait a little longer.",
                    "We are still here. That is a beginning.",
                    "There is a kind of shelter in not being seen.",
                ],
            ),
            Mood::Serene => (
                &[
                    "The space between things",
                    "Where the world exhales",
                    "Nothing to outrun",
                    "A slower kind of time",
                    "The stillness we return to",
                    "A moment left unbroken",
                    "Somewhere the hurry ends",
                    "The room inside a breath",
                ],
                &[
                    "For a moment, nothing asks to be different.",
                    "The world loosens its grip, and the quiet comes through.",
                    "There is room here for a thought to finish.",
                ],
                &[
                    "What seemed so far away settles gently within reach.",
                    "Even the restless parts of us remember how to listen.",
                    "The distance is not an absence. It is somewhere to breathe.",
                ],
                &[
                    "I think of all the days we hurried past ourselves.",
                    "Perhaps enough has been here all along.",
                    "No grand answer arrives; only the relief of being present.",
                ],
                &[
                    "Let this be enough for now.",
                    "We leave the moment whole.",
                    "There is nothing here we need to outrun.",
                ],
            ),
        };
    let title = choose(titles, 1);
    let opening = choose(openings, 2);
    let heart = choose(hearts, 3);
    let turn = choose(turns, 4);
    let close = choose(closings, 5);
    let detail = if source == "image" {
        if a.hue.rem_euclid(360.0) < 70.0 {
            "A warmth like late sunlight lingers at the edges."
        } else if a.hue.rem_euclid(360.0) < 180.0 {
            "A green hush spreads as if the earth has finally stopped to listen."
        } else {
            "Cool shadows hold the last unspoken part of the day."
        }
    } else if a.energy < 0.05 {
        "Even silence leaves a shape when we stay with it."
    } else if a.rising {
        "It comes in waves, each one a little braver than the last."
    } else {
        "It draws back slowly, leaving a little more silence than before."
    };
    let movement = if source == "image" {
        if a.complexity > 0.88 {
            "The longer I stay, the more the small things begin to matter."
        } else if a.rising {
            "Somewhere at the edge, the shadows begin to loosen their hold."
        } else {
            "The brightness slips away, but its afterimage stays."
        }
    } else if a.energy > 0.72 {
        "There is too much life in this moment to keep it neatly contained."
    } else if a.complexity > 0.2 {
        "Each return is a little different, as if something is trying to find its way home."
    } else {
        "It does not need to grow louder to stay with me."
    };
    let reflection = choose(
        &[
            "I do not want to name it too quickly, in case the name makes it smaller.",
            "There are moments we understand only by the way we miss them later.",
            "What stays is not the shape of it, but the way it makes the distance feel.",
            "I let it remain unfinished; some things need that room.",
            "For a little while, the next moment can wait outside.",
            "It reaches the part of us that words usually arrive too late to find.",
        ],
        9,
    );
    let body = if prose {
        format!("{opening} {detail} {heart}\n\n{movement} {turn} {reflection} {close}")
    } else {
        format!("{opening}\n{detail}\n\n{heart}\n{turn}\n\n{movement}\n{close}")
    };
    (title.clone(), format!("{title}\n\n{body}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn polarity_negation_and_romanian() {
        assert!(text_atmosphere("love hope happy").valence > 0.3);
        assert!(text_atmosphere("not happy").valence < 0.0);
        assert!(text_atmosphere("durere trist singur").valence < -0.3);
    }
    #[test]
    fn moods_have_different_language() {
        let mut a = text_atmosphere("love hope joy");
        let bright = writing(&a, false, "image").1;
        a.valence = -0.8;
        a.energy = 0.1;
        let dark = writing(&a, false, "image").1;
        assert_ne!(bright, dark);
        for s in [bright, dark] {
            assert!(!s.contains("Hz"));
            assert!(!s.contains("pixels"));
            assert!(!s.contains("RMS"));
        }
    }
}
