use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::cmd_output;

pub struct LogsChecker;

impl Checker for LogsChecker {
    fn name(&self) -> &'static str {
        "logs"
    }

    fn description(&self) -> &'static str {
        "Kernel panics (typed), grouped app crashes, hang reports, error-subsystem breakdown, Jetsam kills"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Crash & System Log History");

        // System DiagnosticReports: kernel panics + JetsamEvent files (root-owned)
        let sys_dir  = Path::new("/Library/Logs/DiagnosticReports");
        // User DiagnosticReports: app crashes, hangs, spins (~/ prefix)
        let user_dir = home_path("Library/Logs/DiagnosticReports");

        // ── 1. Kernel panics (last 30 days) ───────────────────────────────
        let panic_count = count_by_ext(sys_dir, "panic", 30).unwrap_or(0);
        // ⚠  This value MUST be a plain u32 string — scorer reads first key
        //    containing "Kernel panics" and calls .parse::<u32>().
        result.add_detail("Kernel panics (30d)", panic_count.to_string());

        if panic_count > 0 {
            if let Some(ptype) = classify_panics(sys_dir) {
                result.add_detail("Panic types", ptype);
            }

            if panic_count >= 5 {
                result.add_finding(Finding::critical(
                    format!("{panic_count} kernel panics in 30 days — serious instability"),
                    "1. Run Apple Diagnostics: hold D at boot.\n\
                     2. Safe Mode (hold Shift) — stable = third-party driver culprit.\n\
                     3. Disk Utility → First Aid on Macintosh HD.\n\
                     4. GPU panics: check thermal ventilation, reset SMC/NVRAM.\n\
                     5. Persistent panics → reinstall macOS via Recovery (⌘+R).",
                    25,
                ));
            } else if panic_count > 1 {
                result.add_finding(Finding::warn(
                    format!("{panic_count} kernel panics in 30 days — investigate if unstable"),
                    "Boot Safe Mode to isolate third-party drivers/kexts.\n\
                     Run Disk Utility → First Aid on the boot volume.",
                    10,
                ));
            } else {
                result.add_finding(Finding::warn(
                    "1 kernel panic in 30 days — isolated, monitor for recurrence",
                    "A single panic is often a benign force-shutdown side-effect.\n\
                     If it recurs within days: Safe Mode → Disk Utility First Aid.",
                    2,
                ));
            }
        } else {
            result.add_finding(Finding::ok("No kernel panics in the last 30 days ✨"));
        }

        // ── 2. App crashes (.crash) + modern crash reports (.ips) — 7 days ──
        // User dir only — sys dir holds JetsamEvent .ips (counted separately).
        let crash_count = count_by_ext(&user_dir, "crash", 7).unwrap_or(0);
        let ips_count   = count_by_ext(&user_dir, "ips",   7).unwrap_or(0);
        let total_crashes = crash_count + ips_count;

        result.add_detail(
            "App crashes (7d)",
            if ips_count > 0 {
                format!("{crash_count} .crash + {ips_count} .ips = {total_crashes} total")
            } else {
                total_crashes.to_string()
            },
        );

        // Top 5 crashing apps
        let top_apps = top_crashing_apps(&user_dir, 5);
        if !top_apps.is_empty() {
            let summary = top_apps.iter()
                .map(|(name, n)| format!("{n}× {name}"))
                .collect::<Vec<_>>()
                .join("  ·  ");
            result.add_detail("Top crashing apps", summary);
        }

        if total_crashes > 20 {
            let app_list = top_apps.iter().take(3)
                .map(|(n, c)| format!("{n} ({c}×)"))
                .collect::<Vec<_>>()
                .join(", ");
            result.add_finding(Finding::warn(
                format!("High crash rate: {total_crashes} app crashes in 7 days"),
                format!(
                    "Most affected: {app_list}\n\
                     → Reinstall or update those apps.\n\
                     → Check macOS/app version compatibility.\n\
                     → Clear stale caches: rm -rf ~/Library/Caches/<AppBundleID>",
                ),
                10,
            ));
        } else if total_crashes > 5 {
            let worst = top_apps.first()
                .map(|(n, c)| format!("{n} ({c}×)"))
                .unwrap_or_default();
            result.add_finding(Finding::warn(
                format!("{total_crashes} app crashes in 7 days — above average"),
                format!(
                    "Worst offender: {worst}\n\
                     Inspect ~/Library/Logs/DiagnosticReports/ for patterns.\n\
                     Try reinstalling or updating the crashing app.",
                ),
                4,
            ));
        } else if total_crashes > 0 {
            result.add_finding(Finding::ok(format!(
                "App crash rate is low: {total_crashes} crash(es) in 7 days"
            )));
        } else {
            result.add_finding(Finding::ok("No app crashes in the last 7 days ✨"));
        }

        // ── 3. Hang / spin reports (last 7 days) ──────────────────────────
        let spin_count = count_by_ext(&user_dir, "spin", 7).unwrap_or(0)
            + count_by_ext(&user_dir, "hang", 7).unwrap_or(0);

        if spin_count > 0 {
            result.add_detail("Hang/spin reports (7d)", spin_count.to_string());
            let hung_apps = top_hung_apps(&user_dir);
            let apps_str = if hung_apps.is_empty() {
                String::new()
            } else {
                format!(" — {}", hung_apps.join(", "))
            };
            result.add_finding(Finding::warn(
                format!("{spin_count} app hang/freeze report(s) in 7 days{apps_str}"),
                "Hang reports = the app stopped responding (force-quit by macOS).\n\
                 → Update affected apps to latest version.\n\
                 → Open Activity Monitor: watch CPU % before the next freeze.\n\
                 → System-wide freezes can indicate overheating or memory pressure.",
                3,
            ));
        }

        // ── 4. System error rate (last 1 h) + top noisy processes ─────────
        let (error_count, top_procs) = system_errors_with_subsystems();
        result.add_detail("System errors (last 1h)", error_count.to_string());

        if !top_procs.is_empty() {
            let sub_str = top_procs.iter()
                .map(|(s, n)| format!("{s} ({n}×)"))
                .collect::<Vec<_>>()
                .join("  ·  ");
            result.add_detail("Noisy processes", sub_str);
        }

        if error_count > 500 {
            // List the top noisy processes (all of them — short Apple names like
            // "Storage" or "ospredictiond" are system-level and usually expected).
            let top_str = top_procs.iter()
                .map(|(s, n)| format!("{s} ({n}×)"))
                .collect::<Vec<_>>()
                .join(", ");
            let detail_str = if top_str.is_empty() {
                "No process breakdown available".to_string()
            } else {
                top_str
            };
            result.add_finding(Finding::warn(
                format!("Elevated system error rate: {error_count} errors in last hour"),
                format!(
                    "Top error-generating processes: {detail_str}\n\
                     Apple system processes (Storage, kernel, etc.) are usually expected.\n\
                     Investigate non-Apple apps with:\n\
                     log show --last 1h --predicate 'messageType == error' \\\n\
                     --style compact | grep -v com.apple | head -20",
                ),
                5,
            ));
        } else {
            result.add_finding(Finding::ok(format!(
                "System error log rate is normal ({error_count} entries in last hour)"
            )));
        }

        // ── 5. Memory-pressure kills — JetsamEvent .ips (last 24 h) ──────
        // JetsamEvent files are written to the SYSTEM dir (not user dir).
        // Confirmed filename: "JetsamEvent-YYYY-MM-DD-HHMMSS.ips"
        let jetsam_count = jetsam_kill_count(sys_dir);
        if jetsam_count > 0 {
            result.add_detail("Memory-pressure kills (24h)", jetsam_count.to_string());
            result.add_finding(Finding::warn(
                format!("{jetsam_count} process(es) killed by memory pressure in 24 h"),
                "macOS (Jetsam) terminated processes to reclaim RAM.\n\
                 → sudo purge  — reclaims inactive/cached pages immediately.\n\
                 → ps -Arco pid,rss,comm | sort -k2 -rn | head -10  (find hogs).\n\
                 → Close unused apps and browser tabs.\n\
                 → Recurrent kills: upgrade RAM (Intel) or reduce active workload.",
                6,
            ));
        }

        result
    }
}

