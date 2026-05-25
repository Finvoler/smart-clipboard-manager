const MIN_CHARS: usize = 2;
const MAX_CHARS: usize = 1_000;

pub fn extract_candidates(input: &str) -> Vec<String> {
    let normalized = normalize(input);
    let length = normalized.chars().count();
    if !(MIN_CHARS..=MAX_CHARS).contains(&length) {
        return Vec::new();
    }

    if normalized
        .chars()
        .all(|character| character.is_numeric() || character.is_ascii_punctuation())
    {
        return Vec::new();
    }

    vec![normalized]
}

fn normalize(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::extract_candidates;

    #[test]
    fn extracts_exact_normalized_text_candidate() {
        let candidates = extract_candidates("  alpha   reusable phrase   beta  ");
        assert_eq!(candidates, vec!["alpha reusable phrase beta"]);
    }

    #[test]
    fn does_not_extract_inner_phrase() {
        let candidates = extract_candidates("prefix reusable phrase longer than ten suffix");
        assert!(!candidates
            .iter()
            .any(|candidate| candidate == "reusable phrase longer than ten"));
    }

    #[test]
    fn ignores_tiny_text() {
        assert!(extract_candidates("x").is_empty());
    }
}
