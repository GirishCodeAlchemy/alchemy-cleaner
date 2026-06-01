use colored::Colorize;
use std::fs;
use std::io::Write as IoWrite;
use std::path::PathBuf;

use crate::actions::QUICK_WIN_COMMANDS;
use crate::checker::{CheckResult, Level};
use crate::scorer::{EraseRecommendation, HealthScore, Verdict};

// ─── Terminal width constant ──────────────────────────────────────────────────
const WIDTH: usize = 72;

// ─── Public entry point ───────────────────────────────────────────────────────

/// Print the full diagnostic report to stdout and optionally to a report file.
pub fn print_report(
    results: &[CheckResult],
    health: &HealthScore,
    save_path: Option<&PathBuf>,
) {
    let mut plain_lines: Vec<String> = Vec::new(); // accumulated for file report

    // ── Banner ──────────────────────────────────────────────────────────────
    let banner = format!(
        "\n{}\n{}\n{}\n",
        "╔══════════════════════════════════════════════════════════════════════╗".cyan().bold(),
        "║     🍎  alchemy-cleaner — macOS System Health Diagnostic           ║".cyan().bold(),
        "╚══════════════════════════════════════════════════════════════════════╝".cyan().bold(),
    );
    println!("{banner}");
    plain_lines.push(
        "═══════════════════ alchemy-cleaner — macOS System Health Diagnostic ═══════════════════".into(),
    );

    // ── Note about cached memory ────────────────────────────────────────────
    let note = format!(
        "  {} Inactive/cached memory is {}. macOS reclaims it instantly.\n  {} True pressure = swap-out activity + memory compressor load.\n",
        "ℹ️ ".yellow(),
        "healthy by design".green().bold(),
        "ℹ️ ".yellow()
    );
    println!("{note}");

    // ── Each checker section ─────────────────────────────────────────────────
    for (i, result) in results.iter().enumerate() {
        print_section(i + 1, result, &mut plain_lines);
    }

    // ── Final score & verdict ─────────────────────────────────────────────────
    print_score(health, &mut plain_lines);

    // ── Issues + solutions summary ────────────────────────────────────────────
    print_solutions(results, &mut plain_lines);

    // ── Quick win commands ────────────────────────────────────────────────────
    print_quick_wins(&mut plain_lines);

    // ── Save report file ──────────────────────────────────────────────────────
    if let Some(path) = save_path {
        match save_report_file(path, &plain_lines) {
            Ok(()) => println!(
                "\n  {} Report saved to: {}\n",
                "📄".green(),
                path.display().to_string().bold()
            ),
            Err(e) => eprintln!("  Warning: could not save report file: {e}"),
        }
    }
}

// ─── Section printer ─────────────────────────────────────────────────────────

fn print_section(num: usize, result: &CheckResult, plain: &mut Vec<String>) {
    let header = format!("{num}. {}", result.section.to_uppercase());
    println!(
        "\n{}",
        format!("  {header}  ").on_blue().white().bold()
    );
    println!("{}", "─".repeat(WIDTH).blue());
    plain.push(format!("\n{}", "═".repeat(WIDTH)));
    plain.push(format!("  {header}"));
    plain.push("─".repeat(WIDTH));

    // Details block
    for (k, v) in &result.details {
        let line = format!("  {:<24} {}", k, v);
        println!("  {}  {}", k.dimmed(), v.white());
        plain.push(line);
    }

    // Findings
    if !result.details.is_empty() && !result.findings.is_empty() {
        println!();
    }
    for finding in &result.findings {
        let (icon, coloured) = match finding.level {
            Level::Ok => ("✅", finding.message.green().to_string()),
            Level::Warn => ("⚠️ ", finding.message.yellow().to_string()),
            Level::Critical => ("❌", finding.message.red().bold().to_string()),
        };
        println!("  {icon}  {coloured}");
        plain.push(format!("  [{icon}] {}", finding.message));
    }
}

// ─── Score block ─────────────────────────────────────────────────────────────

