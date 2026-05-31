use sysinfo::{CpuRefreshKind, RefreshKind, System};

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::sysctl_str;

pub struct CpuChecker;

impl Checker for CpuChecker {
    fn name(&self) -> &'static str {
        "cpu"
    }

    fn description(&self) -> &'static str {
        "CPU load averages and instantaneous usage"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("CPU Analysis");

        // ── Logical core count ─────────────────────────────────────────────
        let lcores: u32 = sysctl_str("hw.logicalcpu")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        result.add_detail("Logical Cores", lcores.to_string());

        // ── Load averages ──────────────────────────────────────────────────
        let load = System::load_average();
        result.add_detail(
            "Load Average (1m/5m/15m)",
            format!("{:.2} / {:.2} / {:.2}", load.one, load.five, load.fifteen),
        );
        result.add_detail(
            "Saturation threshold",
            format!(">{lcores} = saturated"),
        );

        // ── Evaluate 5-minute load ─────────────────────────────────────────
        if load.five > lcores as f64 * 1.5 {
            result.add_finding(Finding::critical(
                format!(
                    "5-minute load ({:.2}) is >1.5× core count ({lcores}) — CPU is severely saturated.",
                    load.five
                ),
                "Open Activity Monitor → CPU tab, sort by '% CPU' descending.\n\
                 Identify the top offender; if it's a background service you don't recognise,\n\
                 force-quit it and research the process name.",
                15,
            ));
        } else if load.five > lcores as f64 {
            result.add_finding(Finding::warn(
                format!(
                    "5-minute load ({:.2}) exceeds core count ({lcores}) — CPU under sustained stress.",
                    load.five
                ),
                "Check Activity Monitor → CPU for processes consuming >50% continuously.",
                8,
            ));
        } else if load.five > lcores as f64 * 0.75 {
            result.add_finding(Finding::warn(
                format!(
                    "5-minute load ({:.2}) is at 75%+ of capacity ({lcores} cores) — mild pressure.",
                    load.five
                ),
                "Monitor over the next few minutes; brief spikes are normal.",
                3,
            ));
        } else {
            result.add_finding(Finding::ok(format!(
                "CPU load is healthy (5m={:.2} vs {lcores} logical cores).",
                load.five
            )));
        }

        // ── Instantaneous CPU usage via sysinfo ───────────────────────────
        // sysinfo 0.36: use ::nothing() instead of ::new()
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_cpu(CpuRefreshKind::nothing().with_cpu_usage()),
        );
        // sysinfo requires two samples to compute CPU % — sleep 200ms
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_cpu_usage();

        let global_pct = sys.global_cpu_usage();
        result.add_detail("Current CPU Usage", format!("{global_pct:.1}%"));

        if global_pct > 90.0 {
            result.add_finding(Finding::critical(
                format!("Instantaneous CPU usage is critically high: {global_pct:.1}%"),
                "Check Activity Monitor for runaway processes.",
                10,
            ));
        } else if global_pct > 70.0 {
            result.add_finding(Finding::warn(
                format!("Instantaneous CPU usage elevated: {global_pct:.1}%"),
                "Normal for short bursts; investigate if sustained.",
                3,
            ));
        }

        result
    }
}
