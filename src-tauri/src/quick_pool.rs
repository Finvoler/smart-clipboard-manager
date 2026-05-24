use std::collections::HashSet;

const MIN_CHARS: usize = 11;
const MAX_CHARS: usize = 120;
const MAX_CANDIDATES: usize = 400;

pub fn extract_candidates(input: &str) -> Vec<String> {
  let normalized = normalize(input);
  if normalized.chars().count() < MIN_CHARS {
    return Vec::new();
  }

  let mut seen = HashSet::new();
  let mut candidates = Vec::new();

  for fragment in normalized.split(is_boundary) {
    push_candidate(fragment, &mut seen, &mut candidates);
  }

  let words: Vec<&str> = normalized.split_whitespace().collect();
  for width in 2..=10 {
    if width > words.len() {
      break;
    }
    for window in words.windows(width) {
      push_candidate(&window.join(" "), &mut seen, &mut candidates);
      if candidates.len() >= MAX_CANDIDATES {
        break;
      }
    }
  }

  let chars: Vec<char> = normalized.chars().collect();
  if words.len() <= 3 && chars.len() >= MIN_CHARS {
    for width in [12usize, 16, 24, 32] {
      if width > chars.len() {
        continue;
      }
      for start in 0..=(chars.len() - width) {
        let phrase: String = chars[start..start + width].iter().collect();
        push_candidate(&phrase, &mut seen, &mut candidates);
        if candidates.len() >= MAX_CANDIDATES {
          break;
        }
      }
    }
  }

  candidates.sort_by(|left, right| right.chars().count().cmp(&left.chars().count()).then_with(|| left.cmp(right)));
  candidates.truncate(MAX_CANDIDATES);
  candidates
}

fn normalize(input: &str) -> String {
  input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_boundary(value: char) -> bool {
  matches!(value, '\n' | '\r' | '.' | ',' | ';' | ':' | '!' | '?' | '。' | '，' | '；' | '：' | '！' | '？' | '|' | '\t')
}

fn push_candidate(value: &str, seen: &mut HashSet<String>, candidates: &mut Vec<String>) {
  if candidates.len() >= MAX_CANDIDATES {
    return;
  }

  let trimmed = value.trim().trim_matches(['-', '_', '"', '\'', '`', '“', '”', '‘', '’']);
  let length = trimmed.chars().count();
  if !(MIN_CHARS..=MAX_CHARS).contains(&length) {
    return;
  }

  if trimmed.chars().all(|character| character.is_numeric() || character.is_ascii_punctuation()) {
    return;
  }

  let candidate = trimmed.to_string();
  if seen.insert(candidate.clone()) {
    candidates.push(candidate);
  }
}

#[cfg(test)]
mod tests {
  use super::extract_candidates;

  #[test]
  fn extracts_repeated_phrase_candidate() {
    let candidates = extract_candidates("alpha reusable phrase longer than ten beta");
    assert!(candidates.iter().any(|candidate| candidate.contains("reusable phrase")));
  }

  #[test]
  fn ignores_tiny_text() {
    assert!(extract_candidates("short").is_empty());
  }
}
