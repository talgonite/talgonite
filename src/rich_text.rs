//! Rich text parsing and handling.

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
}