// ─── Path helpers ─────────────────────────────────────────────────────────────

fn home_path(rel: &str) -> std::path::PathBuf {
    std::env::var("HOME")
        .map(|h| std::path::PathBuf::from(h).join(rel))
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp/no-such-dir"))
}

// ─── File-count helpers ───────────────────────────────────────────────────────

/// Count files with `ext` in `dir` modified within the last `days` days.
fn count_by_ext(dir: &Path, ext: &str, days: u64) -> Option<usize> {
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(days * 86_400))?;
    Some(
        std::fs::read_dir(dir).ok()?
            .filter_map(|e| e.ok())
            .filter(|e| {
                let file_ext = e.path().extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                file_ext == ext
                    && e.metadata()
                        .and_then(|m| m.modified())
                        .map(|t| t > cutoff)
                        .unwrap_or(false)
            })
            .count(),
    )
}

// ─── Kernel panic type classifier ────────────────────────────────────────────

/// Scan recent .panic filenames and classify them by type (GPU, Memory, Watchdog, Driver).
/// Returns a human-readable summary string, or `None` if no panics are found.
fn classify_panics(dir: &Path) -> Option<String> {
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(30 * 86_400))?;

    let (mut gpu, mut memory, mut watchdog, mut driver, mut other) =
        (0u32, 0u32, 0u32, 0u32, 0u32);

    for entry in std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("panic") {
            continue;
        }
        // Skip files older than the cutoff
        if entry.metadata().and_then(|m| m.modified())
            .map(|t| t < cutoff)
            .unwrap_or(true)
        {
            continue;
        }

        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();

        if name.contains("gpu") || name.contains("graphics") || name.contains("metal") {
            gpu += 1;
        } else if name.contains("memory") || name.contains("oom") || name.contains("malloc") {
            memory += 1;
        } else if name.contains("watchdog") || name.contains("timeout") {
            watchdog += 1;
        } else if name.contains("driver") || name.contains("kext") || name.contains("iokit") {
            driver += 1;
        } else {
            other += 1;
        }
    }

    let mut parts: Vec<String> = Vec::new();
    if gpu > 0      { parts.push(format!("GPU ×{gpu}")); }
    if memory > 0   { parts.push(format!("Memory ×{memory}")); }
    if watchdog > 0 { parts.push(format!("Watchdog ×{watchdog}")); }
    if driver > 0   { parts.push(format!("Driver/kext ×{driver}")); }
    if other > 0    { parts.push(format!("Other ×{other}")); }

    if parts.is_empty() { None } else { Some(parts.join(", ")) }
}

