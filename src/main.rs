mod actions;
mod checker;
mod cli;
mod config;
mod reporter;
mod scorer;
mod checkers;

use chrono::Local;
use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::time::Duration;

use crate::checker::{CheckResult, Level};
use cli::Cli;
use config::Config;

fn main() {
    let args = Cli::parse();

    // Disable colour when requested or when stdout is not a TTY
    if args.no_color {
        colored::control::set_override(false);
    }

    // ── Build runtime config ───────────────────────────────────────────────
    let config = Config {
        with_sudo:   args.with_sudo,
        only:        args.only.clone(),
        save_report: !args.no_report,
        no_color:    args.no_color,
    };

    // ── Collect all registered checkers ───────────────────────────────────
    let all = checkers::all_checkers();

    // ── --list mode ────────────────────────────────────────────────────────
    if args.list {
        reporter::print_checker_list(&all);
        return;
    }

    // ── Filter checkers if --only was provided ─────────────────────────────
    let to_run: Vec<_> = if config.only.is_empty() {
        all.iter().collect()
    } else {
        all.iter()
            .filter(|c| {
                config.only.iter().any(|name| c.name().eq_ignore_ascii_case(name))
            })
            .collect()
    };

    if to_run.is_empty() {
        eprintln!(
            "No checkers matched your --only filter: {:?}.\n\
             Run `alchemy-cleaner --list` to see available checker names.",
            config.only
        );
        std::process::exit(1);
    }

    // ── Run each checker with a live spinner ───────────────────────────────
    let is_tty = std::io::stdout().is_terminal();

    let pb = ProgressBar::new_spinner();
    if is_tty {
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner:.cyan}  {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.enable_steady_tick(Duration::from_millis(80));
    }

    let total = to_run.len();
    let mut results: Vec<CheckResult> = Vec::with_capacity(total);

    for (i, c) in to_run.iter().enumerate() {
        if is_tty {
            pb.set_message(format!(
                "[{}/{}]  {}...",
                i + 1,
                total,
                c.name()
            ));
        }

        let result = c.run(&config);
        let icon = section_icon(&result);

        if is_tty {
            pb.println(format!(
                "  {}  {} {}",
                icon,
                c.name().bold(),
                format!("({}/{})", i + 1, total).dimmed()
            ));
        }

        results.push(result);
    }

    pb.finish_and_clear();

    // ── Compute health score ───────────────────────────────────────────────
    let health = scorer::HealthScore::compute(&results);

    // ── Report file path ───────────────────────────────────────────────────
    let report_path: Option<PathBuf> = if config.save_report {
        let ts   = Local::now().format("%Y%m%d_%H%M%S");
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Some(PathBuf::from(format!("{home}/Desktop/alchemy_health_report_{ts}.txt")))
    } else {
        None
    };

    // ── Print full report to stdout (and optionally to a file) ────────────
    reporter::print_report(&results, &health, report_path.as_ref());

    // ── Interactive fix-runner — only when running in a real terminal ──────
    let interactive = is_tty && !args.no_interactive;
    if interactive {
        actions::run_interactive_menu();
    }
}

// ─── Helper — icon for a completed checker section ───────────────────────────

fn section_icon(result: &CheckResult) -> &'static str {
    let max = result
        .findings
        .iter()
        .map(|f| &f.level)
        .max()
        .unwrap_or(&Level::Ok);

    match max {
        Level::Ok   => "✅",
        Level::Warn => "⚠️ ",
        Level::Critical => "❌",
    }
}
