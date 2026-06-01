/// Shared quick-win command table and post-TUI executor.
///
/// `QUICK_WIN_COMMANDS` is the single source of truth for the quick-win
/// commands; both `reporter.rs` and `tui.rs` reference it to avoid drift.
///
/// `execute_selected` is called **after** the TUI exits (in normal terminal
/// mode) so that `sudo` password prompts render correctly in the user's
/// shell rather than inside the alternate-screen buffer.

use colored::Colorize;

// ─── Command entry ────────────────────────────────────────────────────────────

/// A single quick-win action entry.
pub struct QuickWinCmd {
    /// Display category (used in TUI to group items under headers).
    pub category: &'static str,
    /// Short label shown in lists and report.
    pub label:    &'static str,
    /// One-sentence description of what the command does.
    pub desc:     &'static str,
    /// The shell command to execute.
    pub cmd:      &'static str,
}

// ─── Shared command table ─────────────────────────────────────────────────────

/// Quick-win commands shown in the report and offered in the interactive TUI.
///
/// Grouped by category; no `sudo rm -rf` on system paths — user-space only.
pub const QUICK_WIN_COMMANDS: &[QuickWinCmd] = &[
    // ── Memory ───────────────────────────────────────────────────────────────
    QuickWinCmd {
        category: "Memory",
        label:    "Purge inactive memory",
        desc:     "Asks macOS to reclaim inactive/cached pages back to the free pool.",
        cmd:      "sudo purge",
    },
    QuickWinCmd {
        category: "Memory",
        label:    "Top 10 memory hogs",
        desc:     "Lists the 10 processes consuming the most resident memory right now.",
        cmd:      "ps -Arco pid,rss,comm | sort -k2 -rn | head -11",
    },

    // ── Cache Cleanup ────────────────────────────────────────────────────────
    QuickWinCmd {
        category: "Cache Cleanup",
        label:    "Clear user cache folder",
        desc:     "Removes stale app caches from ~/Library/Caches (user-space only; safe).",
        cmd:      "rm -rf ~/Library/Caches/* 2>/dev/null; echo 'User cache cleared.'",
    },
    QuickWinCmd {
        category: "Cache Cleanup",
        label:    "Clear Xcode derived data",
        desc:     "Deletes Xcode build artifacts — reclaims several GBs. Xcode rebuilds on next build.",
        cmd:      "rm -rf ~/Library/Developer/Xcode/DerivedData && echo 'DerivedData cleared.'",
    },
    QuickWinCmd {
        category: "Cache Cleanup",
        label:    "Clear crash & diagnostic reports",
        desc:     "Removes old .crash and .ips files from your user diagnostic logs.",
        cmd:      "rm -f ~/Library/Logs/DiagnosticReports/*.crash ~/Library/Logs/DiagnosticReports/*.ips 2>/dev/null; echo 'Crash reports cleared.'",
    },
    QuickWinCmd {
        category: "Cache Cleanup",
        label:    "Rebuild font cache (restart fontd)",
        desc:     "Removes the font database then signals fontd to rebuild — fixes font glitches.",
        cmd:      "atsutil databases -remove; sudo killall -HUP fontd",
    },

    // ── Disk & Storage ───────────────────────────────────────────────────────
    QuickWinCmd {
        category: "Disk & Storage",
        label:    "Show disk usage",
        desc:     "Prints free and used space for every mounted volume.",
        cmd:      "df -h",
    },
    QuickWinCmd {
        category: "Disk & Storage",
        label:    "Find large files (>500 MB) in home",
        desc:     "Searches your home folder for files bigger than 500 MB.",
        cmd:      "find ~ -size +500M -not -path '*/.*' 2>/dev/null",
    },
    QuickWinCmd {
        category: "Disk & Storage",
        label:    "Largest user directories",
        desc:     "Shows sizes of Desktop, Downloads, Documents, Movies, Music, Pictures.",
        cmd:      "du -sh ~/Desktop ~/Downloads ~/Documents ~/Movies ~/Music ~/Pictures 2>/dev/null | sort -rh | head -10",
    },
    QuickWinCmd {
        category: "Disk & Storage",
        label:    "Remove .DS_Store files in home",
        desc:     "Deletes macOS metadata files scattered across your home folder.",
        cmd:      "find ~ -name '.DS_Store' -delete 2>/dev/null; echo '.DS_Store files removed.'",
    },
    QuickWinCmd {
        category: "Disk & Storage",
        label:    "List Time Machine local snapshots",
        desc:     "Shows local TM snapshots that may be consuming significant disk space.",
        cmd:      "tmutil listlocalsnapshots /",
    },

    // ── Network ──────────────────────────────────────────────────────────────
    QuickWinCmd {
        category: "Network",
        label:    "Flush DNS cache",
        desc:     "Clears the local DNS resolver cache and restarts mDNSResponder.",
        cmd:      "sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder",
    },
    QuickWinCmd {
        category: "Network",
        label:    "Show active TCP connections",
        desc:     "Lists all established connections with PIDs and remote addresses.",
        cmd:      "lsof -i -P -n | grep ESTABLISHED",
    },

    // ── System Maintenance ───────────────────────────────────────────────────
    QuickWinCmd {
        category: "System Maintenance",
        label:    "Run daily maintenance scripts",
        desc:     "Executes Apple's built-in daily cleanup (log rotation, tmp cleanup).",
        cmd:      "sudo periodic daily",
    },
    QuickWinCmd {
        category: "System Maintenance",
        label:    "Run weekly maintenance scripts",
        desc:     "Executes Apple's weekly tasks (rebuild locate database, etc.).",
        cmd:      "sudo periodic weekly",
    },
    QuickWinCmd {
        category: "System Maintenance",
        label:    "Rebuild Spotlight index",
        desc:     "Erases and re-indexes Spotlight metadata for the entire boot volume.",
        cmd:      "sudo mdutil -E /",
    },
    QuickWinCmd {
        category: "System Maintenance",
        label:    "Rebuild Launch Services database",
        desc:     "Resets Open With menus and document associations. Fixes 'wrong app opens' issues.",
        cmd:      "sudo /System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister -kill -r -domain local -domain system -domain user",
    },
    QuickWinCmd {
        category: "System Maintenance",
        label:    "Verify disk (First Aid read-only)",
        desc:     "Runs diskutil read-only integrity check on the boot volume — no changes made.",
        cmd:      "diskutil verifyVolume /",
    },

    // ── Developer Tools ──────────────────────────────────────────────────────
    QuickWinCmd {
        category: "Developer Tools",
        label:    "Homebrew cleanup",
        desc:     "Removes outdated packages, caches, and old formula versions from Homebrew.",
        cmd:      "brew cleanup --prune=all",
    },
    QuickWinCmd {
        category: "Developer Tools",
        label:    "Homebrew doctor",
        desc:     "Checks your Homebrew installation for common problems and misconfigurations.",
        cmd:      "brew doctor",
    },
    QuickWinCmd {
        category: "Developer Tools",
        label:    "Clear npm cache",
        desc:     "Forcefully removes the npm cache (~/.npm). npm rebuilds it as needed.",
        cmd:      "npm cache clean --force",
    },
    QuickWinCmd {
        category: "Developer Tools",
        label:    "Clear pip / pip3 cache",
        desc:     "Removes cached pip wheel files to free disk space.",
        cmd:      "pip cache purge 2>/dev/null || pip3 cache purge 2>/dev/null; echo 'pip cache cleared.'",
    },

    // ── Utilities ────────────────────────────────────────────────────────────
    QuickWinCmd {
        category: "Utilities",
        label:    "Battery health info",
        desc:     "Shows cycle count, condition, and maximum capacity of your battery.",
        cmd:      "system_profiler SPPowerDataType | grep -E 'Cycle Count|Condition|Maximum Capacity'",
    },
    QuickWinCmd {
        category: "Utilities",
        label:    "Open Disk Utility",
        desc:     "Launches the macOS Disk Utility app for partition and repair operations.",
        cmd:      "open /System/Applications/Utilities/Disk\\ Utility.app",
    },
];

