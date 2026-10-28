//! Matching a search text against stored values: `contains`,
//! `prefix`, `exact` and `similar` (near spellings), and dates written in
//! another layout. The wire form is `rushdb_protocol::ValueSearch`; this is
//! the engine's side of it, shared by the value dictionaries, the facet
//! scan and the cross-property value search.
//!
//! A string matches as stored; a number, boolean or datetime matches as the
//! records response renders it; an array matches when any element does. A
//! text that reads as a date (year first, at least a month: `1993-11-12`,
//! `1993/11/12`, `2010/2/22`, `2013-06`) also matches values holding that
//! date in another layout (`1993-11-12T00:00:00Z`, `1993.11.12`,
//! `19931112`, `201306`), at the text's precision.

use std::borrow::Cow;
use std::collections::BTreeSet;

use serde_json::Value as Json;

/// How the search text must match a value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextMatch {
    /// The value contains the text.
    #[default]
    Contains,
    /// The value starts with the text.
    Prefix,
    /// The value equals the text.
    Exact,
    /// The value is spelled like the text (see [`similarity`]).
    Similar,
}

/// A search text prepared for matching many values.
#[derive(Debug, Clone)]
pub struct ValueMatcher {
    text: String,
    needle: String,
    matching: TextMatch,
    case_sensitive: bool,
    min_similarity: f64,
    date: Option<DateText>,
}

impl ValueMatcher {
    /// The least similarity a `similar` match needs when none is given.
    pub const DEFAULT_MIN_SIMILARITY: f64 = 0.6;

    /// Prepares `text` for matching (case-insensitive unless
    /// `case_sensitive`); `min_similarity` applies to `similar` only.
    #[must_use]
    pub fn new(
        text: &str,
        matching: TextMatch,
        case_sensitive: bool,
        min_similarity: Option<f64>,
    ) -> Self {
        Self {
            text: text.to_string(),
            needle: if case_sensitive {
                text.to_string()
            } else {
                text.to_lowercase()
            },
            matching,
            case_sensitive,
            min_similarity: min_similarity.unwrap_or(Self::DEFAULT_MIN_SIMILARITY),
            date: parse_date_text(text, false),
        }
    }

    /// How the text must match.
    #[must_use]
    pub fn matching(&self) -> TextMatch {
        self.matching
    }

    /// Whether `value` matches.
    #[must_use]
    pub fn matches(&self, value: &Json) -> bool {
        let text = match value {
            Json::String(text) => Cow::Borrowed(text.as_str()),
            Json::Number(number) => Cow::Owned(number.to_string()),
            Json::Bool(flag) => Cow::Owned(flag.to_string()),
            Json::Array(items) => return items.iter().any(|item| self.matches(item)),
            Json::Null | Json::Object(_) => return false,
        };
        let text = if self.case_sensitive {
            text
        } else {
            Cow::Owned(text.to_lowercase())
        };
        let textual = match self.matching {
            TextMatch::Contains => text.contains(self.needle.as_str()),
            TextMatch::Prefix => text.starts_with(self.needle.as_str()),
            TextMatch::Exact => *text == *self.needle,
            TextMatch::Similar => similarity(&self.text, &text) >= self.min_similarity,
        };
        textual
            || self.date.is_some_and(|wanted| {
                parse_date_text(&text, true)
                    .is_some_and(|held| wanted.matches(held, self.matching == TextMatch::Exact))
            })
    }

    /// How close `value` is to the text, from 0 to 1 (see [`similarity`]);
    /// arrays score their closest element.
    #[must_use]
    pub fn similarity(&self, value: &Json) -> f64 {
        match value {
            Json::String(text) => similarity(&self.text, text),
            Json::Number(number) => similarity(&self.text, &number.to_string()),
            Json::Bool(flag) => similarity(&self.text, &flag.to_string()),
            Json::Array(items) => items
                .iter()
                .map(|item| self.similarity(item))
                .fold(0.0, f64::max),
            Json::Null | Json::Object(_) => 0.0,
        }
    }
}

/// Lowercase alphanumeric words of `text`; everything else separates them.
fn similarity_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Character trigrams of each word, padded so short words still have some.
fn trigrams(words: &[String]) -> BTreeSet<[char; 3]> {
    let mut out = BTreeSet::new();
    for word in words {
        let padded: Vec<char> = std::iter::once(' ')
            .chain(std::iter::once(' '))
            .chain(word.chars())
            .chain(std::iter::once(' '))
            .collect();
        for window in padded.windows(3) {
            out.insert([window[0], window[1], window[2]]);
        }
    }
    out
}

/// How closely `stored` spells `text`, from 0 to 1. Exactly 1 when some run
/// of whole words of `stored` spells `text` once case, punctuation and
/// spacing are ignored ("Spider-Man" for "spiderman"); otherwise the share
/// of the text's character trigrams that occur in `stored` (a typo keeps
/// most of them, an unrelated word few).
#[must_use]
pub fn similarity(text: &str, stored: &str) -> f64 {
    let wanted = similarity_words(text);
    if wanted.is_empty() {
        return 0.0;
    }
    let held = similarity_words(stored);
    // Whole words only: a run of consecutive stored words that spells the
    // text once joined ("spider man" for "spiderman", "orange county" for
    // "Orange County,"), never a word fragment ("male" in "female").
    let joined = wanted.concat();
    let spelled = (0..held.len()).any(|start| {
        let mut run = String::new();
        held[start..].iter().any(|word| {
            run.push_str(word);
            run == joined
        })
    });
    if spelled {
        return 1.0;
    }
    let wanted = trigrams(&wanted);
    let held = trigrams(&held);
    #[expect(
        clippy::cast_precision_loss,
        reason = "trigram counts are small; the ratio is a score"
    )]
    let score = wanted.intersection(&held).count() as f64 / wanted.len() as f64;
    score
}

