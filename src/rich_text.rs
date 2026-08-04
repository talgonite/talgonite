//! Rich text parsing and handling.

use i_slint_core::styled_text::{StyledText, string_to_styled_text};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichTextChunk {
    pub text: String,
    pub color_code: Option<char>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichText {
    pub chunks: Vec<RichTextChunk>,
}

/// Styling options applied when converting a [`RichText`] to rendered text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RichTextOptions {
    pub bold: bool,
    pub italic: bool,
}

impl RichText {
    /// Parse a string containing color codes like `{=c` into chunks.
    pub fn parse(input: &str) -> Self {
        let mut chunks = Vec::new();
        let parts: Vec<&str> = input.split("{=").collect();

        // The first part is before any color code
        if !parts[0].is_empty() {
            chunks.push(RichTextChunk {
                text: parts[0].to_string(),
                color_code: None,
            });
        }

        // Subsequent parts start with a color code character
        for part in &parts[1..] {
            if part.is_empty() {
                continue;
            }

            let mut chars = part.chars();
            if let Some(color_code) = chars.next() {
                let text: String = chars.collect();
                chunks.push(RichTextChunk {
                    text,
                    color_code: Some(color_code),
                });
            }
        }

        Self { chunks }
    }

    /// Convert back to a plain string by stripping all color codes.
    pub fn to_plain_string(&self) -> String {
        self.chunks.iter().map(|c| c.text.as_str()).collect()
    }

    /// Convert to an HTML string by replacing color codes with <font> tags.
    /// Text is always HTML escaped to prevent injection, and Markdown escaped
    /// to prevent unintended formatting when rendered in Slint.
    pub fn to_html_string(&self) -> String {
        self.chunks
            .iter()
            .map(|chunk| chunk.wrap_color(RichTextChunk::escape_text(&chunk.text)))
            .collect()
    }

    /// Same as [`Self::to_html_string`], with the given styling applied.
    /// Each line is escaped, coloured, and wrapped individually so bold and
    /// italic spans never cross paragraph breaks.
    pub fn to_html_string_opts(&self, options: RichTextOptions) -> String {
        // Build each line's core content (escaped + colour-wrapped), merging
        // fragments from adjacent chunks that share a line.
        let mut line_cores: Vec<String> = Vec::new();
        let mut line_index = 0usize;
        for chunk in &self.chunks {
            let mut lines = chunk.text.split('\n').peekable();
            while let Some(line) = lines.next() {
                if line_cores.len() <= line_index {
                    line_cores.push(String::new());
                }
                line_cores[line_index].push_str(&chunk.color_wrapped_line(line, options));
                if lines.peek().is_some() {
                    line_index += 1;
                }
            }
        }

        // Wrap each whole line in the requested styling, keeping leading and
        // trailing whitespace outside the markers so it can't create `****`.
        let mut lines = line_cores
            .into_iter()
            .map(|core| wrap_line_style(core, options))
            .collect::<Vec<_>>();
        let last = lines.len().saturating_sub(1);
        for (idx, line) in lines.iter_mut().enumerate() {
            if idx < last {
                line.push_str("&zwj;\n");
            }
        }
        lines.concat()
    }

    /// Same as [`Self::to_html_string`], but wraps every line in markdown
    /// bold so the whole string renders with bold styling.
    pub fn to_html_string_bold(&self) -> String {
        self.to_html_string_opts(RichTextOptions {
            bold: true,
            ..Default::default()
        })
    }

    pub fn to_slint_styled_text(&self) -> StyledText {
        let parsed = StyledText::from_markdown(&self.to_html_string());

        match parsed {
            Ok(styled) => styled,
            Err(err) => {
                let plain_string = self.to_plain_string();
                tracing::error!(
                    "Failed to parse styled text: {:?}\r\n falling back to plain string: {}",
                    err,
                    plain_string
                );
                string_to_styled_text(plain_string)
            }
        }
    }

    /// Convert to styled text with every chunk rendered bold (in addition to
    /// any colour codes).
    pub fn to_slint_styled_text_bold(&self) -> StyledText {
        self.to_slint_styled_text_opts(RichTextOptions {
            bold: true,
            ..Default::default()
        })
    }

    /// Convert to styled text with the given styling options applied on top of
    /// any colour codes.
    pub fn to_slint_styled_text_opts(&self, options: RichTextOptions) -> StyledText {
        let parsed = StyledText::from_markdown(&self.to_html_string_opts(options));

        match parsed {
            Ok(styled) => styled,
            Err(err) => {
                let plain_string = self.to_plain_string();
                tracing::error!(
                    "Failed to parse styled text: {:?}\r\n falling back to plain string: {}",
                    err,
                    plain_string
                );
                string_to_styled_text(plain_string)
            }
        }
    }
}

