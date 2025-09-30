// crates/text-features/src/lib.rs
use serde::{Serialize, Deserialize};
use anyhow::Result;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFeatures {
    pub n_chars: usize,
    pub n_words: usize,
    pub ttr: f32,                  // type-token ratio
    pub syllables_total: usize,
    pub syllables_per_word: f32,
    pub reading_time_minutes: f32, // words / 180
    pub punctuation_ratio: f32,    // punct chars / total chars
    pub sentiment_score: f32,      // [-1,1] heuristic
    pub char_entropy_bits: f32,    // 0..~log2|alphabet|
    pub word_entropy_bits: f32,    // normalized by log2(vocab)
}

pub fn analyze_text(s: &str) -> Result<TextFeatures> {
    let n_chars = s.chars().count();
    let words: Vec<&str> = s.split_whitespace().collect();
    let n_words = words.len();

    let mut vocab = HashSet::new();
    for w in &words {
        vocab.insert(w.to_lowercase());
    }
    let ttr = if n_words>0 { (vocab.len() as f32)/(n_words as f32) } else { 0.0 };

    // syllables (rough English heuristic)
    fn count_syllables(w: &str) -> usize {
        let w = w.to_lowercase();
        let vowels = "aeiouyăîâoe";
        let mut prev_v = false;
        let mut cnt = 0;
        for ch in w.chars() {
            let is_v = vowels.contains(ch);
            if is_v && !prev_v { cnt += 1; }
            prev_v = is_v;
        }
        cnt.max(1)
    }
    let syllables_total: usize = words.iter().map(|w| count_syllables(w)).sum();
    let syllables_per_word = if n_words>0 { syllables_total as f32 / n_words as f32 } else { 0.0 };
    let reading_time_minutes = if n_words>0 { n_words as f32 / 180.0 } else { 0.0 };

    let punct_set: &[_] = &['.', ',', ';', ':', '!', '?', '-', '(', ')', '[', ']', '{', '}', '"', '\''];
    let punct_count = s.chars().filter(|c| punct_set.contains(c)).count();
    let punctuation_ratio = if n_chars>0 { punct_count as f32 / n_chars as f32 } else { 0.0 };

    // very simple sentiment lexicon (extend as needed)
    fn sentiment_score(s: &str) -> f32 {
        const POS: &[&str] = &["good","great","hope","love","happy","bright","calm","win","nice","excellent","amazing","best"];
        const NEG: &[&str] = &["bad","sad","hate","angry","dark","fail","worst","terrible","awful","ugly","mad"];
        let mut sc = 0i32;
        for w in s.split_whitespace() {
            let w = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if POS.contains(&w.as_str()) { sc += 1; }
            if NEG.contains(&w.as_str()) { sc -= 1; }
        }
        (sc as f32).clamp(-5.0, 5.0)/5.0
    }
    let sentiment_score = sentiment_score(s);

    // char entropy
    let mut char_hist = std::collections::BTreeMap::<char, usize>::new();
    for ch in s.chars() { *char_hist.entry(ch).or_default() += 1; }
    let char_entropy_bits = if n_chars>0 {
        let n = n_chars as f64;
        let h: f64 = char_hist.values().map(|&c| {
            let p = c as f64 / n;
            -p * (p.ln()/std::f64::consts::LN_2)
        }).sum();
        h as f32
    } else { 0.0 };

    // word entropy normalized
    let mut word_hist = std::collections::BTreeMap::<String, usize>::new();
    for w in &words { *word_hist.entry(w.to_lowercase()).or_default() += 1; }
    let word_entropy_bits = if n_words>0 && !word_hist.is_empty() {
        let n = n_words as f64;
        let h: f64 = word_hist.values().map(|&c| {
            let p = c as f64 / n;
            -p * (p.ln()/std::f64::consts::LN_2)
        }).sum();
        // normalize by log2(|vocab|) -> 0..1
        let norm = (word_hist.len() as f64).log2().max(1.0);
        (h / norm) as f32
    } else { 0.0 };

    Ok(TextFeatures{
        n_chars, n_words, ttr, syllables_total, syllables_per_word,
        reading_time_minutes, punctuation_ratio, sentiment_score,
        char_entropy_bits, word_entropy_bits
    })
}
