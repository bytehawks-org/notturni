use std::fmt;

/// Un problema trovato in un manifest, con contesto sufficiente a ritrovarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub module: String,
    pub location: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]: {}", self.module, self.location, self.message)
    }
}

/// Raccolta di problemi: la validazione non si ferma al primo errore.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Issues(pub Vec<Issue>);

impl Issues {
    pub fn push(&mut self, module: &str, location: impl Into<String>, message: impl Into<String>) {
        self.0.push(Issue { module: module.to_string(), location: location.into(), message: message.into() });
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_result<T>(self, value: T) -> Result<T, Issues> {
        if self.0.is_empty() { Ok(value) } else { Err(self) }
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.0.iter().any(|i| i.message.contains(needle))
    }
}

impl fmt::Display for Issues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, issue) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "- {issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Issues {}