// ─── App crash breakdown ──────────────────────────────────────────────────────

/// Return top `n` crashing apps (name + count) from .crash and .ips files in the last 7 days.
fn top_crashing_apps(dir: &Path, n: usize) -> Vec<(String, usize)> {
    let cutoff = match SystemTime::now().checked_sub(Duration::from_secs(7 * 86_400)) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut counts: HashMap<String, usize> = HashMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new(); };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let ext = path.extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "crash" && ext != "ips" {
            continue;
        }
        if entry.metadata().and_then(|m| m.modified())
            .map(|t| t < cutoff)
            .unwrap_or(true)
        {
            continue;
        }

        // "Safari_2024-01-15_123456.crash"       → "Safari"
        // "com.apple.Safari_2024-01-15.crash"    → "com.apple.Safari"
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        let app_name = stem.split('_').next().unwrap_or(stem).to_string();

        *counts.entry(app_name).or_default() += 1;
    }

    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(n);
    sorted
}

// ─── Hang / spin report helpers ───────────────────────────────────────────────

/// Return unique app names (up to 3) that have .spin or .hang reports in the last 7 days.
fn top_hung_apps(dir: &Path) -> Vec<String> {
    let cutoff = match SystemTime::now().checked_sub(Duration::from_secs(7 * 86_400)) {
        Some(c) => c,
        None => return Vec::new(),
    };

    let mut names: Vec<String> = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new(); };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let ext = path.extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_lowercase();
        if ext != "spin" && ext != "hang" {
            continue;
        }
        if entry.metadata().and_then(|m| m.modified())
            .map(|t| t < cutoff)
            .unwrap_or(true)
        {
            continue;
        }

        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("?");
        let app_name = stem.split('_').next().unwrap_or(stem).to_string();
        if !names.contains(&app_name) {
            names.push(app_name);
        }
        if names.len() >= 3 {
            break;
        }
    }

    names
}

