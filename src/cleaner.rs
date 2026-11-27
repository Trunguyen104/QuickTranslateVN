const SENTENCE_ENDINGS: &[char] = &['.', '!', '?', ':', ';', '”', '"', ')', ']'];

/// Repairs broken lines and hyphenated line-wraps copied from multi-column PDFs.
pub fn clean_text(raw: &str, clean_pdf_breaks: bool) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Replace common PDF ligatures
    let normalized = trimmed
        .replace('ﬁ', "fi")
        .replace('ﬂ', "fl")
        .replace('ﬀ', "ff")
        .replace('ﬃ', "ffi")
        .replace('ﬄ', "ffl");

    if !clean_pdf_breaks || !normalized.contains('\n') {
        return normalized;
    }

    let lines: Vec<&str> = normalized.lines().collect();
    if lines.len() <= 1 {
        return normalized;
    }

    let mut out = String::with_capacity(normalized.len());
    for (i, raw_line) in lines.iter().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push_str("\n\n");
            }
            continue;
        }

        // Rejoin PDF end-of-line word hyphenation: "transla-\ntion" -> "translation"
        if line.ends_with('-') && i + 1 < lines.len() {
            let next_trim = lines[i + 1].trim();
            let first_next = next_trim.chars().next();
            let prev_alpha = line.chars().rev().nth(1).map(|c| c.is_alphabetic()).unwrap_or(false);
            if prev_alpha && first_next.map(|c| c.is_lowercase()).unwrap_or(false) {
                out.push_str(&line[..line.len() - 1]);
                continue;
            }
        }

        out.push_str(line);

        if i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            if next.is_empty() {
                continue;
            }

            let is_bullet = next.starts_with("- ")
                || next.starts_with("• ")
                || next.starts_with("* ")
                || (next.len() > 2
                    && next.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                    && (next.as_bytes()[1] == b'.' || next.as_bytes()[1] == b')'));

            let ends_sentence = line
                .chars()
                .last()
                .map(|c| SENTENCE_ENDINGS.contains(&c))
                .unwrap_or(false);
            let next_starts_upper = next.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);

            if is_bullet || (ends_sentence && next_starts_upper && line.chars().count() < 45) {
                out.push('\n');
            } else {
                out.push(' ');
            }
        }
    }

    collapse_spaces(out.trim())
}

fn collapse_spaces(s: &str) -> String {
    let mut res = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                res.push(' ');
                prev_space = true;
            }
        } else {
            prev_space = false;
            res.push(ch);
        }
    }
    res
}

/// Detects characteristic Vietnamese diacritics so we can auto-flip VI <-> EN.
pub fn looks_like_vietnamese(text: &str) -> bool {
    const VN_CHARS: &str = "ăâđêôơưĂÂĐÊÔƠƯáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ";
    let mut count = 0usize;
    let total_chars = text.chars().count();
    for c in text.chars() {
        if VN_CHARS.contains(c) {
            count += 1;
            if count >= 2 || (total_chars <= 12 && count >= 1) {
                return true;
            }
        }
    }
    false
}
