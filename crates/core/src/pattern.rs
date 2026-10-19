use chrono::{DateTime, Datelike, Timelike};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct PatternContext {
    pub app: String,
    pub window: String,
    pub when: DateTime<chrono::Local>,
    pub width: u32,
    pub height: u32,
    pub counter: u32,
}

pub fn render_pattern(pattern: &str, ctx: &PatternContext) -> String {
    let mut out = String::with_capacity(pattern.len() + 16);
    let mut rest = pattern;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        if let Some(end) = rest.find('}') {
            out.push_str(&expand_token(&rest[..end], ctx));
            rest = &rest[end + 1..];
        } else {
            out.push('{');
            break;
        }
    }
    out.push_str(rest);
    sanitise_filename(&out)
}

fn expand_token(token: &str, ctx: &PatternContext) -> String {
    match token {
        "app" => ctx.app.clone(),
        "window" => ctx.window.clone(),
        "yyyy" => format!("{:04}", ctx.when.year()),
        "MM" => format!("{:02}", ctx.when.month()),
        "dd" => format!("{:02}", ctx.when.day()),
        "HH" => format!("{:02}", ctx.when.hour()),
        "mm" => format!("{:02}", ctx.when.minute()),
        "ss" => format!("{:02}", ctx.when.second()),
        "w" => ctx.width.to_string(),
        "h" => ctx.height.to_string(),
        "n" => ctx.counter.to_string(),
        _ => format!("{{{token}}}"),
    }
}

pub fn sanitise_filename(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_control() || matches!(ch, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    let trimmed = out.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() {
        "capture".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn collision_path(path: &Path, exists: impl Fn(&Path) -> bool) -> PathBuf {
    if !exists(path) {
        return path.to_path_buf();
    }
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("capture");
    let ext = path.extension().and_then(|s| s.to_str());
    let mut n = 2u32;
    loop {
        let file_name = match ext {
            Some(ext) => format!("{stem} ({n}).{ext}"),
            None => format!("{stem} ({n})"),
        };
        let candidate = parent.join(file_name);
        if !exists(&candidate) {
            return candidate;
        }
        n += 1;
        if n > 10_000 {
            return candidate;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ctx() -> PatternContext {
        PatternContext {
            app: "Safari".into(),
            window: "Inbox".into(),
            when: chrono::Local
                .with_ymd_and_hms(2026, 10, 19, 9, 41, 12)
                .single()
                .expect("time"),
            width: 1320,
            height: 800,
            counter: 3,
        }
    }

    #[test]
    fn preview_matches_saved_name() {
        let name = render_pattern("{app}-{yyyy}-{MM}-{dd} {HH}.{mm}.{ss}", &ctx());
        assert_eq!(name, "Safari-2026-10-19 09.41.12");
    }

    #[test]
    fn size_and_counter_tokens() {
        let name = render_pattern("{w}x{h}-{n}", &ctx());
        assert_eq!(name, "1320x800-3");
    }

    #[test]
    fn invalid_characters_are_replaced() {
        let mut sample = ctx();
        sample.app = "Mail/app:name".into();
        let name = render_pattern("{app}", &sample);
        assert_eq!(name, "Mail-app-name");
    }

    #[test]
    fn collisions_append_a_number() {
        let first = PathBuf::from("/tmp/Safari-2026-10-19 09.41.12.png");
        let taken = |p: &Path| {
            let s = p.to_string_lossy();
            s.ends_with("09.41.12.png") || s.ends_with("09.41.12 (2).png")
        };
        let path = collision_path(&first, taken);
        assert_eq!(
            path.file_name().unwrap().to_string_lossy(),
            "Safari-2026-10-19 09.41.12 (3).png"
        );
    }
}
