/// Shared quick-win command table and post-TUI executor.
///
/// `QUICK_WIN_COMMANDS` is the single source of truth for the quick-win
/// commands; both `reporter.rs` and `tui.rs` reference it to avoid drift.
///
/// `execute_selected` is called **after** the TUI exits (in normal terminal
/// mode) so that `sudo` password prompts render correctly in the user's
/// shell rather than inside the alternate-screen buffer.

use colored::Colorize;

// ─── Shared command table ─────────────────────────────────────────────────────

/// Quick-win commands shown in the report and offered in the interactive TUI.
///
/// Each entry is `(label, shell_command)`.  Commands that contain `sudo`
/// trigger an admin-password prompt when executed.
pub const QUICK_WIN_COMMANDS: &[(&str, &str)] = &[
    (
        "Flush DNS cache",
        "sudo dscacheutil -flushcache; sudo killall -HUP mDNSResponder",
    ),
    ("Rebuild Spotlight index", "sudo mdutil -E /"),
    (
        "Top 10 memory hogs",
        "ps -Arco pid,rss,comm | sort -k2 -rn | head -11",
    ),
    ("Clear font cache", "atsutil databases -remove"),
    (
        "Open Disk Utility",
        "open /System/Applications/Utilities/Disk\\ Utility.app",
    ),
    (
        "Find large files (>500 MB) in home",
        "find ~ -size +500M -not -path '*/.*' 2>/dev/null",
    ),
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
        .any(|&i| QUICK_WIN_COMMANDS[i].1.contains("sudo"));

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
        let (label, cmd) = QUICK_WIN_COMMANDS[idx];
        println!(
            "  {}  {}\n",
            "▶".cyan().bold(),
            format!("Running: {label}").bold()
        );

        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(cmd)
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