// ─── Post-TUI executor ────────────────────────────────────────────────────────

/// Run the commands chosen by the user in the TUI.
///
/// `indices` is the list of `QUICK_WIN_COMMANDS` indices returned by
/// `tui::run()`.  Each command is run via `sh -c` so shell features
/// (`;`, `~`, redirections) work correctly.
///
/// This function must be called **outside** the ratatui alternate screen so
/// that `sudo` prompts and command output render in the user's normal terminal.
pub fn execute_selected(indices: &[usize]) {
    if indices.is_empty() {
        return;
    }

    println!(
        "\n\n{}",
        "  ⚡  RUNNING SELECTED ACTIONS  ".on_blue().white().bold()
    );

    // Warn if any selected command requires sudo
    let needs_sudo = indices
        .iter()
        .any(|&i| QUICK_WIN_COMMANDS[i].cmd.contains("sudo"));

    if needs_sudo {
        println!(
            "\n  {} {} One or more commands require elevated privileges.\n\
             \t  Your terminal will prompt for your admin password.\n",
            "🔐".yellow(),
            "Heads up:".yellow().bold()
        );
    }

    println!();
    for &idx in indices {
        let qw = &QUICK_WIN_COMMANDS[idx];
        println!(
            "  {}  {}\n",
            "▶".cyan().bold(),
            format!("Running: {}", qw.label).bold()
        );
        println!("  {}  {}\n", "→".dimmed(), qw.cmd.dimmed());

        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(qw.cmd)
            .status();

        match status {
            Ok(s) if s.success() => println!("  {}  Done.\n", "✅".green()),
            Ok(s) => println!(
                "  {}  Exited with code {}\n",
                "⚠️ ".yellow(),
                s.code().unwrap_or(-1)
            ),
            Err(e) => println!("  {}  Failed to spawn: {e}\n", "❌".red()),
        }
    }

    println!(
        "  {}  All done!  Re-run alchemy-cleaner to see the updated health score.\n",
        "🎉".green()
    );
}
