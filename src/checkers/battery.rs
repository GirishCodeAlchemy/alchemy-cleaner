use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::cmd_output;

pub struct BatteryChecker;

impl Checker for BatteryChecker {
    fn name(&self) -> &'static str {
        "battery"
    }

    fn description(&self) -> &'static str {
        "Battery health, cycle count, and charge capacity"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Battery Health");

        let batt_info = match cmd_output("system_profiler", &["SPPowerDataType"]) {
            Some(s) if !s.is_empty() => s,
            _ => {
                result.add_finding(Finding::ok(
                    "Battery information not available (desktop Mac or system_profiler failed).".to_string()
                ));
                return result;
            }
        };

        // ── Parse battery fields ───────────────────────────────────────────
        let parse_field = |label: &str| -> Option<String> {
            batt_info
                .lines()
                .find(|l| l.to_lowercase().contains(&label.to_lowercase()))
                .and_then(|l| l.split(':').nth(1))
                .map(|s| s.trim().to_string())
        };

        let condition      = parse_field("Condition").unwrap_or_else(|| "Unknown".to_string());
        let cycle_count    = parse_field("Cycle Count")
            .and_then(|s| s.parse::<u32>().ok());
        let max_capacity   = parse_field("Maximum Capacity").unwrap_or_else(|| "Unknown".to_string());
        let power_adapter  = parse_field("Connected")
            .unwrap_or_else(|| "Unknown".to_string());
        let full_capacity  = parse_field("Full Charge Capacity (mAh)");
        let design_capacity = parse_field("Design Capacity (mAh)");

        result.add_detail("Battery Condition",   condition.clone());
        result.add_detail("Maximum Capacity",    max_capacity.clone());
        result.add_detail("Power Adapter",       power_adapter);

        if let Some(cc) = cycle_count {
            result.add_detail("Cycle Count", cc.to_string());
        }
        if let (Some(fc), Some(dc)) = (&full_capacity, &design_capacity) {
            result.add_detail("Full / Design Capacity", format!("{fc} / {dc} mAh"));
        }

        // ── Condition assessment ───────────────────────────────────────────
        let cond_lower = condition.to_lowercase();
        if cond_lower.contains("replace") {
            result.add_finding(Finding::critical(
                format!("Battery condition: '{condition}' — replacement required."),
                "A degraded battery causes macOS to throttle the CPU to prevent shutdowns.\n\
                 Book a Genius Bar appointment. Battery replacement is far cheaper than a new Mac.",
                20,
            ));
        } else if cond_lower.contains("service") {
            result.add_finding(Finding::warn(
                format!("Battery condition: '{condition}' — service recommended."),
                "Schedule a battery check with Apple Support soon to avoid performance throttling.",
                10,
            ));
        } else if cond_lower == "normal" || cond_lower == "good" {
            result.add_finding(Finding::ok(format!(
                "Battery condition is good: '{condition}'."
            )));
        } else {
            result.add_finding(Finding::ok(format!(
                "Battery condition: '{condition}'."
            )));
        }

        // ── Cycle count assessment ─────────────────────────────────────────
        if let Some(cc) = cycle_count {
            if cc > 1000 {
                result.add_finding(Finding::warn(
                    format!("Cycle count {cc} exceeds Apple's rated 1000-cycle design life."),
                    "Consider battery replacement if you experience:\n\
                     → Unexpected shutdowns\n\
                     → Capacity below 80%\n\
                     → Excessive heat\n\
                     Replacement cost is typically $100–$200 at an Apple Store.",
                    8,
                ));
            } else if cc > 750 {
                result.add_finding(Finding::warn(
                    format!("Cycle count {cc} is approaching end of rated lifespan (1000 cycles)."),
                    "Start planning for battery replacement. Back up regularly.",
                    3,
                ));
            } else {
                result.add_finding(Finding::ok(format!(
                    "Cycle count {cc} is well within healthy range (<1000)."
                )));
            }
        }

        result
    }
}
