use crate::transcript::{DisplayLineStyle, DisplaySpan};

struct AnsiSpanParser {
    fallback_style: DisplayLineStyle,
    current_style: DisplayLineStyle,
    output: Vec<DisplaySpan>,
    pending_text: String,
}

impl AnsiSpanParser {
    fn new(fallback_style: DisplayLineStyle) -> Self {
        Self {
            fallback_style,
            current_style: fallback_style,
            output: Vec::new(),
            pending_text: String::new(),
        }
    }

    fn parse(&mut self, value: &str) {
        let mut characters = value.chars().peekable();
        while let Some(character) = characters.next() {
            if character != '\u{1b}' {
                self.pending_text.push(character);
                continue;
            }

            if characters.peek() != Some(&'[') {
                continue;
            }
            characters.next();
            let mut codes = String::new();
            for code_character in characters.by_ref() {
                if code_character.is_ascii_alphabetic() {
                    if code_character == 'm' {
                        self.apply_sgr_codes(&codes);
                    }
                    break;
                }
                codes.push(code_character);
            }
        }
    }

    fn finish(mut self) -> Vec<DisplaySpan> {
        self.flush_text();
        self.output
    }

    fn apply_sgr_codes(&mut self, codes: &str) {
        self.flush_text();
        let parts = codes.split(';').collect::<Vec<_>>();
        if parts.is_empty() || parts == [""] || parts.contains(&"0") {
            self.current_style = self.fallback_style;
            return;
        }

        if let Some(style) = semantic_style_from_sgr_parts(&parts) {
            self.current_style = style;
        }
    }

    fn flush_text(&mut self) {
        if self.pending_text.is_empty() {
            return;
        }

        self.output.push(DisplaySpan::new(
            std::mem::take(&mut self.pending_text),
            self.current_style,
        ));
    }
}

fn semantic_style_from_sgr_parts(parts: &[&str]) -> Option<DisplayLineStyle> {
    for window in parts.windows(5) {
        if window[0] != "38" || window[1] != "2" {
            continue;
        }

        let rgb = (
            window[2].parse::<u8>().ok()?,
            window[3].parse::<u8>().ok()?,
            window[4].parse::<u8>().ok()?,
        );
        return display_style_from_rgb(rgb);
    }

    None
}

fn display_style_from_rgb(rgb: (u8, u8, u8)) -> Option<DisplayLineStyle> {
    match rgb {
        (229, 231, 235) => Some(DisplayLineStyle::Text),
        (148, 163, 184) => Some(DisplayLineStyle::Dim),
        (217, 70, 239) => Some(DisplayLineStyle::Tool),
        (107, 114, 128) => Some(DisplayLineStyle::CommandOutput),
        (34, 197, 94) => Some(DisplayLineStyle::DiffAdded),
        (234, 179, 8) => Some(DisplayLineStyle::Warning),
        (248, 113, 113) => Some(DisplayLineStyle::Error),
        _ => None,
    }
}

/// Converts ANSI-styled shared display text into semantic display spans.
pub fn display_spans_from_ansi(value: &str, fallback_style: DisplayLineStyle) -> Vec<DisplaySpan> {
    let mut parser = AnsiSpanParser::new(fallback_style);
    parser.parse(value);
    parser.finish()
}
