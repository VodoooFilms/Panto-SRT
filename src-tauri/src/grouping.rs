//! Subtitle grouping engine: word-level timestamps -> clean subtitle clips.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Word {
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub start: f64,
    #[serde(default)]
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Clip {
    pub index: usize,
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupOptions {
    pub max_chars: Option<usize>,
    pub max_duration: Option<f64>,
    pub min_gap: Option<f64>,
    pub min_sentence_break: Option<usize>,
}

const SENTENCE_END: &[char] = &['.', '!', '?', '…', ';'];
const PUNCT_CHARS: &[char] = &[
    '.', ',', ';', ':', '!', '?', '…', '\u{00a0}', '"', '\'', '-', '–',
];

pub fn sanitize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_punctuation_token(trimmed: &str) -> bool {
    trimmed.chars().count() <= 3
        && !trimmed.is_empty()
        && trimmed.chars().all(|c| PUNCT_CHARS.contains(&c))
}

/// Normalize raw words: drop empty tokens, attach standalone punctuation to
/// the previous word, and repair degenerate timestamps.
pub fn normalize_words(input: &[Word]) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    for item in input {
        let trimmed = item.text.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }
        if is_punctuation_token(&trimmed) {
            if let Some(prev) = words.last_mut() {
                prev.text.push_str(&trimmed);
            }
            continue;
        }
        if !item.start.is_finite() || !item.end.is_finite() {
            continue;
        }
        let (mut start, mut end) = (item.start, item.end);
        if end < start {
            std::mem::swap(&mut start, &mut end);
        }
        if start < 0.0 {
            end -= start;
            start = 0.0;
        }
        words.push(Word {
            text: sanitize(&trimmed),
            start,
            end,
        });
    }

    for i in 0..words.len() {
        if words[i].end <= words[i].start {
            let prev_end = if i > 0 { Some(words[i - 1].end) } else { None };
            let next_start = words.get(i + 1).map(|w| w.start);
            let start = match prev_end {
                Some(pe) => pe.max(words[i].start),
                None => words[i].start.max(0.0),
            };
            let mut end = match next_start {
                Some(ns) => ns.min(start + 1.0),
                None => start + (words[i].text.chars().count() as f64 / 12.0).max(0.25),
            };
            if end <= start {
                end = start + 0.25;
            }
            words[i].start = start;
            words[i].end = end;
        }
    }
    words
}

