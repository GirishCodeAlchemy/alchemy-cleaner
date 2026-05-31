use crate::checker::{CheckResult, Level};

/// Final health verdict derived from the aggregate score.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Healthy,
    NeedsAttention,
    Degraded,
    Critical,
}

impl Verdict {
    pub fn label(&self) -> &'static str {
        match self {
            Verdict::Healthy => "HEALTHY",
            Verdict::NeedsAttention => "NEEDS ATTENTION",
            Verdict::Degraded => "DEGRADED — ACTION REQUIRED",
            Verdict::Critical => "CRITICAL — CONSIDER REINSTALL",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            Verdict::Healthy => "✅",
            Verdict::NeedsAttention => "⚠️ ",
            Verdict::Degraded => "❌",
            Verdict::Critical => "🚨",
        }
    }

    /// Erase recommendation requires evidence of *software corruption*, not just capacity exhaustion.
    /// RAM/swap pressure is a capacity problem — reinstalling macOS won't fix it.
    /// We only escalate to Recommended when panics or SMART failure confirm real corruption.
    pub fn erase_recommendation(&self, panic_count: u32, smart_failure: bool) -> EraseRecommendation {
        match self {
            Verdict::Healthy | Verdict::NeedsAttention => EraseRecommendation::NotNeeded,
            Verdict::Degraded => EraseRecommendation::TryOtherFirst,
            Verdict::Critical => {
                if panic_count >= 1 || smart_failure {
                    EraseRecommendation::Recommended
                } else {
                    // Score is critically low but no evidence of software corruption —
                    // likely RAM/capacity exhaustion or too many background agents.
                    EraseRecommendation::TryOtherFirst
                }
            }
        }
    }
}

/// Advice about whether to erase & reinstall macOS.
#[derive(Debug, Clone)]
pub enum EraseRecommendation {
    NotNeeded,
    TryOtherFirst,
    Recommended,
}

/// Aggregated health score calculated from all check results.
#[derive(Debug)]
pub struct HealthScore {
    pub score: u32,          // 0–100
    pub verdict: Verdict,
    pub smart_failure: bool, // special flag — overrides score for erase advice
    pub panic_count: u32,    // surfaced from logs checker
}

impl HealthScore {
    /// Compute score and verdict from all results.
    pub fn compute(results: &[CheckResult]) -> Self {
        let total_deduction: u32 = results.iter().map(|r| r.total_deduction()).sum();
        let score = 100u32.saturating_sub(total_deduction).clamp(0, 100);

        // Check for special signals
        let smart_failure = results.iter().any(|r| {
            r.findings.iter().any(|f| {
                f.level == Level::Critical && f.message.to_lowercase().contains("s.m.a.r.t")
            })
        });

        let panic_count = results
            .iter()
            .find(|r| r.section.to_lowercase().contains("crash"))
            .and_then(|r| {
                r.details
                    .iter()
                    .find(|(k, _)| k.contains("Kernel panics"))
                    .and_then(|(_, v)| v.parse::<u32>().ok())
            })
            .unwrap_or(0);

        let verdict = match score {
            80..=100 => Verdict::Healthy,
            55..=79 => Verdict::NeedsAttention,
            30..=54 => Verdict::Degraded,
            _ => Verdict::Critical,
        };

        Self { score, verdict, smart_failure, panic_count }
    }

    /// Progress bar string for display.
    pub fn bar(&self, width: usize) -> String {
        let filled = (self.score as usize * width) / 100;
        let empty = width.saturating_sub(filled);
        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    }
}
