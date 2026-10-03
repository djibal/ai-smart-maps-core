//! Diagnostic log. A line carries edge ids, a duration, and an error code.
//! It has no field for a coordinate, a reporter key, or a user id.

use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    Unreachable,
    Refused,
    InvalidArtifact,
    TooSlow,
    UnknownEdge,
}

impl ErrorCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unreachable => "unreachable",
            Self::Refused => "refused",
            Self::InvalidArtifact => "invalid_artifact",
            Self::TooSlow => "too_slow",
            Self::UnknownEdge => "unknown_edge",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub edge_ids: Vec<String>,
    pub duration_us: u128,
    pub error_code: Option<ErrorCode>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiagnosticLog {
    entries: Vec<Entry>,
}

impl DiagnosticLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(
        &mut self,
        edge_ids: &[String],
        duration: Duration,
        error_code: Option<ErrorCode>,
    ) {
        self.entries.push(Entry {
            edge_ids: edge_ids.to_vec(),
            duration_us: duration.as_micros(),
            error_code,
        });
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// One line per record: `edges=<ids> duration_us=<n>` and, when set,
    /// `error=<code>`.
    pub fn render(&self) -> String {
        self.entries
            .iter()
            .map(|entry| {
                let mut line = format!(
                    "edges={} duration_us={}",
                    entry.edge_ids.join(","),
                    entry.duration_us
                );
                if let Some(code) = entry.error_code {
                    line.push_str(" error=");
                    line.push_str(code.as_str());
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
