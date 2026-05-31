use sysinfo::Disks;

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::cmd_output;

pub struct DiskChecker;

impl Checker for DiskChecker {
    fn name(&self) -> &'static str {
        "disk"
    }

    fn description(&self) -> &'static str {
        "Boot volume usage, S.M.A.R.T. health, and largest home directories"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Storage & Disk Health");

        // ── Boot volume stats via sysinfo ─────────────────────────────────
        let disks = Disks::new_with_refreshed_list();
        let boot = disks.iter().find(|d| d.mount_point().to_string_lossy() == "/");

        if let Some(disk) = boot {
            let total_gb  = disk.total_space()     as f64 / 1_073_741_824.0;
            let avail_gb  = disk.available_space() as f64 / 1_073_741_824.0;
            let used_gb   = total_gb - avail_gb;
            let pct       = if total_gb > 0.0 { (used_gb / total_gb * 100.0) as u32 } else { 0 };

            result.add_detail("Total Size",  format!("{total_gb:.1} GB"));
            result.add_detail("Used",        format!("{used_gb:.1} GB ({pct}%)"));
            result.add_detail("Available",   format!("{avail_gb:.1} GB"));

            if pct >= 95 {
                result.add_finding(Finding::critical(
                    format!("Disk is critically full ({pct}%). macOS will become unstable."),
                    "URGENT: Free space immediately.\n\
                     → System Settings → General → Storage → Recommendations\n\
                     → Delete large files in ~/Downloads, empty Trash\n\
                     → 'du -sh ~/Library/Caches/*' to find large caches",
                    30,
                ));
            } else if pct >= 85 {
                result.add_finding(Finding::warn(
                    format!("Disk is {pct}% full. macOS requires ~10% free for efficient operation."),
                    "Free at least 20 GB via System Settings → General → Storage.",
                    15,
                ));
            } else if pct >= 70 {
                result.add_finding(Finding::warn(
                    format!("Disk at {pct}%. Consider a clean-up before it becomes critical."),
                    "Run System Settings → Storage → Recommendations for easy wins.",
                    5,
                ));
            } else {
                result.add_finding(Finding::ok(
                    format!("Disk usage is healthy ({pct}% used, {avail_gb:.1} GB free).")
                ));
            }
        } else {
            result.add_detail("Boot Volume", "Not found via sysinfo");
        }

        // ── S.M.A.R.T. status (macOS diskutil) ────────────────────────────
        if let Some(out) = cmd_output("diskutil", &["info", "/"]) {
            let smart = out
                .lines()
                .find(|l| l.to_lowercase().contains("smart status"))
                .and_then(|l| l.split(':').nth(1))
                .map(str::trim)
                .unwrap_or("Not Available");

            result.add_detail("S.M.A.R.T. Status", smart);

            let smart_lower = smart.to_lowercase();
            if smart_lower.contains("fail") {
                result.add_finding(Finding::critical(
                    format!("S.M.A.R.T. Status: {smart} — drive hardware is failing."),
                    "CRITICAL: Back up all data immediately to Time Machine or external drive.\n\
                     Book an Apple Genius Bar appointment or contact Apple Support.\n\
                     A failing drive alone is a strong signal to replace hardware.",
                    40,
                ));
            } else if smart_lower.contains("verified") || smart_lower.contains("ok") || smart_lower.contains("passed") {
                result.add_finding(Finding::ok(format!("S.M.A.R.T. Status: {smart} — drive health is good.")));
            } else {
                result.add_detail("S.M.A.R.T. Note", "Status unclear — may be an APFS container");
            }
        }

        // ── Additional mounted volumes ─────────────────────────────────────
        let disks2 = Disks::new_with_refreshed_list();
        let non_boot: Vec<_> = disks2
            .iter()
            .filter(|d| d.mount_point().to_string_lossy() != "/")
            .collect();

        if !non_boot.is_empty() {
            result.add_detail("Other Volumes", non_boot.len().to_string());
        }

        // ── Largest home directories (top 6, using du) ────────────────────
        result.add_detail("Large Home Dirs (top 6)", String::new());
        if let Ok(home) = std::env::var("HOME") {
            let pattern = format!("{home}/*/");
            if let Some(out) = cmd_output("sh", &["-c", &format!("du -sh {pattern} 2>/dev/null | sort -rh | head -6")]) {
                for line in out.lines() {
                    let mut parts = line.splitn(2, '\t');
                    let size = parts.next().unwrap_or("?");
                    let path = parts.next().unwrap_or("?")
                        .replace(&home, "~");
                    result.add_detail(format!("  {path}"), size);
                }
            }
        }

        result
    }
}
