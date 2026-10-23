use serde::{Deserialize, Serialize};

use crate::TextLine;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretKind {
    Email,
    Phone,
    Address,
    Card,
    Iban,
    Key,
}

impl SecretKind {
    pub fn serious(self) -> bool {
        matches!(self, Self::Card | Self::Key)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub id: String,
    pub kind: SecretKind,
    pub text: String,
    pub preview: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

pub fn scan(lines: &[TextLine], dismissed: &[String]) -> Vec<Finding> {
    let mut found = Vec::new();
    for line in lines {
        for hit in hits(&line.text) {
            let id = format!("{:?}:{}:{:.0}:{:.0}", hit.kind, hit.text, line.x, line.y);
            if dismissed.iter().any(|item| item == &id) {
                continue;
            }
            found.push(Finding {
                id,
                kind: hit.kind,
                preview: preview(&hit.text),
                text: hit.text,
                x: line.x,
                y: line.y,
                w: line.w,
                h: line.h,
            });
        }
    }
    found
}

struct Hit {
    kind: SecretKind,
    text: String,
}

fn hits(text: &str) -> Vec<Hit> {
    let mut found = Vec::new();
    for token in tokens(text) {
        if let Some(kind) = classify(&token) {
            found.push(Hit { kind, text: token });
        }
    }
    found
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | ';' | '(' | ')' | '"' | '\''))
        .filter(|token| !token.is_empty())
        .map(|token| token.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '@' && ch != '+' && ch != '-' && ch != '_' && ch != '.').to_string())
        .filter(|token| !token.is_empty())
        .collect()
}

fn classify(token: &str) -> Option<SecretKind> {
    if looks_like_email(token) {
        return Some(SecretKind::Email);
    }
    if looks_like_phone(token) {
        return Some(SecretKind::Phone);
    }
    if looks_like_iban(token) {
        return Some(SecretKind::Iban);
    }
    if looks_like_card(token) {
        return Some(SecretKind::Card);
    }
    if looks_like_key(token) {
        return Some(SecretKind::Key);
    }
    if looks_like_address(token) {
        return Some(SecretKind::Address);
    }
    None
}

fn looks_like_email(token: &str) -> bool {
    let mut parts = token.split('@');
    let local = parts.next().unwrap_or("");
    let host = parts.next().unwrap_or("");
    parts.next().is_none() && local.len() >= 1 && host.contains('.') && !host.starts_with('.') && !host.ends_with('.')
}

fn looks_like_phone(token: &str) -> bool {
    let digits: String = token.chars().filter(|ch| ch.is_ascii_digit()).collect();
    (10..=15).contains(&digits.len()) && token.chars().any(|ch| matches!(ch, '+' | '-' | '(' | ')'))
}

fn looks_like_card(token: &str) -> bool {
    let digits: String = token.chars().filter(|ch| ch.is_ascii_digit()).collect();
    (13..=19).contains(&digits.len()) && luhn(&digits)
}

fn looks_like_iban(token: &str) -> bool {
    let compact: String = token.chars().filter(|ch| ch.is_ascii_alphanumeric()).collect();
    let compact = compact.to_ascii_uppercase();
    (15..=34).contains(&compact.len())
        && compact.chars().take(2).all(|ch| ch.is_ascii_uppercase())
        && compact.chars().skip(2).take(2).all(|ch| ch.is_ascii_digit())
}

fn looks_like_key(token: &str) -> bool {
    let prefixes = ["sk-", "sk_", "pk_", "rk_", "ghp_", "gho_", "github_pat_", "xox", "AKIA", "AIza", "ya29."];
    if prefixes.iter().any(|prefix| token.starts_with(prefix)) && token.len() >= 16 {
        return true;
    }
    token.len() >= 24 && token.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-') && entropy(token) >= 3.5
}

fn looks_like_address(token: &str) -> bool {
    let lower = token.to_ascii_lowercase();
    ["street", "st.", "road", "rd.", "avenue", "ave.", "drive", "lane"].iter().any(|word| lower.contains(word))
        && token.chars().any(|ch| ch.is_ascii_digit())
}

pub fn luhn(digits: &str) -> bool {
    if digits.len() < 13 || !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    let mut sum = 0;
    let mut alt = false;
    for ch in digits.chars().rev() {
        let mut n = ch.to_digit(10).unwrap_or(0);
        if alt {
            n *= 2;
            if n > 9 {
                n -= 9;
            }
        }
        sum += n;
        alt = !alt;
    }
    sum % 10 == 0
}

fn entropy(token: &str) -> f32 {
    if token.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 128];
    for ch in token.chars() {
        let code = ch as usize;
        if code < counts.len() {
            counts[code] += 1;
        }
    }
    let len = token.chars().count() as f32;
    let mut value = 0.0;
    for count in counts {
        if count == 0 {
            continue;
        }
        let p = count as f32 / len;
        value -= p * p.log2();
    }
    value
}

fn preview(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= 6 {
        return "•".repeat(chars.len());
    }
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{}{tail}", "•".repeat(4))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextLine;

    #[test]
    fn luhn_accepts_a_known_card() {
        assert!(luhn("4242424242424242"));
        assert!(!luhn("4242424242424243"));
    }

    #[test]
    fn entropy_rejects_short_words() {
        assert!(looks_like_key("sk-live-abcdefghijklmnopqrstuvwxyz"));
        assert!(!looks_like_key("password"));
    }

    #[test]
    fn dismissed_findings_stay_dismissed() {
        let lines = [TextLine {
            text: "ada@example.com".into(),
            x: 10.0,
            y: 20.0,
            w: 80.0,
            h: 12.0,
        }];
        let first = scan(&lines, &[]);
        assert_eq!(first.len(), 1);
        let again = scan(&lines, &[first[0].id.clone()]);
        assert!(again.is_empty());
    }
}
