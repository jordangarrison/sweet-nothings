//! Conservative preferred-word correction for raw transcription output.

use rphonetic::DoubleMetaphone;
use unicode_segmentation::UnicodeSegmentation;

const MAX_CANDIDATE_WORDS: usize = 3;
const LEVENSHTEIN_THRESHOLD: f64 = 0.80;
const PHONETIC_SPELLING_THRESHOLD: f64 = 0.60;
const MIN_FUZZY_CHARACTERS: usize = 4;
const SCORE_EPSILON: f64 = f64::EPSILON;

#[derive(Debug)]
struct WordSpan {
    start: usize,
    end: usize,
}

#[derive(Debug)]
struct Preference {
    normalized: String,
    punctuation: Vec<(usize, char)>,
    trailing_punctuation: String,
}

#[derive(Debug)]
struct Correction {
    start: usize,
    end: usize,
    preference_index: usize,
    score: f64,
    word_count: usize,
}

/// Correct preferred words in one pass over raw transcription text.
///
/// Candidates contain one to three adjacent Unicode words. Punctuation ends a
/// multi-word candidate. Only the uniquely best preference for each candidate
/// is considered, then overlapping candidates are resolved by score.
pub fn correct(text: &str, preferred_words: &[String]) -> String {
    if text.is_empty() || preferred_words.is_empty() {
        return text.to_string();
    }

    let words = word_spans(text);
    let preferences: Vec<Preference> = preferred_words
        .iter()
        .map(|preference| Preference {
            normalized: normalize(preference),
            punctuation: punctuation_structure(preference),
            trailing_punctuation: trailing_punctuation(preference).to_string(),
        })
        .collect();
    let mut corrections = Vec::new();

    for start_index in 0..words.len() {
        for word_count in 1..=MAX_CANDIDATE_WORDS {
            let end_index = start_index + word_count - 1;
            if end_index >= words.len() {
                break;
            }
            if end_index > start_index
                && !text[words[end_index - 1].end..words[end_index].start]
                    .chars()
                    .all(char::is_whitespace)
            {
                break;
            }

            let start = words[start_index].start;
            let end = words[end_index].end;
            if let Some((preference_index, score, end)) =
                unique_best_match(text, start, end, &preferences)
            {
                corrections.push(Correction {
                    start,
                    end,
                    preference_index,
                    score,
                    word_count,
                });
            }
        }
    }

    corrections.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| right.word_count.cmp(&left.word_count))
            .then_with(|| (right.end - right.start).cmp(&(left.end - left.start)))
            .then_with(|| left.start.cmp(&right.start))
    });

    let mut selected = Vec::new();
    for correction in corrections {
        if selected.iter().all(|existing: &Correction| {
            correction.end <= existing.start || correction.start >= existing.end
        }) {
            selected.push(correction);
        }
    }
    selected.sort_by_key(|correction| correction.start);

    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    for correction in selected {
        output.push_str(&text[cursor..correction.start]);
        output.push_str(&preferred_words[correction.preference_index]);
        cursor = correction.end;
    }
    output.push_str(&text[cursor..]);
    output
}

fn word_spans(text: &str) -> Vec<WordSpan> {
    text.unicode_word_indices()
        .filter_map(|(start, word)| {
            let word = strip_possessive(word);
            if word.chars().any(char::is_alphanumeric) {
                Some(WordSpan {
                    start,
                    end: start + word.len(),
                })
            } else {
                None
            }
        })
        .collect()
}

fn strip_possessive(word: &str) -> &str {
    ["'s", "'S", "’s", "’S"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix))
        .unwrap_or(word)
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|character| character.is_alphanumeric())
        .collect()
}

fn punctuation_structure(value: &str) -> Vec<(usize, char)> {
    let mut alphanumeric_count = 0;
    let mut punctuation = Vec::new();

    for character in value.chars() {
        if character.is_alphanumeric() {
            alphanumeric_count += 1;
        } else if !character.is_whitespace() {
            punctuation.push((alphanumeric_count, character));
        }
    }

    punctuation
}

fn trailing_punctuation(value: &str) -> &str {
    value
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_alphanumeric())
        .map(|(index, character)| &value[index + character.len_utf8()..])
        .unwrap_or(value)
}

fn unique_best_match(
    text: &str,
    start: usize,
    end: usize,
    preferences: &[Preference],
) -> Option<(usize, f64, usize)> {
    let mut best: Option<(usize, f64, usize)> = None;
    let mut tied = false;

    for (index, preference) in preferences.iter().enumerate() {
        let candidate_end = if text[start..end].ends_with(&preference.trailing_punctuation) {
            end
        } else {
            let trailing_text = text.get(end..)?;
            let trailing_length = preference.trailing_punctuation.len();
            if !trailing_text.starts_with(&preference.trailing_punctuation) {
                continue;
            }
            end + trailing_length
        };
        let candidate = &text[start..candidate_end];
        if punctuation_structure(candidate) != preference.punctuation {
            continue;
        }

        let normalized_candidate = normalize(candidate);
        let Some(score) = match_score(&normalized_candidate, &preference.normalized) else {
            continue;
        };

        match best {
            None => {
                best = Some((index, score, candidate_end));
                tied = false;
            }
            Some((_, best_score, _)) if score > best_score + SCORE_EPSILON => {
                best = Some((index, score, candidate_end));
                tied = false;
            }
            Some((_, best_score, _)) if (score - best_score).abs() <= SCORE_EPSILON => {
                tied = true;
            }
            Some(_) => {}
        }
    }

    if tied {
        None
    } else {
        best
    }
}

