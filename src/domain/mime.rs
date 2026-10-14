/// A MIME type as known to the local MIME database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MimeType {
    /// Canonical MIME name, e.g. `text/x-rust`.
    pub name: String,
    /// Lowercase file extensions (without the dot) mapped to this type.
    pub extensions: Vec<String>,
    /// Superclasses this type inherits from, as recorded in `subclasses`.
    pub parents: Vec<String>,
    /// Aliases that resolve to this type.
    pub aliases: Vec<String>,
}

impl MimeType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            extensions: Vec::new(),
            parents: Vec::new(),
            aliases: Vec::new(),
        }
    }

    /// The top-level category (`text`, `image`, …), i.e. the part before the first `/`.
    pub fn top_level(&self) -> &str {
        self.name.split('/').next().unwrap_or(&self.name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_is_the_category() {
        assert_eq!(MimeType::new("image/png").top_level(), "image");
        assert_eq!(MimeType::new("text/x-rust").top_level(), "text");
    }
}
