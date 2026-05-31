use std::path::Path;

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::cmd_output;

pub struct LogsChecker;

impl Checker for LogsChecker {
    fn name(&self) -> &'static str {
        "logs"
    }

    fn description(&self) -> &'static str {
        "Kernel panics (30 days), application crashes (7 days), and system error rate"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Crash & System Log History");

        // ── Kernel panics (last 30 days) ──────────────────────────────────
        let panic_dir = Path::new("/Library/Logs/DiagnosticReports");
        let panic_count = count_files_by_age(panic_dir, "*.panic", 30).unwrap_or(0);

        result.add_detail("Kernel panics (30 days)", panic_count.to_string());

        // Expose panic count so scorer can detect it
        if panic_count > 0 {
            // Store in details with a parseable key for scorer
            result.add_detail("Kernel panics (scorer)", panic_count.to_string());
        }

        if panic_count >= 5 {
            result.add_finding(Finding::critical(
                format!("{panic_count} kernel panics in the last 30 days — serious instability."),
                "Kernel panics indicate driver conflicts, failing hardware, or corrupted files.\n\
                 Steps:\n\
                 1. Run Apple Diagnostics: hold D at boot.\n\
                 2. Boot Safe Mode (hold Shift): if stable, a third-party driver is the culprit.\n\
                 3. Disk Utility → First Aid on Macintosh HD.\n\
                 4. If panics persist → reinstall macOS from Recovery (⌘+R).",
                25,
            ));
        } else if panic_count > 1 {
            result.add_finding(Finding::warn(
                format!("{panic_count} kernel panics in 30 days. Investigate if system feels unstable."),
                "Boot into Safe Mode to isolate third-party drivers.\n\
                 Run Disk Utility First Aid on the boot volume.",
                10,
            ));
        } else if panic_count == 1 {
            result.add_finding(Finding::warn(
                "1 kernel panic in the last 30 days. Isolated incidents are usually not alarming.".to_string(),
                "Monitor for recurrence. A single panic can result from a force-shutdown.",
                2,
            ));
        } else {
            result.add_finding(Finding::ok("No kernel panics in the last 30 days. ✨".to_string()));
        }

        // ── Application crashes (last 7 days) ─────────────────────────────
        let crash_dir_path = match std::env::var("HOME") {
            Ok(home) => std::path::PathBuf::from(home).join("Library/Logs/DiagnosticReports"),
            Err(_) => std::path::PathBuf::from("/tmp/no-such-dir"),
        };

        let crash_count = count_files_by_age(&crash_dir_path, "*.crash", 7).unwrap_or(0);
        result.add_detail("App crashes (7 days)", crash_count.to_string());

        if crash_count > 20 {
            result.add_finding(Finding::warn(
                format!("High app crash count: {crash_count} in 7 days."),
                "Check which app crashes most:\n\
                 ls -lt ~/Library/Logs/DiagnosticReports/*.crash | head -10\n\
                 Re-install the crashing app or check for macOS version incompatibility.",
                10,
            ));
        } else if crash_count > 5 {
            result.add_finding(Finding::warn(
                format!("{crash_count} app crashes in 7 days — above average."),
                "Review ~/Library/Logs/DiagnosticReports/ for the most frequent crashing app.",
                4,
            ));
        } else {
            result.add_finding(Finding::ok(format!(
                "App crash rate is low: {crash_count} crashes in the last 7 days."
            )));
        }

        // ── Most frequently crashing app ───────────────────────────────────
        if crash_count > 0 {
            if let Some(top) = top_crashing_app(&crash_dir_path) {
                result.add_detail("Most frequent crash", top);
            }
        }

        // ── System error rate (log show, last 1 hour) ──────────────────────
        // `log show` can be slow; we keep the window to 1 hour with a count only.
        let error_count = cmd_output(
            "sh",
            &["-c",
              "log show --last 1h \
               --predicate 'eventType == logEvent AND messageType == error' \
               --style compact 2>/dev/null \
               | grep -vc '^Filtering\\|^---\\|^Log'",
            ],
        )
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

        result.add_detail("System errors (last 1h)", error_count.to_string());

        if error_count > 500 {
            result.add_finding(Finding::warn(
                format!("Elevated system error rate: {error_count} errors in the last hour."),
                "Run: log show --last 1h --predicate 'messageType == error' \\\n\
                 --style compact | grep -v com.apple | head -30\n\
                 Focus on non-Apple subsystems — those point to third-party culprits.",
                5,
            ));
        } else {
            result.add_finding(Finding::ok(format!(
                "System error log rate is normal ({error_count} entries in last hour)."
            )));
        }

        result
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Count files matching `glob_pattern` in `dir` modified within the last `days` days.
fn count_files_by_age(dir: &Path, _glob_pattern: &str, days: u64) -> Option<usize> {
    let entries = std::fs::read_dir(dir).ok()?;
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(days * 86_400))?;

    let count = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            // match *.panic or *.crash by extension
            let ext = e.path().extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_string();
            let matches = ext == "panic" || ext == "crash" || ext == "ips";
            if !matches { return false; }
            // age filter
            e.metadata()
                .and_then(|m| m.modified())
                .map(|t| t > cutoff)
                .unwrap_or(false)
        })
        .count();

    Some(count)
}

/// Return `"N × AppName"` for the most frequently seen crash base-name.
fn top_crashing_app(dir: &Path) -> Option<String> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut counts: std::collections::HashMap<String, usize> = Default::default();
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(7 * 86_400))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("");
        if ext != "crash" { continue; }
        let mtime = entry.metadata().and_then(|m| m.modified()).ok()?;
        if mtime < cutoff { continue; }

        // Strip trailing "_<date>_<pid>.crash" → keep app name prefix
        let stem = path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?");
        // Take only the first component before the first '_'
        let app_name = stem.split('_').next().unwrap_or(stem).to_string();
        *counts.entry(app_name).or_default() += 1;
    }

    counts
        .into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(name, n)| format!("{n}× {name}"))
}
