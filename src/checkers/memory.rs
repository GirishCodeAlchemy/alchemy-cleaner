use sysinfo::{MemoryRefreshKind, RefreshKind, System};

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::cmd_output;

pub struct MemoryChecker;

impl Checker for MemoryChecker {
    fn name(&self) -> &'static str {
        "memory"
    }

    fn description(&self) -> &'static str {
        "RAM usage, swap pressure, and memory compressor activity"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Memory Analysis");

        // ── sysinfo totals ─────────────────────────────────────────────────
        // sysinfo 0.36: use ::nothing() instead of ::new()
        let mut sys = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::nothing().with_ram().with_swap()),
        );
        sys.refresh_memory();

        let total_mb  = sys.total_memory()     / 1_048_576;
        let used_mb   = sys.used_memory()      / 1_048_576;
        let avail_mb  = sys.available_memory() / 1_048_576;
        let swap_total_mb = sys.total_swap()   / 1_048_576;
        let swap_used_mb  = sys.used_swap()    / 1_048_576;

        result.add_detail("Total RAM",      format!("{total_mb} MB"));
        result.add_detail("Used RAM",       format!("{used_mb} MB"));
        result.add_detail("Available RAM",  format!("{avail_mb} MB"));
        result.add_detail("Swap Total",     format!("{swap_total_mb} MB"));
        result.add_detail("Swap Used",      format!("{swap_used_mb} MB"));

        // ── vm_stat for page-level detail ──────────────────────────────────
        let vm = parse_vm_stat();
        if let Some(ref v) = vm {
            result.add_detail(
                "In Compressor (RAM)",
                format!("{} MB  ← compressed pages in RAM", v.compressed_mb),
            );
            result.add_detail(
                "Swap-outs (since boot)",
                v.swapouts.to_string(),
            );
            result.add_detail(
                "Inactive/Cache",
                format!("{} MB  ← healthy, macOS file cache", v.inactive_mb),
            );
        }

        // ── Swap usage assessment ──────────────────────────────────────────
        let swap_pct = if swap_total_mb > 0 {
            (swap_used_mb as f64 / swap_total_mb as f64) * 100.0
        } else {
            0.0
        };

        if swap_used_mb > 0 && swap_pct > 80.0 {
            result.add_finding(Finding::critical(
                format!("Swap usage is critically high: {swap_used_mb} MB / {swap_total_mb} MB ({swap_pct:.0}%)."),
                "Close memory-intensive apps (browser tabs, Slack, Teams, Docker).\n\
                 If this occurs regularly, the Mac needs more RAM.\n\
                 On Apple Silicon RAM cannot be upgraded post-purchase.",
                25,
            ));
        } else if swap_used_mb > 0 && swap_pct > 40.0 {
            result.add_finding(Finding::warn(
                format!("Elevated swap usage: {swap_used_mb} MB used ({swap_pct:.0}% of swap)."),
                "Monitor RAM consumption in Activity Monitor → Memory tab.\n\
                 Browser tabs are common culprits — limit open tabs or use Safari.",
                10,
            ));
        } else if swap_used_mb > 100 {
            result.add_finding(Finding::warn(
                format!("Swap is active ({swap_used_mb} MB) — system has been paging."),
                "Normal after heavy workloads; restart the Mac to clear if performance feels slow.",
                4,
            ));
        } else {
            result.add_finding(Finding::ok(format!(
                "Swap usage is minimal ({swap_used_mb} MB). RAM pressure is low."
            )));
        }

        // ── vm_stat swap-out history ───────────────────────────────────────
        if let Some(ref v) = vm {
            if v.swapouts > 500_000 {
                result.add_finding(Finding::critical(
                    format!("Very high swap-out activity since boot: {} — system has been paging heavily to SSD.", v.swapouts),
                    "A restart will clear the counter. If swapouts climb quickly after a fresh\n\
                     boot, close unused apps or upgrade to a Mac with more RAM.",
                    20,
                ));
            } else if v.swapouts > 100_000 {
                result.add_finding(Finding::warn(
                    format!("Elevated swap-out activity since boot: {}.", v.swapouts),
                    "Monitor which apps consume the most RAM in Activity Monitor → Memory tab.",
                    8,
                ));
            } else {
                result.add_finding(Finding::ok(format!(
                    "Swap-out activity is low ({} swap-outs since boot).", v.swapouts
                )));
            }

            // ── Compressor pressure ────────────────────────────────────────
            if v.compressed_mb > 3000 {
                result.add_finding(Finding::warn(
                    format!("Compressor is holding {} MB — RAM is under sustained pressure.", v.compressed_mb),
                    "macOS compresses memory to delay swapping. If consistent, you're near\n\
                     the RAM ceiling. Quit apps you don't use or restart to clear compression.",
                    10,
                ));
            } else {
                result.add_finding(Finding::ok(format!(
                    "Memory compression is low ({} MB). RAM is comfortable.", v.compressed_mb
                )));
            }
        }

        // ── system memory_pressure tool ────────────────────────────────────
        if let Some(output) = cmd_output("memory_pressure", &[]) {
            let pressure_line = output
                .lines()
                .find(|l| l.contains("System-wide memory free percentage"))
                .unwrap_or("unavailable");
            result.add_detail("memory_pressure", pressure_line.trim().to_string());

            if output.contains("Critical") {
                result.add_finding(Finding::critical(
                    "System reports CRITICAL memory pressure.".to_string(),
                    "Immediately close memory-heavy apps. Restart if sluggish.\n\
                     Persistent critical pressure means you need more RAM.",
                    20,
                ));
            } else if output.contains("Warn") {
                result.add_finding(Finding::warn(
                    "System reports WARNING memory pressure.".to_string(),
                    "Close browser tabs, quit apps running in background (Slack, Teams, Spotify).",
                    8,
                ));
            } else {
                result.add_finding(Finding::ok("Memory pressure is at a normal level.".to_string()));
            }
        }

        // ── Top memory processes ───────────────────────────────────────────
        result.add_detail("Top Memory Processes", String::new());
        let mut mem_sys = System::new_with_specifics(
            RefreshKind::nothing().with_memory(MemoryRefreshKind::everything()),
        );
        mem_sys.refresh_all();

        let mut procs: Vec<_> = mem_sys
            .processes()
            .values()
            .map(|p| (p.name().to_string_lossy().to_string(), p.memory() / 1_048_576))
            .collect();
        procs.sort_by(|a, b| b.1.cmp(&a.1));

        for (name, mb) in procs.iter().take(5) {
            result.add_detail(format!("  {name}"), format!("{mb} MB"));
        }

        result
    }
}

// ─── vm_stat parser ───────────────────────────────────────────────────────────

struct VmStat {
    inactive_mb: u64,
    compressed_mb: u64,
    swapouts: u64,
}

fn parse_vm_stat() -> Option<VmStat> {
    let out = cmd_output("vm_stat", &[])?;

    // Page size from header
    let page_size: u64 = out
        .lines()
        .next()
        .and_then(|l| {
            l.split_whitespace()
                .find(|t| t.parse::<u64>().is_ok())
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(4096);

    let parse_pages = |key: &str| -> u64 {
        out.lines()
            .find(|l| l.contains(key))
            .and_then(|l| {
                l.split_whitespace()
                    .last()
                    .and_then(|s| s.trim_end_matches('.').parse::<u64>().ok())
            })
            .unwrap_or(0)
    };

    let pages_inactive   = parse_pages("Pages inactive:");
    let pages_compressed = parse_pages("Pages occupied by compressor:");
    let swapouts         = parse_pages("Swapouts:");

    Some(VmStat {
        inactive_mb:   pages_inactive   * page_size / 1_048_576,
        compressed_mb: pages_compressed * page_size / 1_048_576,
        swapouts,
    })
}
