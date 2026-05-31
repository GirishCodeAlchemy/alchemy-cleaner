mod checker;
mod cli;
mod config;
mod reporter;
mod scorer;
mod checkers;

use chrono::Local;
use clap::Parser;
use std::path::PathBuf;

use cli::Cli;
use config::Config;

fn main() {
    let args = Cli::parse();

    // Disable colour when requested or when stdout is not a tty
    if args.no_color {
        colored::control::set_override(false);
    }

    // ── Build runtime config ───────────────────────────────────────────────
    let config = Config {
        with_sudo: args.with_sudo,
        only:      args.only.clone(),
        save_report: !args.no_report,
        no_color:   args.no_color,
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
                config.only.iter().any(|name| {
                    c.name().eq_ignore_ascii_case(name)
                })
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

    // ── Run each checker ───────────────────────────────────────────────────
    let results: Vec<_> = to_run.iter().map(|c| c.run(&config)).collect();

    // ── Compute health score ───────────────────────────────────────────────
    let health = scorer::HealthScore::compute(&results);

    // ── Report file path ───────────────────────────────────────────────────
    let report_path: Option<PathBuf> = if config.save_report {
        let ts = Local::now().format("%Y%m%d_%H%M%S");
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Some(PathBuf::from(format!("{home}/Desktop/alchemy_health_report_{ts}.txt")))
    } else {
        None
    };

    // ── Print report to stdout (and optionally to file) ────────────────────
    reporter::print_report(&results, &health, report_path.as_ref());
}
