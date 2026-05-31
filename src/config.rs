/// Runtime configuration threaded through every checker.
// Fields not yet used by checkers are reserved for future use (thermal, colour override).
#[allow(dead_code)]
///
/// Built once in `main` from CLI args and passed as a shared reference.
#[derive(Debug, Clone)]
pub struct Config {
    /// Whether the user ran the tool with `sudo` / as root.
    pub with_sudo: bool,
    /// Run only the named checkers (empty = run all).
    pub only: Vec<String>,
    /// Write a plain-text report file to the Desktop.
    pub save_report: bool,
    /// Silence colour output (useful for CI / piped output).
    pub no_color: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            with_sudo: false,
            only: Vec::new(),
            save_report: true,
            no_color: false,
        }
    }
}
