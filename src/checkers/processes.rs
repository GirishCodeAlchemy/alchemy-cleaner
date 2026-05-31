use sysinfo::{ProcessesToUpdate, RefreshKind, System};

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;

pub struct ProcessChecker;

impl Checker for ProcessChecker {
    fn name(&self) -> &'static str {
        "processes"
    }

    fn description(&self) -> &'static str {
        "Detect runaway processes consuming excess CPU or RAM"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Process Analysis");

        let mut sys = System::new_with_specifics(
            RefreshKind::everything(),
        );
        // Two-pass needed for accurate CPU %
        std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        sys.refresh_processes(ProcessesToUpdate::All, true);

        let total_processes = sys.processes().len();
        result.add_detail("Total Processes", total_processes.to_string());

        // ── Top 5 by CPU ──────────────────────────────────────────────────
        let mut by_cpu: Vec<_> = sys
            .processes()
            .values()
            .map(|p| (p.name().to_string_lossy().to_string(), p.cpu_usage(), p.memory() / 1_048_576))
            .collect();
        by_cpu.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        result.add_detail("── Top by CPU ──", String::new());
        for (name, cpu, mem_mb) in by_cpu.iter().take(5) {
            result.add_detail(
                format!("  {name}"),
                format!("CPU {cpu:.1}%  RAM {mem_mb} MB"),
            );
        }

        // ── Top 5 by Memory ───────────────────────────────────────────────
        let mut by_mem: Vec<_> = by_cpu.clone();
        by_mem.sort_by(|a, b| b.2.cmp(&a.2));

        result.add_detail("── Top by Memory ──", String::new());
        for (name, cpu, mem_mb) in by_mem.iter().take(5) {
            result.add_detail(
                format!("  {name}"),
                format!("RAM {mem_mb} MB  CPU {cpu:.1}%"),
            );
        }

        // ── Runaway process detection ─────────────────────────────────────
        let runaway_cpu: Vec<_> = by_cpu.iter()
            .filter(|(_, cpu, _)| *cpu > 80.0)
            .collect();

        let runaway_mem: Vec<_> = by_mem.iter()
            .filter(|(_, _, mb)| *mb > 2048)
            .collect();

        if runaway_cpu.is_empty() && runaway_mem.is_empty() {
            result.add_finding(Finding::ok(
                "No runaway processes detected. All processes are within normal ranges.".to_string()
            ));
        }

        for (name, cpu, _) in &runaway_cpu {
            result.add_finding(Finding::warn(
                format!("'{name}' is consuming {cpu:.1}% CPU — potential runaway process."),
                format!(
                    "Check Activity Monitor → search for '{name}'.\n\
                     If you don't recognise this process, research it online.\n\
                     Force-quit only if certain it is not a critical system process."
                ),
                8,
            ));
        }

        for (name, _, mb) in &runaway_mem {
            result.add_finding(Finding::warn(
                format!("'{name}' is using {mb} MB of RAM — memory-heavy process."),
                format!(
                    "'{name}' may have a memory leak. Restart the application.\n\
                     If it's a browser, close unused tabs or restart the browser."
                ),
                5,
            ));
        }

        result
    }
}