impl RichTextChunk {
    fn escape_text(text: &str) -> String {
        Self::escape_text_with(text, false)
    }

    fn escape_text_with(text: &str, escape_markdown_emphasis: bool) -> String {
        let mut escaped = String::with_capacity(text.len() + 20);
        for ch in text.chars() {
            match ch {
                '&' => escaped.push_str("&amp;"),
                '<' => escaped.push_str("&lt;"),
                '>' => escaped.push_str("&gt;"),
                '"' => escaped.push_str("&quot;"),
                '\'' => escaped.push_str("&#39;"),
                '\\' => escaped.push_str("\\\\"),
                '`' => escaped.push_str("\\`"),
                '~' => escaped.push_str("\\~"),
                '[' => escaped.push_str("\\["),
                ']' => escaped.push_str("\\]"),
                '#' => escaped.push_str("\\#"),
                '\r' | '\n' => escaped.push_str("&zwj;\n"), // fixes empty newlines being ignored
                '*' if escape_markdown_emphasis => escaped.push_str("\\*"),
                '_' if escape_markdown_emphasis => escaped.push_str("\\_"),
                _ => escaped.push(ch),
            }
        }
        escaped
    }

    /// Escape and colour-wrap a single line (no line breaks). Markdown
    /// emphasis markers are escaped when needed so they can't collide with
    /// the delimiters added by [`wrap_line_style`].
    fn color_wrapped_line(&self, line: &str, options: RichTextOptions) -> String {
        let escape_markdown_emphasis = options.bold || options.italic;
        let escaped = Self::escape_text_with(line, escape_markdown_emphasis);
        self.wrap_color(escaped)
    }

    fn wrap_color(&self, escaped: String) -> String {
        if let Some(code) = self.color_code {
            if let Some(color) = RichTextColor::from_char_code(code) {
                let [r, g, b, _] = color.to_color();
                return format!(
                    "<font color=\"#{:02x}{:02x}{:02x}\">{}</font>",
                    r, g, b, escaped
                );
            }
        }
        escaped
    }
}

pub enum RichTextColor {
    White,
    Red,
    Yellow,
    DarkGreen,
    LightBlue,
    DarkBlue,
    Grey0,
    Grey1,
    Grey2,
    Grey3,
    Grey4,
    Grey5,
    Grey6,
    Black,
    Pink,
    Purple,
    LimeGreen,
    Green,
    Orange,
    Brown,
    Invisible,
}

impl RichTextColor {
    pub fn from_char_code(code: char) -> Option<Self> {
        match code {
            'a' => Some(Self::White),
            'b' => Some(Self::Red),
            'c' => Some(Self::Yellow),
            'd' => Some(Self::DarkGreen),
            'e' => Some(Self::LightBlue),
            'f' => Some(Self::DarkBlue),
            'g' => Some(Self::Grey0),
            'h' => Some(Self::Grey1),
            'i' => Some(Self::Grey2),
            'j' => Some(Self::Grey3),
            'k' => Some(Self::Grey4),
            'l' => Some(Self::Grey5),
            'm' => Some(Self::Grey6),
            'n' => Some(Self::Black),
            'o' => Some(Self::Pink),
            'p' => Some(Self::Purple),
            'q' => Some(Self::LimeGreen),
            'r' => Some(Self::Green),
            's' => Some(Self::Orange),
            't' => Some(Self::Brown),
            'u' => Some(Self::White),
            'v' => Some(Self::LightBlue),
            'w' => Some(Self::Pink),
            'x' => Some(Self::Invisible),
            _ => None,
        }
    }

    pub fn to_color(&self) -> [u8; 4] {
        match self {
            Self::White => [255, 255, 255, 255],
            Self::Red => [255, 0, 16, 255],
            Self::Yellow => [255, 231, 57, 255],
            Self::DarkGreen => [0, 97, 0, 255],
            Self::LightBlue => [123, 165, 247, 255],
            Self::DarkBlue => [33, 24, 156, 255],
            Self::Grey0 => [222, 219, 222, 255],
            Self::Grey1 => [189, 186, 189, 255],
            Self::Grey2 => [148, 150, 148, 255],
            Self::Grey3 => [115, 117, 115, 255],
            Self::Grey4 => [82, 81, 82, 255],
            Self::Grey5 => [49, 47, 49, 255],
            Self::Grey6 => [9, 12, 9, 255],
            Self::Black => [0, 0, 0, 255],
            Self::Pink => [247, 88, 140, 255],
            Self::Purple => [115, 24, 115, 255],
            Self::LimeGreen => [0, 255, 0, 255],
            Self::Green => [0, 97, 0, 255],
            Self::Orange => [247, 142, 24, 255],
            Self::Brown => [99, 52, 24, 255],
            Self::Invisible => [0, 0, 0, 0],
        }
    }
}

