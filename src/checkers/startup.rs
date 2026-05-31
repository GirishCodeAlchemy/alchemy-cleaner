use std::path::Path;

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::cmd_output;

pub struct StartupChecker;

impl Checker for StartupChecker {
    fn name(&self) -> &'static str {
        "startup"
    }

    fn description(&self) -> &'static str {
        "Login items, LaunchAgents, LaunchDaemons, and failing services"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Startup Items & Background Agents");

        // ── GUI Login Items (osascript) ────────────────────────────────────
        let login_items = cmd_output(
            "osascript",
            &["-e", "tell application \"System Events\" to get the name of every login item"],
        );

        match &login_items {
            Some(s) if !s.is_empty() => {
                let items: Vec<_> = s.split(", ").collect();
                let count = items.len();
                result.add_detail("GUI Login Items", format!("{count} items"));
                result.add_detail("Items", items.join(", "));

                if count > 15 {
                    result.add_finding(Finding::critical(
                        format!("{count} login items — excessive startup apps slow boot and consume RAM."),
                        "Remove unnecessary items: System Settings → General → Login Items.\n\
                         Keep only items you actively need at every startup.",
                        10,
                    ));
                } else if count > 8 {
                    result.add_finding(Finding::warn(
                        format!("{count} login items found. Consider trimming."),
                        "Review System Settings → General → Login Items.\n\
                         Disable apps you don't need at every login.",
                        5,
                    ));
                } else {
                    result.add_finding(Finding::ok(format!("{count} login items — healthy count.")));
                }
            }
            _ => {
                result.add_detail("GUI Login Items", "Unavailable (permission denied)".to_string());
            }
        }

        // ── LaunchAgents / LaunchDaemons (plist file count) ───────────────
        let dirs = [
            ("~/Library/LaunchAgents",   dirs::home_dir().map(|h| h.join("Library/LaunchAgents"))),
            ("/Library/LaunchAgents",    Some(Path::new("/Library/LaunchAgents").to_path_buf())),
            ("/Library/LaunchDaemons",   Some(Path::new("/Library/LaunchDaemons").to_path_buf())),
        ];

        let mut total_agents = 0usize;
        for (label, path_opt) in &dirs {
            let count = path_opt
                .as_ref()
                .and_then(|p| std::fs::read_dir(p).ok())
                .map(|entries| {
                    entries
                        .filter_map(|e| e.ok())
                        .filter(|e| {
                            e.path().extension().and_then(|x| x.to_str()) == Some("plist")
                        })
                        .count()
                })
                .unwrap_or(0);

            result.add_detail(*label, format!("{count} agents"));
            total_agents += count;
        }

        result.add_detail("Total Agents / Daemons", total_agents.to_string());

        if total_agents > 50 {
            result.add_finding(Finding::warn(
                format!("High agent/daemon count ({total_agents}) — may include orphaned software."),
                "Review ~/Library/LaunchAgents for plists from uninstalled apps.\n\
                 Use AppCleaner (free) to fully remove apps including their agents.",
                5,
            ));
        } else {
            result.add_finding(Finding::ok(format!(
                "Background agent count is normal ({total_agents} total)."
            )));
        }

        // ── Running user services (launchctl) ──────────────────────────────
        let failed_count = cmd_output(
            "sh",
            &["-c", "launchctl list 2>/dev/null | awk '$1 != \"-\" && $1 != \"0\" && $1 != \"PID\"' | tail -n +2 | wc -l | tr -d ' '"],
        )
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);

        result.add_detail("Failing User Services", failed_count.to_string());

        if failed_count > 15 {
            result.add_finding(Finding::warn(
                format!("{failed_count} user services have non-zero exit codes (respawn loops waste CPU)."),
                "Run: launchctl list | awk '$1 != 0 && $1 != \"-\"'\n\
                 to identify failing services. Remove orphaned plists from ~/Library/LaunchAgents.",
                5,
            ));
        } else {
            result.add_finding(Finding::ok(format!(
                "{failed_count} services with non-zero exits (normal range)."
            )));
        }

        result
    }
}

// ─── Home directory helper ────────────────────────────────────────────────────

mod dirs {
    use std::path::PathBuf;
    pub fn home_dir() -> Option<PathBuf> {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}
