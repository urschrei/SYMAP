use std::fmt;

/// A fatal problem with a deck, tied to a card (line) where possible.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub line: Option<usize>,
    pub message: String,
}

impl Error {
    pub fn at(line: usize, message: impl Into<String>) -> Self {
        Self {
            line: Some(line),
            message: message.into(),
        }
    }

    pub fn general(message: impl Into<String>) -> Self {
        Self {
            line: None,
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(n) => write!(f, "line {n}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for Error {}

/// A non-fatal note about a deck (ignored card, unimplemented elective, ...).
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}
