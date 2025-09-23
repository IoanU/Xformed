
/// Very simple sentiment signal in [-1, 1]
pub fn sentiment_polarity(text: &str) -> f32 {
    let pos = ["sun", "warm", "love", "bright", "calm", "happy", "hope"];
    let neg = ["dark", "cold", "sad", "storm", "alone", "hate", "anger"];
    let mut s=0f32; let low = text.to_lowercase();
    for w in pos { if low.contains(w) { s += 1.0; } }
    for w in neg { if low.contains(w) { s -= 1.0; } }
    s.tanh()
}

/// Crude syllable estimate (vowel count fallback)
pub fn syllable_guess(text: &str) -> usize {
    text.split_whitespace()
        .map(|w| w.chars().filter(|c| "aeiouăîâoyuAEIOUĂÎÂOYU".contains(*c)).count().max(1))
        .sum()
}
