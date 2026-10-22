use std::fmt;

/// One problem found while loading game data, tied to the file it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct DataError {
    /// Path relative to `assets/data`, e.g. `maps/foundry_01.json`.
    pub file: String,
    pub message: String,
}

impl DataError {
    pub fn new(file: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file, self.message)
    }
}

impl std::error::Error for DataError {}

/// Every problem found in one load pass, so a modder sees all mistakes at once.
#[derive(Debug, Clone, PartialEq)]
pub struct DataErrors(pub Vec<DataError>);

impl fmt::Display for DataErrors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, e) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{e}")?;
        }
        Ok(())
    }
}

impl std::error::Error for DataErrors {}