/// A calendar date read from text, to the precision written: year and month,
/// optionally the day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DateText {
    year: u32,
    month: u32,
    day: Option<u32>,
}

impl DateText {
    /// Whether `held` falls in this date (same year and month, and the same
    /// day when this one has a day). `exact` also requires equal precision.
    fn matches(self, held: Self, exact: bool) -> bool {
        self.year == held.year
            && self.month == held.month
            && match self.day {
                Some(day) => held.day == Some(day),
                None => !exact || held.day.is_none(),
            }
    }
}

/// Reads a year-first date: `Y-M[-D]` with `-`, `/` or `.` separators (month
/// and day may be unpadded), and, when `stored`, also a compact `YYYYMM[DD]`
/// and a trailing time (`T…` or ` …`, either case). Day-first and
/// month-first layouts are ambiguous and not read.
fn parse_date_text(text: &str, stored: bool) -> Option<DateText> {
    let text = text.trim();
    let date = if stored {
        text.split(['T', 't', ' ']).next().unwrap_or(text)
    } else {
        text
    };
    let valid = |year: u32, month: u32, day: Option<u32>| {
        ((1..=9999).contains(&year)
            && (1..=12).contains(&month)
            && day.is_none_or(|day| (1..=31).contains(&day)))
        .then_some(DateText { year, month, day })
    };
    let number = |part: &str, max_len: usize| {
        (!part.is_empty() && part.len() <= max_len && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
    };
    for separator in ['-', '/', '.'] {
        let parts: Vec<&str> = date.split(separator).collect();
        if parts.len() < 2 || parts.len() > 3 || parts[0].len() != 4 {
            continue;
        }
        let year = number(parts[0], 4)?;
        let month = number(parts[1], 2)?;
        let day = match parts.get(2) {
            Some(part) => Some(number(part, 2)?),
            None => None,
        };
        return valid(year, month, day);
    }
    if stored && (date.len() == 6 || date.len() == 8) && date.bytes().all(|b| b.is_ascii_digit()) {
        let year = number(&date[..4], 4)?;
        let month = number(&date[4..6], 2)?;
        let day = if date.len() == 8 {
            Some(number(&date[6..], 2)?)
        } else {
            None
        };
        return valid(year, month, day);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{parse_date_text, similarity, TextMatch, ValueMatcher};
    use serde_json::json;

    fn search(text: &str, matching: TextMatch) -> ValueMatcher {
        ValueMatcher::new(text, matching, false, None)
    }

    #[test]
    fn matches_dates_across_layouts() {
        let day = search("1993/11/12", TextMatch::Contains);
        for held in [
            "1993-11-12",
            "1993-11-12T00:00:00.000Z",
            "1993.11.12",
            "1993/11/12 08:30",
            "19931112",
        ] {
            assert!(day.matches(&json!(held)), "{held}");
        }
        assert!(day.matches(&json!(19_931_112)));
        assert!(!day.matches(&json!("1993-11-13")));
        assert!(!day.matches(&json!("1993-11")));
        // Unpadded month and day.
        assert!(search("2010/2/22", TextMatch::Contains).matches(&json!("2010-02-22")));
        // A month matches every day in it, and a compact year-month.
        let month = search("2013-06", TextMatch::Contains);
        assert!(month.matches(&json!(201_306)));
        assert!(month.matches(&json!("2013-06-30T23:00:00Z")));
        assert!(!month.matches(&json!("2013-07-01")));
        // Exact also requires the same precision.
        let exact = search("2013/06", TextMatch::Exact);
        assert!(exact.matches(&json!("2013-06")));
        assert!(!exact.matches(&json!("2013-06-01")));
    }

    #[test]
    fn reads_only_year_first_dates() {
        // Not dates: day-first, a bare year, compact text in the search,
        // an invalid month, and a word that splits on a separator.
        for text in ["12/11/1993", "1993", "19931112", "1993-13", "self-interest"] {
            assert!(parse_date_text(text, false).is_none(), "{text}");
        }
        assert!(!search("1993", TextMatch::Exact).matches(&json!("1993-01-01")));
        assert!(parse_date_text("199313", true).is_none());
        assert!(parse_date_text("2024-01-02t10:00:00z", true).is_some());
    }

    #[test]
    fn matches_text_case_insensitively_unless_asked() {
        assert!(search("ann", TextMatch::Contains).matches(&json!("Joanne")));
        assert!(search("JO", TextMatch::Prefix).matches(&json!("Joanne")));
        assert!(search("joanne", TextMatch::Exact).matches(&json!("Joanne")));
        assert!(
            !ValueMatcher::new("joanne", TextMatch::Exact, true, None).matches(&json!("Joanne"))
        );
        assert!(search("4", TextMatch::Contains).matches(&json!([1, 42])));
        assert!(!search("x", TextMatch::Contains).matches(&json!(null)));
    }

    #[test]
    fn similar_spellings_score_high_and_unrelated_words_low() {
        assert!((similarity("spiderman", "Spider-Man") - 1.0).abs() < f64::EPSILON);
        assert!(similarity("male", "female") < 1.0);
        assert!(similarity("Califronia", "California") >= 0.6);
        assert!(similarity("Texas", "California") < 0.3);
        let near = search("Orange County", TextMatch::Similar);
        assert!(near.matches(&json!("orange county,")));
        assert!(near.similarity(&json!(["Texas", "Orange County"])) > 0.99);
    }
}
