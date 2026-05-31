mod actions;
mod checker;
mod cli;
mod config;
mod reporter;
mod scorer;
mod checkers;
mod tui;

use chrono::Local;
use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use std::io::IsTerminal as _;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use crate::tui::ScanMsg;
use cli::Cli;
use config::Config;

fn main() {
    let args = Cli::parse();

    // Disable colour when requested or when stdout is not a TTY
    if args.no_color {
        colored::control::set_override(false);
    }

    // ── Build runtime config ───────────────────────────────────────────────
    let config = Arc::new(Config {
        with_sudo:   args.with_sudo,
        only:        args.only.clone(),
        save_report: !args.no_report,
        no_color:    args.no_color,
    });

    // ── Collect all registered checkers ───────────────────────────────────
    let all = checkers::all_checkers();

    // ── --list mode ────────────────────────────────────────────────────────
    if args.list {
        reporter::print_checker_list(&all);
        return;
    }

    // ── Filter checkers if --only was provided ─────────────────────────────
    let to_run: Vec<Box<dyn checker::Checker>> = if config.only.is_empty() {
        all.into_iter().collect()
    } else {
        all.into_iter()
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

    // Snapshot names and count before consuming to_run into threads.
    let checker_names: Vec<String> = to_run.iter().map(|c| c.name().to_string()).collect();
    let total = to_run.len();

    // ── Report file path ───────────────────────────────────────────────────
    let report_path: Option<PathBuf> = if config.save_report {
        let ts   = Local::now().format("%Y%m%d_%H%M%S");
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        Some(PathBuf::from(format!("{home}/Desktop/alchemy_health_report_{ts}.txt")))
    } else {
        None
    };

    // ── Spawn all checkers in parallel ────────────────────────────────────
    let (tx, rx) = mpsc::channel::<ScanMsg>();

    for (i, checker) in to_run.into_iter().enumerate() {
        let tx_clone = tx.clone();
        let cfg      = Arc::clone(&config);
        thread::spawn(move || {
            let _ = tx_clone.send(ScanMsg::Started(i));
            let result = checker.run(&cfg);
            let _ = tx_clone.send(ScanMsg::Done(i, result));
        });
    }
    // Drop the original sender so the channel closes when all threads finish.
    drop(tx);

    // ── Run mode: interactive TUI or plain-text stdout ────────────────────
    let is_tty      = std::io::stdout().is_terminal();
    let interactive = is_tty && !args.no_interactive;

    if interactive {
        // TUI takes the receiver and drives the live display itself.
        let report_str = report_path.as_ref().map(|p| p.to_string_lossy().into_owned());

        match tui::run(checker_names, rx, report_str.as_deref()) {
            Ok(output) => {
                let health = scorer::HealthScore::compute(&output.results);

                // Save report file (quietly — user already saw everything in the TUI).
                if let Some(path) = &report_path {
                    match reporter::save_report(&output.results, &health, path) {
                        Ok(()) => println!(
                            "\n  {} Report saved to: {}\n",
                            "📄".green(),
                            path.display().to_string().bold()
                        ),
                        Err(e) => eprintln!("  Warning: could not save report file: {e}"),
                    }
                }

                // Execute quick-win actions the user selected in the TUI.
                if !output.pending.is_empty() {
                    actions::execute_selected(&output.pending);
                }
            }
            Err(e) => eprintln!("TUI error: {e}"),
        }
    } else {
        // ── Non-interactive: spinner + full stdout report ──────────────────
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

        // Accumulate results in original index order (threads finish out of order).
        let mut results: Vec<Option<checker::CheckResult>> = (0..total).map(|_| None).collect();
        let mut done_count = 0usize;

        for msg in rx {
            match msg {
                ScanMsg::Started(i) => {
                    if is_tty {
                        pb.set_message(format!(
                            "[{}/{}]  {}...",
                            done_count + 1, total, checker_names[i]
                        ));
                    }
                }
                ScanMsg::Done(i, result) => {
                    let icon = section_icon(&result);
                    if is_tty {
                        pb.println(format!(
                            "  {}  {} {}",
                            icon,
                            checker_names[i].bold(),
                            format!("({}/{})", done_count + 1, total).dimmed()
                        ));
                    }
                    results[i] = Some(result);
                    done_count += 1;
                }
            }
        }

        pb.finish_and_clear();

        let results: Vec<checker::CheckResult> = results.into_iter().flatten().collect();
        let health = scorer::HealthScore::compute(&results);

        reporter::print_report(&results, &health, report_path.as_ref());
    }
}

// ─── Helper — icon for a completed checker section ───────────────────────────

fn section_icon(result: &checker::CheckResult) -> &'static str {
    use crate::checker::Level;
    let max = result
        .findings
        .iter()
        .map(|f| &f.level)
        .max()
        .unwrap_or(&Level::Ok);

    match max {
        Level::Ok       => "✅",
        Level::Warn     => "⚠️ ",
        Level::Critical => "❌",
    }
}
