use std::time::Instant;

use crate::checker::{CheckResult, Checker, Finding};
use crate::config::Config;
use super::system_info::cmd_output;

pub struct NetworkChecker;

impl Checker for NetworkChecker {
    fn name(&self) -> &'static str {
        "network"
    }

    fn description(&self) -> &'static str {
        "DNS responsiveness, active interfaces, and TCP connection count"
    }

    fn run(&self, _config: &Config) -> CheckResult {
        let mut result = CheckResult::new("Network Status");

        // ── Active interfaces ─────────────────────────────────────────────
        if let Some(ifcfg) = cmd_output("ifconfig", &[]) {
            let interfaces: Vec<_> = ifcfg
                .lines()
                .filter(|l| l.starts_with("en") && !l.contains("flags="))
                .map(|l| l.split(':').next().unwrap_or("").trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            result.add_detail(
                "Active Interfaces",
                if interfaces.is_empty() {
                    "None detected".to_string()
                } else {
                    interfaces.join(", ")
                },
            );
        }

        // ── DNS responsiveness via dig ─────────────────────────────────────
        let start = Instant::now();
        let dns_out = cmd_output("dig", &["google.com", "+time=3", "+tries=1"]);
        let elapsed_ms = start.elapsed().as_millis();

        if let Some(out) = dns_out {
            let query_time: Option<u64> = out
                .lines()
                .find(|l| l.contains("Query time:"))
                .and_then(|l| l.split_whitespace().nth(3))
                .and_then(|s| s.parse().ok());

            let dns_ms = query_time.unwrap_or(elapsed_ms as u64);
            result.add_detail("DNS Query Time", format!("{dns_ms} ms (google.com)"));

            if dns_ms > 500 {
                result.add_finding(Finding::warn(
                    format!("DNS response is slow ({dns_ms} ms). Web browsing will feel sluggish."),
                    "Switch to a faster DNS resolver:\n\
                     → System Settings → Network → [connection] → DNS\n\
                     → Add 1.1.1.1 (Cloudflare) or 8.8.8.8 (Google)\n\
                     → Remove the existing slow entries and reconnect Wi-Fi.",
                    5,
                ));
            } else if dns_ms > 200 {
                result.add_finding(Finding::warn(
                    format!("DNS response is moderately slow ({dns_ms} ms)."),
                    "Consider switching DNS to 1.1.1.1 (Cloudflare) for faster resolution.",
                    2,
                ));
            } else {
                result.add_finding(Finding::ok(format!("DNS response is fast ({dns_ms} ms).")));
            }
        } else {
            result.add_detail("DNS Test", "Skipped (dig not available or offline)".to_string());
        }

        // ── TCP established connections ────────────────────────────────────
        let conn_count = cmd_output("sh", &["-c", "netstat -an 2>/dev/null | grep -c ESTABLISHED"])
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0);

        result.add_detail("Established TCP Connections", conn_count.to_string());

        if conn_count > 200 {
            result.add_finding(Finding::warn(
                format!("Unusually high TCP connection count: {conn_count}."),
                "Run: netstat -an | grep ESTABLISHED | awk '{print $5}' | cut -d. -f1-4 | sort | uniq -c | sort -rn | head\n\
                 to see which remote hosts have the most connections.",
                5,
            ));
        } else {
            result.add_finding(Finding::ok(format!("TCP connection count is normal ({conn_count}).")));
        }

        // ── Wi-Fi signal quality ──────────────────────────────────────────
        let airport_path = "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport";
        if std::path::Path::new(airport_path).exists() {
            if let Some(wifi_info) = cmd_output(airport_path, &["-I"]) {
                let rssi: Option<i32> = wifi_info
                    .lines()
                    .find(|l| l.trim_start().starts_with("agrCtlRSSI:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|s| s.trim().parse().ok());

                let ssid = wifi_info
                    .lines()
                    .find(|l| l.trim_start().starts_with("SSID:"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(str::trim)
                    .unwrap_or("?");

                if let Some(rssi_val) = rssi {
                    result.add_detail("Wi-Fi SSID",   ssid.to_string());
                    result.add_detail("Wi-Fi Signal",  format!("{rssi_val} dBm"));

                    if rssi_val < -80 {
                        result.add_finding(Finding::warn(
                            format!("Wi-Fi signal is weak ({rssi_val} dBm). Network will feel unreliable."),
                            "Move closer to your router. Consider a Wi-Fi extender or mesh network.",
                            5,
                        ));
                    } else if rssi_val < -70 {
                        result.add_finding(Finding::warn(
                            format!("Wi-Fi signal is moderate ({rssi_val} dBm). Some packet loss possible."),
                            "Move the Mac closer to the router for better throughput.",
                            2,
                        ));
                    } else {
                        result.add_finding(Finding::ok(
                            format!("Wi-Fi signal strength is good ({rssi_val} dBm).")
                        ));
                    }
                }
            }
        }

        result
    }
}
