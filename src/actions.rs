/// Interactive fix-runner and shared quick-win command table.
///
/// `QUICK_WIN_COMMANDS` is the single source of truth for the 6 quick-win
/// commands; both `reporter.rs` and this module reference it to avoid drift.

use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};

// ─── Shared command table ─────────────────────────────────────────────────────

/// Quick-win commands shown in the report and offered in the interactive menu.
///
/// Each entry is `(label, shell_command)`.  Commands that contain `sudo`
/// trigger an admin-password warning before execution.
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

// ─── Interactive menu ─────────────────────────────────────────────────────────

/// Display the interactive fix-runner after the report.
///
/// * Presents `QUICK_WIN_COMMANDS` via a `MultiSelect` prompt.
/// * Confirms before executing anything.
/// * Warns when a selected command requires `sudo`.
/// * Runs each command via `sh -c` so shell features (`;`, `~`) work.
///
/// Call only when `stdout().is_terminal()` is `true` and `--no-interactive`
/// was not passed.
pub fn run_interactive_menu() {
    println!(
        "\n\n{}",
        "  🔧  INTERACTIVE FIX RUNNER  ".on_blue().white().bold()
    );
    println!(
        "  {}\n",
        "Space = toggle  ·  Enter = confirm  ·  Ctrl-C = skip".dimmed()
    );

    // Build display labels with sudo indicator
    let labels: Vec<String> = QUICK_WIN_COMMANDS
        .iter()
        .map(|(label, cmd)| {
            if cmd.contains("sudo") {
                format!("{label}  🔐")
            } else {
                label.to_string()
            }
        })
        .collect();

    let theme = ColorfulTheme::default();

    let selection = match MultiSelect::with_theme(&theme)
        .items(&labels)
        .with_prompt("Choose quick-win actions to run")
        .interact()
    {
        Ok(s) => s,
        Err(_) => {
            println!(
                "\n  {} Interactive menu cancelled — no changes made.",
                "ℹ️ ".dimmed()
            );
            return;
        }
    };

    if selection.is_empty() {
        println!(
            "\n  {} Nothing selected — no changes made.",
            "✅".green()
        );
        return;
    }

    // Preview selected commands
    println!(
        "\n  {} About to run {} action(s):\n",
        "⚡".yellow(),
        selection.len()
    );
    for &idx in &selection {
        let (label, cmd) = QUICK_WIN_COMMANDS[idx];
        println!("  {}  ", format!("# {label}").dimmed());
        println!("  {}\n", cmd.green());
    }

    // Warn about sudo before the confirmation gate
    let needs_sudo = selection
        .iter()
        .any(|&idx| QUICK_WIN_COMMANDS[idx].1.contains("sudo"));

    if needs_sudo {
        println!(
            "  {} {} One or more selected commands require elevated privileges.\n\
             \t  Your terminal will prompt for your admin password.\n",
            "🔐".yellow(),
            "Heads up:".yellow().bold()
        );
    }

    let confirmed = Confirm::with_theme(&theme)
        .with_prompt("Run selected actions now?")
        .default(false)
        .interact()
        .unwrap_or(false);

    if !confirmed {
        println!(
            "\n  {} Run cancelled — no changes made.",
            "ℹ️ ".dimmed()
        );
        return;
    }

    // ── Execute ────────────────────────────────────────────────────────────────
    println!();
    for &idx in &selection {
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
