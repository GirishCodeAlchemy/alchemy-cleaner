/// Core trait and types for the alchemy-cleaner checker system.
///
/// # Extending the tool
/// To add a new diagnostic feature:
///   1. Create `src/checkers/your_feature.rs`
///   2. Implement `Checker` for your struct
///   3. Add it to the `all_checkers()` list in `src/checkers/mod.rs`
///
/// That's all — the runner, reporter, and scorer pick it up automatically.

// ─── Severity level of a single finding ──────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Everything looks good — informational line only.
    Ok,
    /// Potential issue, worth monitoring.
    Warn,
    /// Confirmed problem that degrades performance or stability.
    Critical,
}

// ─── A single finding within a checker section ───────────────────────────────

#[derive(Debug, Clone)]
pub struct Finding {
    pub level: Level,
    /// One-line human-readable diagnosis.
    pub message: String,
    /// Optional, actionable fix the user can apply manually.
    pub solution: Option<String>,
    /// Points deducted from the 0-100 health score for this finding.
    pub score_deduction: u32,
}

impl Finding {
    // Use `AsRef<str>` instead of `Into<String>` to avoid type-inference
    // ambiguity introduced by clap 4.x which adds `From<&str>` for its own types.
    pub fn ok(message: impl AsRef<str>) -> Self {
        Self {
            level: Level::Ok,
            message: message.as_ref().to_string(),
            solution: None,
            score_deduction: 0,
        }
    }

    pub fn warn(message: impl AsRef<str>, solution: impl AsRef<str>, deduction: u32) -> Self {
        Self {
            level: Level::Warn,
            message: message.as_ref().to_string(),
            solution: Some(solution.as_ref().to_string()),
            score_deduction: deduction,
        }
    }

    pub fn critical(
        message: impl AsRef<str>,
        solution: impl AsRef<str>,
        deduction: u32,
    ) -> Self {
        Self {
            level: Level::Critical,
            message: message.as_ref().to_string(),
            solution: Some(solution.as_ref().to_string()),
            score_deduction: deduction,
        }
    }
}

// ─── Result returned by every Checker ────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Display name for this section header.
    pub section: String,
    /// All findings produced by this checker (mix of Ok / Warn / Critical).
    pub findings: Vec<Finding>,
    /// Freeform key-value pairs for the "details" block (e.g. "Total RAM: 16 GB").
    pub details: Vec<(String, String)>,
}

impl CheckResult {
    pub fn new(section: impl Into<String>) -> Self {
        Self {
            section: section.into(),
            findings: Vec::new(),
            details: Vec::new(),
        }
    }

    pub fn add_finding(&mut self, f: Finding) {
        self.findings.push(f);
    }

    // Use `AsRef<str>` to avoid `Into<String>` ambiguity from clap's From impls.
    pub fn add_detail(&mut self, key: impl AsRef<str>, value: impl AsRef<str>) {
        self.details.push((key.as_ref().to_string(), value.as_ref().to_string()));
    }

    /// Total score points this section deducts.
    pub fn total_deduction(&self) -> u32 {
        self.findings.iter().map(|f| f.score_deduction).sum()
    }

    /// Highest severity level found in this section.
    /// Reserved for future use (e.g. per-section colour in the reporter).
    #[allow(dead_code)]
    pub fn max_level(&self) -> Level {
        self.findings
            .iter()
            .map(|f| f.level.clone())
            .max()
            .unwrap_or(Level::Ok)
    }
}

// ─── The Checker trait ────────────────────────────────────────────────────────

pub trait Checker: Send + Sync {
    /// Short, unique name used in `--only <name>` filtering.
    fn name(&self) -> &'static str;
    /// One-line description shown in `--list` output.
    fn description(&self) -> &'static str;
    /// Execute the diagnostic and return findings. Must not mutate system state.
    fn run(&self, config: &crate::config::Config) -> CheckResult;
}
