//! 临时池候选抽取规则。
//!
//! 当前实现非常保守：只把“整段标准化文本”当候选，不切内部子串，
//! 这是为了避免误把一长段文本里的局部短语当成高频模板。

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
