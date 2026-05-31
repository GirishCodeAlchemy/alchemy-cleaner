# 🍎 alchemy-cleaner

A fast, read-only macOS system health diagnostic tool written in Rust.  
Run it, get a score, read the fixes — no installation of third-party agents required.

```
╔══════════════════════════════════════════════════════════════════════╗
║     🍎  alchemy-cleaner — macOS System Health Diagnostic           ║
╚══════════════════════════════════════════════════════════════════════╝

  Health Score: 87/100    [██████████████████████████████████░░░░░░]
  Verdict     : ✅ HEALTHY
```

---

## Features

- **Health score** — 0–100 aggregate score with colour-coded verdict
- **9 diagnostic checkers** — CPU, memory, disk, processes, startup items, battery, network, crash logs, and system info
- **Actionable fixes** — every finding includes a concrete solution
- **Erase/reinstall advice** — recommends a macOS reinstall only when there is real evidence of software corruption (kernel panics, S.M.A.R.T. failure), not just RAM pressure
- **Plain-text report** — saved automatically to `~/Desktop/alchemy_health_report_<timestamp>.txt`
- **Safe & read-only** — never modifies system state
- **Extensible** — adding a new checker is one file + one line

---

## Requirements

| Item | Version |
|------|---------|
| macOS | Monterey 12+ (Apple Silicon or Intel) |
| Rust | 1.81+ |

---

## Build

```bash
# Clone
git clone <repo-url>
cd alchemy-cleaner

# Release build (recommended)
make

# Or directly with cargo
cargo build --release
```

The binary lands at `target/release/alchemy-cleaner`.

---

## Usage

```bash
# Full diagnostic (all checkers)
./target/release/alchemy-cleaner

# Run specific checkers only
./target/release/alchemy-cleaner --only memory --only cpu

# List all available checkers
./target/release/alchemy-cleaner --list

# Disable saving the Desktop report
./target/release/alchemy-cleaner --no-report

# Disable colour output (useful for CI / pipes)
./target/release/alchemy-cleaner --no-color

# Skip the interactive fix-runner menu after the report
./target/release/alchemy-cleaner --no-interactive

# Print help
./target/release/alchemy-cleaner --help
```

### Via Make

```bash
make run                 # build + run full diagnostic
make run-no-report       # run without saving a Desktop report
make run-no-interactive  # run without the interactive fix-runner menu
make list                # print available checkers
make help                # print all make targets
```

---

## Checkers

| Name | What it checks |
|------|---------------|
| `system-info` | macOS version, chip type, RAM, uptime, hostname |
| `cpu` | Load averages (1m/5m/15m) and instantaneous CPU usage |
| `memory` | RAM usage, swap pressure, memory compressor activity, vm_stat |
| `disk` | Boot volume usage, S.M.A.R.T. status, largest home directories |
| `processes` | Top processes by CPU and RAM; flags runaway processes |
| `startup` | GUI login items, LaunchAgents/Daemons count, failing services |
| `battery` | Condition, cycle count, maximum capacity |
| `network` | DNS response time, established TCP connections, Wi-Fi signal |
| `logs` | Kernel panics (30 days), app crashes (7 days), system error rate |

---

## Health Score

| Score | Verdict | Action |
|-------|---------|--------|
| 80 – 100 | ✅ Healthy | No action needed |
| 55 – 79 | ⚠️ Needs Attention | Fix the listed issues |
| 30 – 54 | ❌ Degraded | Follow solutions; try Safe Mode + Disk Utility |
| 0 – 29 | 🚨 Critical | Serious issues — see recommendations below |

### Erase & Reinstall Recommendation

`alchemy-cleaner` will only recommend an erase and reinstall when there is concrete evidence of **software corruption**:

- S.M.A.R.T. drive failure detected → back up immediately, do not erase (hardware issue)
- Score < 30 **and** kernel panics recorded → erase & reinstall macOS
- Score < 30 but **no** panics and **no** SMART failure → RAM/capacity exhaustion; erasing will not help