fn print_score(health: &HealthScore, plain: &mut Vec<String>) {
    println!("\n\n{}", "─".repeat(WIDTH).dimmed());
    println!("{}", "  FINAL HEALTH SCORE & VERDICT  ".on_blue().white().bold());
    println!("{}\n", "─".repeat(WIDTH).dimmed());

    plain.push("\n".into());
    plain.push("═══ FINAL HEALTH SCORE & VERDICT ═══".into());

    let bar = health.bar(40);
    let score_str = format!("{}/100", health.score);
    let bar_display = format!("  Health Score: {score_str:<8}  [{bar}]");

    let coloured_bar = match health.verdict {
        Verdict::Healthy => bar_display.green().bold().to_string(),
        Verdict::NeedsAttention => bar_display.yellow().bold().to_string(),
        Verdict::Degraded | Verdict::Critical => bar_display.red().bold().to_string(),
    };
    println!("{coloured_bar}");
    plain.push(format!("  Health Score: {score_str}  [{bar}]"));

    let verdict_line = format!(
        "  Verdict     : {} {}",
        health.verdict.icon(),
        health.verdict.label()
    );
    let coloured_verdict = match health.verdict {
        Verdict::Healthy => verdict_line.green().bold().to_string(),
        Verdict::NeedsAttention => verdict_line.yellow().bold().to_string(),
        Verdict::Degraded | Verdict::Critical => verdict_line.red().bold().to_string(),
    };
    println!("{coloured_verdict}\n");
    plain.push(verdict_line);

    // Erase recommendation
    println!("  {}", "Should you erase and reinstall macOS?".bold());
    println!(
        "  {} (\"Erase All Content and Settings\" on Apple Silicon / Recovery Mode on Intel)\n",
        "(i.e.".dimmed()
    );
    plain.push("\n  Should you erase and reinstall macOS?".into());
    plain.push("  (\"Erase All Content and Settings\" / Recovery Mode)".into());

    if health.smart_failure {
        let msg = "  ⚠  HARDWARE ISSUE — S.M.A.R.T. failure detected. \
            Back up NOW and book Apple Support.\n  Erasing will NOT fix failing hardware.";
        println!("{}", msg.red().bold());
        plain.push(msg.into());
        return;
    }

    match health.verdict.erase_recommendation(health.panic_count, health.smart_failure) {
        EraseRecommendation::NotNeeded => {
            let msg = format!(
                "  ✅  NOT recommended. Score {}/100 — fix the issues below first.",
                health.score
            );
            println!("{}", msg.green());
            plain.push(msg);
        }
        EraseRecommendation::TryOtherFirst => {
            let msg = format!(
                "  ⚠️   Score {}/100 — action needed, but no evidence of software corruption.",
                health.score
            );
            println!("{}", msg.yellow());
            plain.push(msg);
            let advice = "  If issues are memory/swap related: this is a capacity problem.\n  \
                Close unused apps, quit browser tabs, or restart to free RAM.\n  \
                Erasing macOS will NOT fix RAM exhaustion — only more RAM or fewer apps will.\n  \
                If issues are service/startup related: try Safe Mode (hold Shift at boot)\n  \
                then run Disk Utility → First Aid before considering a reinstall.";
            println!("{}", advice.yellow());
            plain.push(advice.into());
        }
        EraseRecommendation::Recommended => {
            let msg = format!(
                "  🚨  Score {}/100 + {} kernel panic(s) detected → ERASE & REINSTALL recommended.",
                health.score, health.panic_count
            );
            println!("{}", msg.red().bold());
            plain.push(msg);
            plain.push("  Apple Silicon: System Settings → General → Transfer or Reset → Erase All Content.".into());
            plain.push("  Intel: hold ⌘+R at boot → Disk Utility Erase → Reinstall macOS.".into());
        }
    }
}

// ─── Solutions summary ────────────────────────────────────────────────────────

fn print_solutions(results: &[CheckResult], plain: &mut Vec<String>) {
    let actionable: Vec<_> = results
        .iter()
        .flat_map(|r| r.findings.iter())
        .filter(|f| f.solution.is_some() && f.score_deduction > 0)
        .collect();

    if actionable.is_empty() {
        println!("\n  {} No issues found — system is in great shape! 🎉", "✅".green());
        plain.push("\n  No issues found — system is in great shape!".into());
        return;
    }

    println!("\n\n{}", "  ISSUES & SOLUTIONS  ".on_blue().white().bold());
    println!("{}", "─".repeat(WIDTH).blue());
    plain.push("\n═══ ISSUES & SOLUTIONS ═══".into());

    for (i, finding) in actionable.iter().enumerate() {
        let num = i + 1;
        println!(
            "\n  {} {} {}",
            format!("Issue {num}:").red().bold(),
            "→".dimmed(),
            finding.message.yellow()
        );
        plain.push(format!("\n  Issue {num}: → {}", finding.message));

        if let Some(sol) = &finding.solution {
            println!("  {} Fix:", "✅".green());
            for line in sol.lines() {
                println!("    {}", line.white());
            }
            plain.push(format!("  Fix:\n    {}", sol.replace('\n', "\n    ")));
        }
    }
}

// ─── Quick win commands ───────────────────────────────────────────────────────

fn print_quick_wins(plain: &mut Vec<String>) {
    println!("\n\n{}", "─".repeat(WIDTH).dimmed());
    println!("{}", "  ⚡  QUICK WIN COMMANDS (safe to run manually)  ".cyan().bold());
    println!("{}\n", "─".repeat(WIDTH).dimmed());

    plain.push("\n⚡ QUICK WIN COMMANDS".into());

    for qw in QUICK_WIN_COMMANDS {
        println!("  {}  ", format!("# {}", qw.label).dimmed());
        println!("  {}\n", qw.cmd.green());
        plain.push(format!("  # {}", qw.label));
        plain.push(format!("  {}\n", qw.cmd));
    }
}

// ─── Report file writer ───────────────────────────────────────────────────────

fn save_report_file(path: &PathBuf, lines: &[String]) -> anyhow::Result<()> {
    let mut file = fs::File::create(path)?;
    for line in lines {
        writeln!(file, "{line}")?;
    }
    Ok(())
}