// ─── System log error rate + process grouping ────────────────────────────────

/// Run a single time-boxed `log show` (last 1h, errors only) and return
/// `(total_error_count, top_5_processes_by_error_count)`.
///
/// Parsing is done in Rust from `--style compact` output — avoids brittle awk.
fn system_errors_with_subsystems() -> (u32, Vec<(String, u32)>) {
    // NOTE: `log show --last 1h` typically takes 5–10 s. The checker runs in a
    // background thread so it does not stall the TUI. We do NOT use `timeout`
    // here because macOS ships without GNU coreutils `timeout` by default.
    let output = cmd_output("sh", &[
        "-c",
        "log show --last 1h \
         --predicate 'eventType == logEvent AND messageType == error' \
         --style compact 2>/dev/null",
    ])
    .unwrap_or_default();

    let mut total = 0u32;
    let mut proc_counts: HashMap<String, u32> = HashMap::new();

    for line in output.lines() {
        let t = line.trim();
        if t.is_empty()
            || t.starts_with("Filtering")
            || t.starts_with("---")
            || t.starts_with("Log ")
        {
            continue;
        }
        total += 1;

        // Compact format: <timestamp> <thread> <level> <actid> <pid> <Process>: <msg>
        // We want the field that ends with ':' — that is the process name.
        if let Some(proc) = extract_process_name(t) {
            if !proc.is_empty() {
                *proc_counts.entry(proc.to_string()).or_default() += 1;
            }
        }
    }

    let mut sorted: Vec<(String, u32)> = proc_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(5);
    (total, sorted)
}

/// Extract the process name from a single compact-format log line.
///
/// macOS `log show --style compact` format (actual observed format):
/// ```
/// DATE TIME E  ProcessName[PID:THREAD] [Subsystem:Category] message text
/// ```
/// Field indices (split on whitespace):
///   [0] date, [1] time, [2] single-letter level, [3] ProcessName[PID:THREAD], [4] [sub:cat], [5+] message
///
/// We extract the part before `[` in field [3] — that is the process name.
fn extract_process_name(line: &str) -> Option<&str> {
    let mut fields = line.split_ascii_whitespace();
    // Skip date[0], time[1], level[2]
    fields.next()?;
    fields.next()?;
    fields.next()?;
    // Field [3]: "ProcessName[PID:THREAD]"
    let proc_field = fields.next()?;
    let name = proc_field.split('[').next()?;
    if name.is_empty() { None } else { Some(name) }
}

// ─── Memory-pressure (Jetsam) kill counter ───────────────────────────────────

/// Count JetsamEvent .ips files created in `dir` within the last 24 hours.
///
/// JetsamEvent files live in the SYSTEM DiagnosticReports directory (not user dir).
/// Confirmed filename pattern: `JetsamEvent-YYYY-MM-DD-HHMMSS.ips`
fn jetsam_kill_count(dir: &Path) -> u32 {
    let cutoff = match SystemTime::now().checked_sub(Duration::from_secs(24 * 86_400)) {
        Some(c) => c,
        None => return 0,
    };

    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    let name = e.file_name();
                    let name_lower = name.to_string_lossy().to_lowercase();
                    // Only match confirmed JetsamEvent pattern
                    name_lower.starts_with("jetsameven")
                        && e.metadata()
                            .and_then(|m| m.modified())
                            .map(|t| t > cutoff)
                            .unwrap_or(false)
                })
                .count() as u32
        })
        .unwrap_or(0)
}
