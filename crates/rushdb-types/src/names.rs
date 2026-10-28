//! Names of labels and properties compared the way people misspell them:
//! tokens split on case and separators with plural endings dropped, then a
//! spelling score. Shared by the catalog's name suggestions and SQL's
//! "did you mean" errors.

use std::collections::BTreeSet;

/// Lowercase name tokens: camelCase and every non-alphanumeric character
/// split words, and a plural ending is dropped (`ies` -> `y`; `es` after
/// o, s, x, z, ch, sh; else a final `s`), as the Core platform's
/// relationship analysis does (it drops `ies`/`s` only).
#[must_use]
pub fn name_tokens(name: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in name.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        if c.is_uppercase() && previous_lower && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        previous_lower = c.is_lowercase() || c.is_ascii_digit();
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
        .into_iter()
        .map(|word| {
            if let Some(stem) = word.strip_suffix("ies") {
                format!("{stem}y")
            } else if ["oes", "ses", "xes", "zes", "ches", "shes"]
                .iter()
                .any(|ending| word.len() > ending.len() + 1 && word.ends_with(ending))
            {
                word[..word.len() - 2].to_string()
            } else if word.len() > 1 && word.ends_with('s') {
                word[..word.len() - 1].to_string()
            } else {
                word
            }
        })
        .collect()
}

/// Edit distance between two strings, in characters.
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (diagonal + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(above + 1);
            diagonal = above;
        }
    }
    row[b.len()]
}

/// Spelling similarity of two names from 0 to 1: 1 when their tokens are
/// equal (case, separators and plural endings ignored), else the larger of
/// the edit-distance ratio and the token overlap of the joined forms.
#[must_use]
pub fn name_similarity(wanted: &str, candidate: &str) -> f64 {
    let wanted_tokens = name_tokens(wanted);
    let candidate_tokens = name_tokens(candidate);
    let wanted_joined = wanted_tokens.concat();
    let candidate_joined = candidate_tokens.concat();
    if wanted_joined.is_empty() || candidate_joined.is_empty() {
        return 0.0;
    }
    if wanted_joined == candidate_joined {
        return 1.0;
    }
    let longest = wanted_joined
        .chars()
        .count()
        .max(candidate_joined.chars().count());
    #[expect(
        clippy::cast_precision_loss,
        reason = "name lengths are small; the ratio is a score"
    )]
    let edit = 1.0 - levenshtein(&wanted_joined, &candidate_joined) as f64 / longest as f64;
    let wanted_set: BTreeSet<&String> = wanted_tokens.iter().collect();
    let candidate_set: BTreeSet<&String> = candidate_tokens.iter().collect();
    #[expect(
        clippy::cast_precision_loss,
        reason = "token counts are small; the ratio is a score"
    )]
    let overlap = wanted_set.intersection(&candidate_set).count() as f64
        / wanted_set.union(&candidate_set).count() as f64;
    edit.max(overlap)
}

/// The best-scoring candidates for `wanted` (score at least 0.5), best first.
#[must_use]
pub fn closest<'a>(
    wanted: &str,
    candidates: impl IntoIterator<Item = &'a str>,
    limit: usize,
) -> Vec<&'a str> {
    let mut scored: Vec<(f64, &str)> = candidates
        .into_iter()
        .map(|candidate| (name_similarity(wanted, candidate), candidate))
        .filter(|(score, _)| *score >= 0.5)
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    scored.dedup_by(|a, b| a.1 == b.1);
    scored
        .into_iter()
        .take(limit)
        .map(|(_, name)| name)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_split_case_and_separators_and_drop_plurals() {
        assert_eq!(
            name_tokens("commander_character_ids"),
            ["commander", "character", "id"]
        );
        assert_eq!(name_tokens("parentId"), ["parent", "id"]);
        assert_eq!(name_tokens("Categories"), ["category"]);
        assert_eq!(name_tokens("superheroes"), ["superhero"]);
        assert_eq!(name_tokens("Examination Date"), ["examination", "date"]);
    }

    #[test]
    fn names_score_by_spelling() {
        assert!((name_similarity("patients", "Patient") - 1.0).abs() < f64::EPSILON);
        assert!((name_similarity("superheroes", "superhero") - 1.0).abs() < f64::EPSILON);
        assert!(name_similarity("eye_color_id", "eye_colour_id") > 0.8);
        assert!(name_similarity("heroes", "publisher") < 0.5);
        assert_eq!(
            closest("patients", ["Patient", "Laboratory"], 3),
            ["Patient"]
        );
    }
}
