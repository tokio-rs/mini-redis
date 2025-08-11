use std::fmt;

/// A compiled glob pattern for matching channel names
#[derive(Debug, Clone)]
pub struct Pattern {
    /// The original pattern string
    pub pattern: String,
    /// Compiled regex for efficient matching
    regex: regex::Regex,
}

impl Pattern {
    /// Create a new pattern from a glob-style string
    /// 
    /// Supports:
    /// - `*` matches any sequence of characters
    /// - `?` matches any single character
    /// - Other characters match literally
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        // Convert glob pattern to regex
        let regex_str = pattern
            .chars()
            .map(|c| match c {
                '*' => ".*".to_string(),
                '?' => ".".to_string(),
                '.' => "\\.".to_string(),
                '+' => "\\+".to_string(),
                '^' => "\\^".to_string(),
                '$' => "\\$".to_string(),
                '(' => "\\(".to_string(),
                ')' => "\\)".to_string(),
                '[' => "\\[".to_string(),
                ']' => "\\]".to_string(),
                '{' => "\\{".to_string(),
                '}' => "\\}".to_string(),
                '|' => "\\|".to_string(),
                '\\' => "\\\\".to_string(),
                _ => c.escape_default().to_string(),
            })
            .collect::<String>();
        
        let regex = regex::Regex::new(&format!("^{}$", regex_str))?;
        
        Ok(Pattern {
            pattern: pattern.to_string(),
            regex,
        })
    }
    
    /// Check if a channel name matches this pattern
    pub fn matches(&self, channel: &str) -> bool {
        self.regex.is_match(channel)
    }
    
    /// Get the original pattern string
    pub fn as_str(&self) -> &str {
        &self.pattern
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pattern_matching() {
        // Test basic patterns
        let pattern = Pattern::new("news.*").unwrap();
        assert!(pattern.matches("news.sports"));
        assert!(pattern.matches("news.weather"));
        assert!(!pattern.matches("sports.news"));
        
        // Test single character matching
        let pattern = Pattern::new("user:?123").unwrap();
        assert!(pattern.matches("user:a123"));
        assert!(pattern.matches("user:b123"));
        assert!(!pattern.matches("user:123"));
        assert!(!pattern.matches("user:ab123"));
        
        // Test literal characters
        let pattern = Pattern::new("test.channel").unwrap();
        assert!(pattern.matches("test.channel"));
        assert!(!pattern.matches("test.channel.extra"));
        
        // Test edge cases
        let pattern = Pattern::new("*").unwrap();
        assert!(pattern.matches(""));
        assert!(pattern.matches("anything"));
        
        let pattern = Pattern::new("?").unwrap();
        assert!(pattern.matches("a"));
        assert!(!pattern.matches(""));
        assert!(!pattern.matches("ab"));
    }
    
    #[test]
    fn test_special_characters() {
        // Test that special regex characters are escaped
        let pattern = Pattern::new("test.channel+").unwrap();
        assert!(pattern.matches("test.channel+"));
        assert!(!pattern.matches("test.channel"));
        
        let pattern = Pattern::new("test[channel]").unwrap();
        assert!(pattern.matches("test[channel]"));
        assert!(!pattern.matches("testchannel"));
    }
} 