// ─── Save-only report (used by interactive TUI mode) ─────────────────────────

/// Write a plain-text report to `path` without printing anything to stdout.
///
/// Used when the interactive TUI already showed all details to the user and
/// we just want a persisted copy on disk.
pub fn save_report(
    results: &[CheckResult],
    health: &HealthScore,
    path: &PathBuf,
) -> anyhow::Result<()> {
    let mut lines: Vec<String> = Vec::new();

    lines.push(
        "═══════════════════ alchemy-cleaner — macOS System Health Diagnostic ═══════════════════"
            .into(),
    );

    // ── Sections ──────────────────────────────────────────────────────────
    for (i, result) in results.iter().enumerate() {
        let header = format!("{}. {}", i + 1, result.section.to_uppercase());
        lines.push(format!("\n{}", "═".repeat(WIDTH)));
        lines.push(format!("  {header}"));
        lines.push("─".repeat(WIDTH));

        for (k, v) in &result.details {
            lines.push(format!("  {:<24} {}", k, v));
        }

        for finding in &result.findings {
            let icon = match finding.level {
                Level::Ok       => "✅",
                Level::Warn     => "⚠️ ",
                Level::Critical => "❌",
            };
            lines.push(format!("  [{icon}] {}", finding.message));
        }
    }

    // ── Score & verdict ────────────────────────────────────────────────────
    lines.push("\n".into());
    lines.push("═══ FINAL HEALTH SCORE & VERDICT ═══".into());
    let score_str = format!("{}/100", health.score);
    let bar = health.bar(40);
    lines.push(format!("  Health Score: {score_str}  [{bar}]"));
    lines.push(format!(
        "  Verdict     : {} {}",
        health.verdict.icon(),
        health.verdict.label()
    ));
    lines.push("\n  Should you erase and reinstall macOS?".into());
    lines.push("  (\"Erase All Content and Settings\" / Recovery Mode)".into());

    if health.smart_failure {
        lines.push(
            "  ⚠  HARDWARE ISSUE — S.M.A.R.T. failure detected. \
             Back up NOW and book Apple Support.\n  Erasing will NOT fix failing hardware."
                .into(),
        );
    } else {
        match health.verdict.erase_recommendation(health.panic_count, health.smart_failure) {
            EraseRecommendation::NotNeeded => {
                lines.push(format!(
                    "  ✅  NOT recommended. Score {}/100 — fix the issues below first.",
                    health.score
                ));
            }
            EraseRecommendation::TryOtherFirst => {
                lines.push(format!(
                    "  ⚠️   Score {}/100 — action needed, but no evidence of software corruption.",
                    health.score
                ));
                lines.push(
                    "  If issues are memory/swap related: this is a capacity problem.\n  \
                     Close unused apps, quit browser tabs, or restart to free RAM.\n  \
                     Erasing macOS will NOT fix RAM exhaustion — only more RAM or fewer apps will."
                        .into(),
                );
            }
            EraseRecommendation::Recommended => {
                lines.push(format!(
                    "  🚨  Score {}/100 + {} kernel panic(s) detected → ERASE & REINSTALL recommended.",
                    health.score, health.panic_count
                ));
                lines.push(
                    "  Apple Silicon: System Settings → General → Transfer or Reset → Erase All Content."
                        .into(),
                );
                lines.push(
                    "  Intel: hold ⌘+R at boot → Disk Utility Erase → Reinstall macOS.".into(),
                );
            }
        }
    }

    // ── Issues & solutions ─────────────────────────────────────────────────
    let actionable: Vec<_> = results
        .iter()
        .flat_map(|r| r.findings.iter())
        .filter(|f| f.solution.is_some() && f.score_deduction > 0)
        .collect();

    if actionable.is_empty() {
        lines.push("\n  No issues found — system is in great shape!".into());
    } else {
        lines.push("\n═══ ISSUES & SOLUTIONS ═══".into());
        for (i, finding) in actionable.iter().enumerate() {
            let num = i + 1;
            lines.push(format!("\n  Issue {num}: → {}", finding.message));
            if let Some(sol) = &finding.solution {
                lines.push(format!("  Fix:\n    {}", sol.replace('\n', "\n    ")));
            }
        }
    }

    // ── Quick wins ─────────────────────────────────────────────────────────
    lines.push("\n⚡ QUICK WIN COMMANDS".into());
    for qw in QUICK_WIN_COMMANDS {
        lines.push(format!("  # {}", qw.label));
        lines.push(format!("  {}\n", qw.cmd));
    }

    save_report_file(path, &lines)
}

// ─── Helper for printing list of available checkers ──────────────────────────

pub fn print_checker_list(checkers: &[Box<dyn crate::checker::Checker>]) {
    println!("\n{}", "  Available Checkers  ".on_blue().white().bold());
    println!("{}", "─".repeat(50).blue());
    for c in checkers {
        println!("  {:<20}  {}", c.name().cyan(), c.description().dimmed());
    }
    println!();
}