---

## Sample Output

```
  1. SYSTEM INFORMATION
────────────────────────────────────────────────────────────────────────
  macOS Version          26.5 (Build 25F71)
  Processor              Apple M2 (arm64)
  Silicon type           Apple Silicon
  Total RAM              16.0 GB
  CPU Cores              8 physical / 8 logical

  ✅  macOS 26.5 (Sonoma or later) — fully supported.

  2. MEMORY ANALYSIS
────────────────────────────────────────────────────────────────────────
  Swap Used              2048 MB
  Swap-outs (since boot) 1024
  ...

  ✅  Swap usage is within normal range.
  ✅  Swap-out activity since boot is low.

...

  FINAL HEALTH SCORE & VERDICT
────────────────────────────────────────────────────────────────────────
  Health Score: 94/100   [█████████████████████████████████████░░░]
  Verdict     : ✅ HEALTHY
```

---

## Make Targets

```bash
make              # Release build (default)
make dev          # Debug build
make run                 # Build + run full diagnostic
make run-no-report       # Run without saving Desktop report
make run-no-interactive  # Run without the interactive fix-runner menu
make check        # Fast syntax/type check (no binary)
make lint         # Clippy with warnings-as-errors
make fmt          # Auto-format with rustfmt
make fmt-check    # Check formatting (CI-safe)
make test         # Run unit tests
make clean        # Remove build artefacts
make install      # Install binary to ~/.cargo/bin
make uninstall    # Remove installed binary
make list         # Print available checkers
make help         # Print all targets
```

---

## Adding a New Checker

The architecture is designed for easy extension:

1. Create `src/checkers/your_feature.rs`
2. Implement the `Checker` trait:

```rust
use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;

pub struct YourChecker;

impl Checker for YourChecker {
    fn name(&self) -> &'static str { "your-feature" }
    fn description(&self) -> &'static str { "One-line description" }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Your Feature Section");

        // add details (key/value pairs shown in the output)
        result.add_detail("Some Key", "some value".to_string());

        // add findings
        result.add_finding(Finding::ok("Everything looks good."));
        // or:
        result.add_finding(Finding::warn("Something is off.", "How to fix it.", 5));
        result.add_finding(Finding::critical("Serious problem.", "How to fix it.", 20));

        result
    }
}
```

3. Register it in `src/checkers/mod.rs`:

```rust
pub mod your_feature;

pub fn all_checkers() -> Vec<Box<dyn Checker>> {
    vec![
        // ... existing checkers ...
        Box::new(your_feature::YourChecker),
    ]
}
```

That's it — the runner, scorer, reporter, and `--only` filter pick it up automatically.

---

## Project Structure

```
alchemy-cleaner/
├── Cargo.toml
├── Makefile
├── src/
│   ├── main.rs          # Entry point, CLI parsing, orchestration
│   ├── cli.rs           # clap argument definitions
│   ├── config.rs        # Runtime config struct
│   ├── checker.rs       # Checker trait, Finding, CheckResult types
│   ├── scorer.rs        # Health score, verdict, erase recommendation
│   ├── reporter.rs      # Terminal output + report file writer
│   └── checkers/
│       ├── mod.rs       # Checker registry (all_checkers)
│       ├── system_info.rs
│       ├── cpu.rs
│       ├── memory.rs
│       ├── disk.rs
│       ├── processes.rs
│       ├── startup.rs
│       ├── battery.rs
│       ├── network.rs
│       └── logs.rs
└── mac_health_check.sh  # Original bash version of this tool
```

---

## Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | =4.4.18 | CLI argument parsing |
| `colored` | 3.1 | ANSI terminal colours |
| `sysinfo` | 0.36 | Cross-platform system information |
| `chrono` | 0.4 | Report file timestamp |
| `anyhow` | 1.0 | Error handling |

> **Note:** `clap` is pinned to `=4.4.18` for compatibility with Rust ≤ 1.84. Newer clap versions require `edition2024` (rustc 1.85+).

---

## License

MIT