fn match_score(candidate: &str, preference: &str) -> Option<f64> {
    if candidate.is_empty() || preference.is_empty() {
        return None;
    }
    if candidate == preference {
        return Some(1.0);
    }
    if candidate.chars().count() < MIN_FUZZY_CHARACTERS
        || preference.chars().count() < MIN_FUZZY_CHARACTERS
    {
        return None;
    }

    let similarity = strsim::normalized_levenshtein(candidate, preference);
    if similarity >= LEVENSHTEIN_THRESHOLD
        || (similarity >= PHONETIC_SPELLING_THRESHOLD
            && double_metaphone_keys_match(candidate, preference))
    {
        Some(similarity)
    } else {
        None
    }
}

fn double_metaphone_keys_match(left: &str, right: &str) -> bool {
    if !left
        .chars()
        .all(|character| character.is_ascii_alphabetic())
        || !right
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return false;
    }

    let encoder = DoubleMetaphone::default();
    let left = encoder.double_metaphone(left);
    let right = encoder.double_metaphone(right);
    let left_keys = [left.primary(), left.alternate()];
    let right_keys = [right.primary(), right.alternate()];

    left_keys
        .iter()
        .any(|left_key| !left_key.is_empty() && right_keys.contains(left_key))
}

#[cfg(test)]
mod tests {
    use super::correct;

    fn preferences(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn corrects_common_mikayla_spellings_and_phrase() {
        let words = preferences(&["Mikayla"]);

        assert_eq!(correct("Michaela", &words), "Mikayla");
        assert_eq!(correct("Makayla", &words), "Mikayla");
        assert_eq!(correct("my Kayla", &words), "Mikayla");
    }

    #[test]
    fn preserves_exact_configured_casing() {
        let words = preferences(&["Mikayla"]);

        assert_eq!(
            correct("mikayla met MIKAYLA", &words),
            "Mikayla met Mikayla"
        );
        assert_eq!(correct("Mikayla", &words), "Mikayla");
    }

    #[test]
    fn preserves_punctuation_whitespace_and_possessives() {
        let words = preferences(&["Mikayla"]);

        assert_eq!(
            correct("Hi,  Michaela's here.", &words),
            "Hi,  Mikayla's here."
        );
        assert_eq!(correct("my Kayla’s bag", &words), "Mikayla’s bag");
    }

    #[test]
    fn preserves_surrounding_sentence_punctuation() {
        let words = preferences(&["Mikayla"]);

        assert_eq!(correct("(Michaela),", &words), "(Mikayla),");
        assert_eq!(correct("\"Michaela!\"", &words), "\"Mikayla!\"");
    }

    #[test]
    fn punctuation_structure_must_match() {
        assert_eq!(correct("foo.bar", &preferences(&["Fubar"])), "foo.bar");
        assert_eq!(correct("32.3", &preferences(&["323"])), "32.3");
        assert_eq!(correct("foobar", &preferences(&["Foo.Bar"])), "foobar");
    }

    #[test]
    fn punctuation_bearing_preferences_replace_the_full_token() {
        let words = preferences(&["C++"]);

        assert_eq!(correct("c++", &words), "C++");
        assert_eq!(correct("Use c++.", &words), "Use C++.");
        assert_eq!(correct("c++'s types", &words), "C++'s types");
    }

    #[test]
    fn supports_unicode_exact_matches() {
        let words = preferences(&["Élodie", "東京"]);

        assert_eq!(
            correct("élodie visited 東京", &words),
            "Élodie visited 東京"
        );
    }

    #[test]
    fn leaves_unrelated_words_unchanged() {
        let words = preferences(&["Isla", "Shinobi"]);

        assert_eq!(correct("The island", &words), "The island");
        assert_eq!(correct("Son of a sailor", &words), "Son of a sailor");
    }

    #[test]
    fn leaves_equal_preference_scores_ambiguous() {
        let words = preferences(&["Mikayla", "Makayla"]);

        assert_eq!(correct("Mekayla", &words), "Mekayla");
    }

    #[test]
    fn short_preferences_only_receive_exact_correction() {
        let words = preferences(&["ID"]);

        assert_eq!(correct("id", &words), "ID");
        assert_eq!(correct("od", &words), "od");
    }

    #[test]
    fn does_not_cross_punctuation() {
        let words = preferences(&["Mikayla"]);

        assert_eq!(correct("my, Kayla", &words), "my, Kayla");
        assert_eq!(correct("my\nKayla", &words), "Mikayla");
    }

    #[test]
    fn considers_at_most_three_words() {
        let three_words = preferences(&["112233"]);
        let four_words = preferences(&["11223344"]);

        assert_eq!(correct("11 22 33", &three_words), "112233");
        assert_eq!(correct("11 22 33 44", &three_words), "112233 44");
        assert_eq!(correct("11 22 33 44", &four_words), "11 22 33 44");
    }

    #[test]
    fn stronger_raw_match_wins_over_overlapping_phrase() {
        let words = preferences(&["Mikayla"]);

        assert_eq!(correct("my Mikayla", &words), "my Mikayla");
    }

    #[test]
    fn exact_phrase_wins_over_overlapping_exact_word() {
        let words = preferences(&["MARY JANE", "Jane"]);

        assert_eq!(correct("mary jane", &words), "MARY JANE");
    }

    #[test]
    fn corrections_do_not_trigger_another_correction_pass() {
        let words = preferences(&["Mikayla", "Widget", "MikaylaWidget"]);

        assert_eq!(correct("Michaela Widgek", &words), "Mikayla Widget");
    }

    #[test]
    fn empty_preferences_leave_text_untouched() {
        assert_eq!(correct("Michaela", &[]), "Michaela");
    }
}