pub fn build_clips(input: &[Word], options: &GroupOptions) -> Vec<Clip> {
    let max_chars = options.max_chars.unwrap_or(42);
    let max_duration = options.max_duration.unwrap_or(3.5);
    let min_gap = options.min_gap.unwrap_or(0.2);
    let min_sentence_break = options.min_sentence_break.unwrap_or(12);

    let words = normalize_words(input);
    let mut clips: Vec<Clip> = Vec::new();
    let mut bucket: Vec<Word> = Vec::new();

    fn flush(bucket: &mut Vec<Word>, clips: &mut Vec<Clip>) {
        if bucket.is_empty() {
            return;
        }
        let start = bucket.first().unwrap().start;
        let end = bucket.last().unwrap().end;
        let joined = bucket
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        // Remove the space before closing punctuation.
        let mut text = String::with_capacity(joined.len());
        for ch in joined.chars() {
            if matches!(ch, ',' | '.' | ';' | ':' | '!' | '?' | '…')
                && text.ends_with(' ')
            {
                text.pop();
            }
            text.push(ch);
        }
        let text = text.trim().to_string();
        if !text.is_empty() {
            clips.push(Clip {
                index: 0,
                start,
                end,
                text,
            });
        }
        bucket.clear();
    }

    for word in words {
        if bucket.is_empty() {
            bucket.push(word);
            continue;
        }
        let previous = bucket.last().unwrap().clone();
        let gap = word.start - previous.end;
        let clip_start = bucket[0].start;
        let clip_text = bucket
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let candidate_len = clip_text.chars().count() + 1 + word.text.chars().count();

        let would_overflow = candidate_len > max_chars;
        let would_overrun = word.end - clip_start > max_duration;
        let mature = word.end - clip_start >= 0.5;
        let has_clear_pause = gap >= min_gap && mature;
        let sentence_ended = sanitize(&previous.text)
            .chars()
            .last()
            .map(|c| SENTENCE_END.contains(&c))
            .unwrap_or(false);
        let mature_sentence = sentence_ended && clip_text.chars().count() >= min_sentence_break;

        if would_overflow || would_overrun || has_clear_pause || mature_sentence {
            flush(&mut bucket, &mut clips);
        }
        bucket.push(word);
    }
    flush(&mut bucket, &mut clips);

    for (i, clip) in clips.iter_mut().enumerate() {
        clip.index = i + 1;
    }
    clips
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(text: &str, start: f64, end: f64) -> Word {
        Word {
            text: text.into(),
            start,
            end,
        }
    }
    fn opts() -> GroupOptions {
        GroupOptions {
            max_chars: Some(42),
            max_duration: Some(3.5),
            min_gap: Some(0.2),
            min_sentence_break: None,
        }
    }

    #[test]
    fn punctuation_attaches_to_previous() {
        let words = normalize_words(&[w("Hola", 0.0, 0.4), w(",", 0.4, 0.45), w("mundo", 0.45, 1.0)]);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hola,");
    }

    #[test]
    fn respects_max_chars() {
        let mut words = Vec::new();
        let mut t = 0.0;
        for word in "una frase larga que deberia dividirse en varios clips".split(' ') {
            words.push(w(word, t, t + 0.4));
            t += 0.4;
        }
        let mut o = opts();
        o.max_chars = Some(18);
        o.min_gap = Some(0.0);
        let clips = build_clips(&words, &o);
        assert!(clips.len() > 1);
        for clip in &clips {
            assert!(clip.text.chars().count() <= 19, "clip over limit: {}", clip.text);
        }
    }

    #[test]
    fn respects_max_duration() {
        let words: Vec<Word> = ["uno", "dos", "tres", "cuatro", "cinco", "seis", "siete"]
            .iter()
            .enumerate()
            .map(|(i, s)| w(s, i as f64 * 0.3, i as f64 * 0.3 + 0.3))
            .collect();
        let mut o = opts();
        o.max_chars = Some(200);
        o.min_gap = Some(0.0);
        o.max_duration = Some(0.9);
        let clips = build_clips(&words, &o);
        assert!(clips.len() >= 3);
        for clip in &clips {
            assert!(clip.end - clip.start <= 0.9 + 1e-9);
        }
    }

    #[test]
    fn splits_on_clear_pause() {
        let clips = build_clips(
            &[w("Hola", 0.0, 0.5), w("señores", 0.5, 1.0), w("gracias", 1.8, 2.3)],
            &GroupOptions { max_chars: Some(200), max_duration: Some(20.0), min_gap: Some(0.2), min_sentence_break: None },
        );
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].text, "Hola señores");
        assert_eq!(clips[1].text, "gracias");
    }

    #[test]
    fn prefers_sentence_boundary() {
        let clips = build_clips(
            &[w("Este", 0.0, 0.3), w("es", 0.3, 0.6), w("un", 0.6, 0.9), w("titular.", 0.9, 1.2), w("Y", 1.2, 1.5), w("esto", 1.5, 1.8)],
            &GroupOptions { max_chars: Some(200), max_duration: Some(20.0), min_gap: Some(1.0), min_sentence_break: None },
        );
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].text, "Este es un titular.");
        assert_eq!(clips[1].text, "Y esto");
    }

    #[test]
    fn indexed_and_trimmed() {
        let clips = build_clips(&[w("  hola  ", 0.0, 0.4), w("mundo", 0.4, 0.9)], &opts());
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].text, "hola mundo");
        assert_eq!(clips[0].index, 1);
        assert_eq!(clips[0].start, 0.0);
        assert_eq!(clips[0].end, 0.9);
    }

    #[test]
    fn never_drops_content_across_silences() {
        let clips = build_clips(
            &[w("a", 0.0, 0.1), w("b", 0.1, 0.19), w("c", 3.0, 3.4)],
            &GroupOptions { max_chars: Some(200), max_duration: Some(20.0), min_gap: Some(0.2), min_sentence_break: None },
        );
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].text, "a b");
        assert_eq!(clips[1].text, "c");
    }
}
