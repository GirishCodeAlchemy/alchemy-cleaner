use clap::Parser;

/// 🍎 alchemy-cleaner — macOS System Health & Performance Diagnostic
///
/// Performs a comprehensive read-only analysis of your Mac and
/// produces a health score with actionable recommendations.
/// Optionally saves a plain-text report to ~/Desktop.
#[derive(Parser, Debug)]
#[command(
    name        = "alchemy-cleaner",
    version     = env!("CARGO_PKG_VERSION"),
    author      = "Wibey AI Coding Assistant",
    about       = "macOS system health & performance diagnostic tool",
    long_about  = None,
)]
pub struct Cli {
    /// Run only the named checker(s) — e.g. --only memory --only cpu
    /// Run `alchemy-cleaner --list` to see available checker names.
    #[arg(long, value_name = "NAME", num_args = 1..)]
    pub only: Vec<String>,

    /// List all available checkers and exit.
    #[arg(long)]
    pub list: bool,

    /// Disable saving a report file to ~/Desktop.
    #[arg(long)]
    pub no_report: bool,

    /// Disable colour output (useful for CI or piped output).
    #[arg(long)]
    pub no_color: bool,

    /// Enable deeper thermal/power diagnostics (requires sudo).
    #[arg(long)]
    pub with_sudo: bool,
}
