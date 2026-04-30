//! Rich text parsing and handling.

use i_slint_core::styled_text::{parse_markdown, StyledText};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichTextChunk {
    pub text: String,
    pub color_code: Option<char>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RichText {
    pub chunks: Vec<RichTextChunk>,
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
            .map(|c| {
                let mut escaped = String::with_capacity(c.text.len() + 20);
                for ch in c.text.chars() {
                    match ch {
                        // HTML Escapes
                        '&' => escaped.push_str("&amp;"),
                        '<' => escaped.push_str("&lt;"),
                        '>' => escaped.push_str("&gt;"),
                        '"' => escaped.push_str("&quot;"),
                        '\'' => escaped.push_str("&#39;"),
                        // Markdown escapes (note: * and _ are intentionally unescaped to allow bold/italics)
                        '\\' => escaped.push_str("\\\\"),
                        '`' => escaped.push_str("\\`"),
                        '~' => escaped.push_str("\\~"),
                        '[' => escaped.push_str("\\["),
                        ']' => escaped.push_str("\\]"),
                        '#' => escaped.push_str("\\#"),
                        _ => escaped.push(ch),
                    }
                }

                if let Some(code) = c.color_code {
                    if let Some(color) = RichTextColor::from_char_code(code) {
                        let [r, g, b, _] = color.to_color();
                        return format!("<font color=\"#{:02x}{:02x}{:02x}\">{}</font>", r, g, b, escaped);
                    }
                }
                
                escaped
            })
            .collect()
    }

    pub fn to_slint_styled_text(&self) -> StyledText {
        parse_markdown(&self.to_html_string(), &[] as &[StyledText])
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
    Invisible
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
}
