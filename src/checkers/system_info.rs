use std::process::Command;

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;

pub struct SystemInfoChecker;

impl Checker for SystemInfoChecker {
    fn name(&self) -> &'static str {
        "system-info"
    }

    fn description(&self) -> &'static str {
        "macOS version, chip, RAM, uptime"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("System Information");

        // ── macOS version (sw_vers) ──────────────────────────────────────────
        let macos_ver = cmd_output("sw_vers", &["-productVersion"])
            .unwrap_or_else(|| "Unknown".to_string());
        let build = cmd_output("sw_vers", &["-buildVersion"])
            .unwrap_or_else(|| "Unknown".to_string());
        result.add_detail("macOS Version", format!("{macos_ver} (Build {build})"));

        // ── Architecture & chip ──────────────────────────────────────────────
        let arch = cmd_output("uname", &["-m"]).unwrap_or_else(|| "unknown".to_string());
        let chip = sysctl_str("machdep.cpu.brand_string")
            .unwrap_or_else(|| "Unknown CPU".to_string());
        result.add_detail("Processor", format!("{chip} ({arch})"));
        let chip_kind = if arch.trim() == "arm64" { "Apple Silicon" } else { "Intel x86_64" };
        result.add_detail("Silicon type", chip_kind);

        // ── RAM ──────────────────────────────────────────────────────────────
        let ram_bytes = sysctl_u64("hw.memsize").unwrap_or(0);
        let ram_gb = ram_bytes as f64 / 1_073_741_824.0;
        result.add_detail("Total RAM", format!("{ram_gb:.1} GB"));

        // ── CPU cores ────────────────────────────────────────────────────────
        let pcores = sysctl_str("hw.physicalcpu").unwrap_or_else(|| "?".to_string());
        let lcores = sysctl_str("hw.logicalcpu").unwrap_or_else(|| "?".to_string());
        result.add_detail("CPU Cores", format!("{pcores} physical / {lcores} logical"));

        // ── Uptime ────────────────────────────────────────────────────────────
        let uptime = cmd_output("uptime", &[])
            .map(|s| {
                // Extract the "up X days/hours" part
                s.split("up ").nth(1)
                    .and_then(|p| p.split(", load").next())
                    .map(str::trim)
                    .unwrap_or("unknown")
                    .to_string()
            })
            .unwrap_or_else(|| "unknown".to_string());
        result.add_detail("Uptime", uptime);

        // ── Hostname ──────────────────────────────────────────────────────────
        let hostname = cmd_output("scutil", &["--get", "ComputerName"])
            .or_else(|| cmd_output("hostname", &[]))
            .unwrap_or_else(|| "unknown".to_string());
        result.add_detail("Hostname", hostname);

        // ── Version freshness check ───────────────────────────────────────────
        let major: u32 = macos_ver.split('.').next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if major >= 14 {
            result.add_finding(Finding::ok(
                format!("macOS {macos_ver} (Sonoma or later) — fully supported.")
            ));
        } else if major == 13 {
            result.add_finding(Finding::ok(
                format!("macOS {macos_ver} (Ventura) — supported, consider upgrading to Sonoma.")
            ));
        } else if major >= 12 {
            result.add_finding(Finding::warn(
                format!("macOS {macos_ver} is Monterey or older — security updates may have ended."),
                "Update: System Settings → General → Software Update.",
                5,
            ));
        } else {
            result.add_finding(Finding::critical(
                format!("macOS {macos_ver} is significantly outdated — performance optimisations and security patches are unavailable."),
                "Update macOS via System Settings → General → Software Update immediately.",
                10,
            ));
        }

        result
    }
}

// ─── macOS helper utilities ────────────────────────────────────────────────────

pub fn cmd_output(prog: &str, args: &[&str]) -> Option<String> {
    Command::new(prog)
        .args(args)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

pub fn sysctl_str(key: &str) -> Option<String> {
    cmd_output("sysctl", &["-n", key])
}

pub fn sysctl_u64(key: &str) -> Option<u64> {
    sysctl_str(key).and_then(|s| s.trim().parse().ok())
}