fn wrap_line_style(core: String, options: RichTextOptions) -> String {
    let leading = core
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let trailing = core
        .chars()
        .rev()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let core_start = leading.len();
    let core_end = core.len().saturating_sub(trailing.len());
    let content = if core_end <= core_start {
        ""
    } else {
        &core[core_start..core_end]
    };
    if content.is_empty() {
        return core;
    }

    let mut prefix = String::new();
    if options.bold {
        prefix.push_str("**");
    }
    if options.italic {
        prefix.push('*');
    }
    let suffix = prefix.chars().rev().collect::<String>();
    format!("{leading}{prefix}{content}{suffix}{trailing}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rich_text() {
        let input = "Normal {=rRed {=bBlue";
        let rich = RichText::parse(input);

        assert_eq!(rich.chunks.len(), 3);
        assert_eq!(rich.chunks[0].text, "Normal ");
        assert_eq!(rich.chunks[0].color_code, None);

        assert_eq!(rich.chunks[1].text, "Red ");
        assert_eq!(rich.chunks[1].color_code, Some('r'));

        assert_eq!(rich.chunks[2].text, "Blue");
        assert_eq!(rich.chunks[2].color_code, Some('b'));

        assert_eq!(rich.to_plain_string(), "Normal Red Blue");
    }

    #[test]
    fn test_parse_empty() {
        let rich = RichText::parse("");
        assert!(rich.chunks.is_empty());
        assert_eq!(rich.to_plain_string(), "");
    }

    #[test]
    fn test_parse_only_color() {
        let rich = RichText::parse("{=r");
        assert_eq!(rich.chunks.len(), 1);
        assert_eq!(rich.chunks[0].text, "");
        assert_eq!(rich.chunks[0].color_code, Some('r'));
    }

    #[test]
    fn test_html_and_markdown_escape() {
        // * and _ are allowed for bold/italics, while ` and [ are escaped
        let rich = RichText::parse("<script>alert('1')</script> *user_input* [link] `code` {=r&");
        assert_eq!(
            rich.to_html_string(),
            "&lt;script&gt;alert(&#39;1&#39;)&lt;/script&gt; *user_input* \\[link\\] \\`code\\` <font color=\"#006100\">&amp;</font>"
        );
    }

    #[test]
    fn test_to_html_string() {
        let input = "Normal {=rRed {=bBlue";
        let rich = RichText::parse(input);
        assert_eq!(
            rich.to_html_string(),
            "Normal <font color=\"#006100\">Red </font><font color=\"#ff0010\">Blue</font>"
        );
    }

    #[test]
    fn test_to_html_string_bold() {
        let input = "Normal {=rRed";
        let rich = RichText::parse(input);
        assert_eq!(
            rich.to_html_string_bold(),
            "**Normal <font color=\"#006100\">Red</font>**"
        );
    }

    #[test]
    fn test_to_html_string_bold_multiline() {
        let input = "First {=rRed\nSecond";
        let rich = RichText::parse(input);
        assert_eq!(
            rich.to_html_string_bold(),
            "**First <font color=\"#006100\">Red</font>**&zwj;\n**<font color=\"#006100\">Second</font>**"
        );
    }

    #[test]
    fn test_bold_adjacent_color_blocks_have_no_double_asterisks() {
        let input = "{=rRed {=bBlue";
        let rich = RichText::parse(input);
        assert_eq!(
            rich.to_html_string_bold(),
            "**<font color=\"#006100\">Red </font><font color=\"#ff0010\">Blue</font>**"
        );
        let styled = rich.to_slint_styled_text_bold();
        let raw = i_slint_core::styled_text::get_raw_text(&styled);
        assert_eq!(raw, "Red Blue");
    }

    #[test]
    fn test_bold_italic_options() {
        let rich = RichText::parse("Hello {=bWorld");
        let styled = rich.to_slint_styled_text_opts(crate::rich_text::RichTextOptions {
            bold: true,
            italic: true,
        });
        let raw = i_slint_core::styled_text::get_raw_text(&styled);
        assert_eq!(raw, "Hello World");

        let html = rich.to_html_string_opts(crate::rich_text::RichTextOptions {
            bold: true,
            italic: true,
        });
        assert_eq!(html, "***Hello <font color=\"#ff0010\">World</font>***");
    }

    #[test]
    fn test_bold_escapes_markdown_emphasis() {
        // Stray `*`/`_` must render literally and never leak `****` or
        // accidentally trigger emphasis inside the bold wrapper.
        for case in ["**bold?**", "Some *italic* text", "under_score"] {
            let rich = RichText::parse(case);
            let styled = rich.to_slint_styled_text_bold();
            let raw = i_slint_core::styled_text::get_raw_text(&styled);
            assert_eq!(raw, case, "raw text should round-trip unchanged");
        }
    }
}
