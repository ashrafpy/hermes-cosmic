use std::path::PathBuf;
use std::process::{Output, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct GatewayStatus {
    pub is_running: bool,
    pub uptime: Option<String>,
    pub profiles: Vec<ProfileInfo>,
    pub resource_stats: Option<ResourceStats>,
    pub connected_platforms: Vec<PlatformStatus>,
    pub dashboard_running: bool,
}

#[derive(Debug, Clone)]
pub struct ProfileInfo {
    pub name: String,
    pub is_current: bool,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct CpuSample {
    pub pid: u32,
    pub cpu_ns: u64,
    pub sampled_at: Instant,
}

#[derive(Debug, Clone)]
pub struct ResourceStats {
    pub memory: String,
    pub cpu_usage: String,
    pub pid: u32,
    pub cpu_sample: CpuSample,
}

#[derive(Debug, Clone)]
pub struct PlatformStatus {
    pub name: String,
    #[allow(dead_code)]
    pub detail: Option<String>,
    pub connected: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
}

/// Resolve the `hermes` executable, falling back to `~/.local/bin/hermes`
/// when spawned from a minimal graphical session `PATH`.
fn hermes_bin() -> PathBuf {
    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join("hermes");
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let local_bin = PathBuf::from(home).join(".local/bin/hermes");
        if local_bin.is_file() {
            return local_bin;
        }
    }
    PathBuf::from("hermes")
}

pub async fn fetch_status(prev_cpu: Option<(CpuSample, String)>) -> GatewayStatus {
    let is_running = check_active().await;

    let (profiles, resource_stats, connected_platforms, dashboard_running) = if is_running {
        tokio::join!(
            fetch_profiles(),
            fetch_resource_stats(prev_cpu),
            fetch_connected_platforms(),
            check_dashboard_status(),
        )
    } else {
        (Vec::new(), None, Vec::new(), check_dashboard_status().await)
    };

    let pid = resource_stats.as_ref().map(|s| s.pid);
    let uptime = if is_running {
        fetch_uptime(pid).await
    } else {
        None
    };

    GatewayStatus {
        is_running,
        uptime,
        profiles,
        resource_stats,
        connected_platforms,
        dashboard_running,
    }
}

async fn check_active() -> bool {
    let result = tokio::process::Command::new("systemctl")
        .args(["--user", "is-active", "hermes-gateway.service"])
        .output()
        .await;
    match result {
        Ok(Output { stdout, .. }) => String::from_utf8_lossy(&stdout).trim() == "active",
        Err(_) => false,
    }
}

async fn fetch_uptime(pid: Option<u32>) -> Option<String> {
    if let Some(pid) = pid {
        if let Ok(output) = tokio::process::Command::new("ps")
            .args(["-o", "etimes=", "-p", &pid.to_string()])
            .output()
            .await
        {
            if let Ok(secs) = String::from_utf8_lossy(&output.stdout).trim().parse::<u64>() {
                return Some(format_duration_secs(secs));
            }
        }
    }

    // Fallback: parse ActiveEnterTimestamp local date/time portion
    let result = tokio::process::Command::new("systemctl")
        .args([
            "--user",
            "show",
            "hermes-gateway.service",
            "--property=ActiveEnterTimestamp",
        ])
        .output()
        .await
        .ok()?;

    let output = String::from_utf8_lossy(&result.stdout);
    let timestamp_str = output.trim().strip_prefix("ActiveEnterTimestamp=")?;
    if timestamp_str.is_empty() {
        return None;
    }

    // Format example: "Thu 2026-11-19 15:29:55 +06" -> take "2026-11-19 15:29:55"
    let parts: Vec<&str> = timestamp_str.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let dt_str = format!("{} {}", parts[1], parts[2]);
    let started = chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%d %H:%M:%S").ok()?;
    let now = chrono::Local::now().naive_local();
    let total_secs = now.signed_duration_since(started).num_seconds().max(0) as u64;
    Some(format_duration_secs(total_secs))
}

fn format_duration_secs(total_secs: u64) -> String {
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let mins = (total_secs % 3600) / 60;

    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m")
    } else {
        "<1m".to_string()
    }
}

async fn fetch_profiles() -> Vec<ProfileInfo> {
    let result = tokio::process::Command::new(hermes_bin())
        .args(["gateway", "list"])
        .output()
        .await;

    match result {
        Ok(Output { stdout, .. }) => parse_profiles(&String::from_utf8_lossy(&stdout)),
        Err(_) => Vec::new(),
    }
}

fn parse_profiles(output: &str) -> Vec<ProfileInfo> {
    let mut profiles = Vec::new();
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("Gateway") {
            continue;
        }

        let is_current = trimmed.contains("(current)") || trimmed.starts_with('✓');
        let cleaned = trimmed
            .trim_start_matches('✓')
            .trim_start_matches('✗')
            .trim();

        let parts: Vec<&str> = cleaned.splitn(2, '—').collect();
        let name_part = parts.first().map(|s| s.trim()).unwrap_or("");
        let status_part = parts.get(1).map(|s| s.trim()).unwrap_or("");

        let name = name_part.replace("(current)", "").trim().to_string();
        if name.is_empty() {
            continue;
        }

        let pid = if status_part.starts_with("PID") {
            status_part
                .strip_prefix("PID")
                .and_then(|s| s.trim().parse::<u32>().ok())
        } else {
            None
        };

        profiles.push(ProfileInfo {
            name,
            is_current,
            pid,
        });
    }
    profiles
}

pub async fn service_action(action: ServiceAction) -> Result<(), String> {
    let cmd = match action {
        ServiceAction::Start => "start",
        ServiceAction::Stop => "stop",
        ServiceAction::Restart => "restart",
    };

    let result = tokio::process::Command::new("systemctl")
        .args(["--user", cmd, "hermes-gateway.service"])
        .output()
        .await
        .map_err(|e| format!("Failed to execute systemctl: {e}"))?;

    if result.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        Err(format!("systemctl {cmd} failed: {}", stderr.trim()))
    }
}

async fn query_systemd_resources() -> Option<(u64, u64, u32, Instant)> {
    let result = tokio::process::Command::new("systemctl")
        .args([
            "--user",
            "show",
            "hermes-gateway.service",
            "--property=MemoryCurrent,CPUUsageNSec,MainPID",
        ])
        .output()
        .await
        .ok()?;

    let now = Instant::now();
    let output = String::from_utf8_lossy(&result.stdout);
    let mut memory_bytes: Option<u64> = None;
    let mut cpu_ns: Option<u64> = None;
    let mut pid: Option<u32> = None;

    for line in output.lines() {
        let line = line.trim();
        if let Some(val) = line.strip_prefix("MemoryCurrent=") {
            memory_bytes = val.trim().parse().ok();
        } else if let Some(val) = line.strip_prefix("CPUUsageNSec=") {
            cpu_ns = val.trim().parse().ok();
        } else if let Some(val) = line.strip_prefix("MainPID=") {
            pid = val.trim().parse().ok();
        }
    }

    let pid = pid.filter(|&p| p > 0)?;
    Some((memory_bytes.unwrap_or(0), cpu_ns.unwrap_or(0), pid, now))
}

async fn fetch_resource_stats(prev_cpu: Option<(CpuSample, String)>) -> Option<ResourceStats> {
    let (memory_bytes, cpu_ns, pid, now) = query_systemd_resources().await?;

    let (cpu_usage, cpu_sample) = match prev_cpu {
        Some((prev, prev_label)) if prev.pid == pid => {
            let elapsed = now.duration_since(prev.sampled_at);
            if elapsed >= Duration::from_millis(200) {
                let pct = compute_cpu_percent(prev.cpu_ns, cpu_ns, elapsed);
                (
                    format!("{pct:.1}%"),
                    CpuSample {
                        pid,
                        cpu_ns,
                        sampled_at: now,
                    },
                )
            } else {
                (prev_label, prev)
            }
        }
        _ => {
            // Take a short baseline window so first popup render shows live CPU %
            tokio::time::sleep(Duration::from_millis(250)).await;
            if let Some((_, cpu_ns2, pid2, now2)) = query_systemd_resources().await {
                if pid2 == pid {
                    let elapsed = now2.duration_since(now);
                    let pct = compute_cpu_percent(cpu_ns, cpu_ns2, elapsed);
                    (
                        format!("{pct:.1}%"),
                        CpuSample {
                            pid,
                            cpu_ns: cpu_ns2,
                            sampled_at: now2,
                        },
                    )
                } else {
                    (
                        "0.0%".to_string(),
                        CpuSample {
                            pid: pid2,
                            cpu_ns: cpu_ns2,
                            sampled_at: now2,
                        },
                    )
                }
            } else {
                (
                    "0.0%".to_string(),
                    CpuSample {
                        pid,
                        cpu_ns,
                        sampled_at: now,
                    },
                )
            }
        }
    };

    Some(ResourceStats {
        memory: format_bytes(memory_bytes),
        cpu_usage,
        pid,
        cpu_sample,
    })
}

fn compute_cpu_percent(prev_ns: u64, curr_ns: u64, elapsed: Duration) -> f64 {
    let elapsed_ns = elapsed.as_nanos();
    if elapsed_ns == 0 {
        return 0.0;
    }
    let delta_ns = curr_ns.saturating_sub(prev_ns) as f64;
    (delta_ns / elapsed_ns as f64) * 100.0
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;

    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

async fn fetch_connected_platforms() -> Vec<PlatformStatus> {
    let result = tokio::process::Command::new(hermes_bin())
        .args(["logs", "gateway", "-n", "200"])
        .output()
        .await;

    match result {
        Ok(Output { stdout, stderr, .. }) => {
            let output = String::from_utf8_lossy(&stdout);
            let err_output = String::from_utf8_lossy(&stderr);
            let combined = format!("{output}\n{err_output}");
            parse_connected_platforms(&combined)
        }
        Err(_) => Vec::new(),
    }
}

fn parse_connected_platforms(output: &str) -> Vec<PlatformStatus> {
    let known = [
        "Telegram",
        "Discord",
        "WhatsApp",
        "Weixin",
        "Signal",
        "Matrix",
        "Slack",
        "IRC",
        "Feishu",
        "DingTalk",
    ];

    // Only inspect lines from the latest gateway startup onward if present
    let lines: Vec<&str> = output.lines().collect();
    let start_idx = lines
        .iter()
        .rposition(|l| l.contains("Hermes Gateway Starting") || l.contains("Connecting to "))
        .map(|idx| {
            // Walk back slightly if "Connecting to" was after startup banner
            lines[..=idx]
                .iter()
                .rposition(|l| l.contains("Hermes Gateway Starting"))
                .unwrap_or(idx)
        })
        .unwrap_or(0);

    let relevant = &lines[start_idx..];
    let mut platforms = Vec::new();

    for name in &known {
        let bracket_name = format!("[{name}]");
        let lower_name = name.to_lowercase();
        let connected_marker = format!("✓ {lower_name} connected");
        let mut found = false;
        let mut connected = false;
        let mut detail: Option<String> = None;

        for line in relevant {
            let lower = line.to_lowercase();
            let mentions_platform = line.contains(&bracket_name)
                || lower.contains(&connected_marker)
                || lower.contains(&format!("connecting to {lower_name}"));

            if !mentions_platform {
                continue;
            }
            found = true;

            if lower.contains(&connected_marker)
                || (lower.contains("connected") && !lower.contains("disconnected"))
            {
                connected = true;
                if lower.contains("polling mode") {
                    detail = Some("polling".to_string());
                } else if lower.contains("webhook") {
                    detail = Some("webhook".to_string());
                }
            } else if lower.contains("disconnected")
                || lower.contains("connection failed")
                || lower.contains("failed to connect")
            {
                connected = false;
                detail = Some("disconnected".to_string());
            }
        }

        if found {
            platforms.push(PlatformStatus {
                name: (*name).to_string(),
                detail,
                connected,
            });
        }
    }

    platforms
}

async fn check_dashboard_status() -> bool {
    let result = tokio::process::Command::new(hermes_bin())
        .args(["dashboard", "--status"])
        .output()
        .await;

    match result {
        Ok(output) => {
            let combined = format!(
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let lower = combined.to_lowercase();
            output.status.success()
                && !lower.contains("no hermes dashboard")
                && (lower.contains("running") || lower.contains("pid"))
        }
        Err(_) => false,
    }
}

pub async fn open_dashboard() -> Result<(), String> {
    if check_dashboard_status().await {
        tokio::process::Command::new("xdg-open")
            .arg("http://127.0.0.1:9119")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to open browser: {e}"))?;
        return Ok(());
    }

    tokio::process::Command::new(hermes_bin())
        .args(["dashboard"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Failed to launch dashboard: {e}"))?;
    Ok(())
}

pub async fn open_chat() -> Result<(), String> {
    let hermes = hermes_bin();
    let hermes_str = hermes.to_string_lossy().into_owned();

    // Respect $TERMINAL if set and available
    if let Ok(term) = std::env::var("TERMINAL") {
        let term = term.trim().to_string();
        if !term.is_empty() && command_exists(&term).await {
            return tokio::process::Command::new(&term)
                .args(["-e", &hermes_str, "chat"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("Failed to launch {term}: {e}"));
        }
    }

    // Note: cosmic-term does not support `-e` / executing a command via CLI flags,
    // so check terminals that support launching commands first.
    let candidates: &[(&str, &[&str])] = &[
        ("ghostty", &["-e"]),
        ("kitty", &[]),
        ("foot", &[]),
        ("alacritty", &["-e"]),
        ("wezterm", &["start", "--"]),
        ("gnome-terminal", &["--"]),
        ("konsole", &["-e"]),
        ("xfce4-terminal", &["-e"]),
        ("xterm", &["-e"]),
    ];

    for &(term, prefix_args) in candidates {
        if command_exists(term).await {
            let mut cmd = tokio::process::Command::new(term);
            cmd.args(prefix_args);
            if term == "xfce4-terminal" {
                cmd.arg(format!("{hermes_str} chat"));
            } else {
                cmd.args([&hermes_str, "chat"]);
            }
            return cmd
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("Failed to launch {term}: {e}"));
        }
    }

    Err("No terminal with command-exec support found (tried ghostty, kitty, foot, alacritty, etc.)"
        .to_string())
}

async fn command_exists(name: &str) -> bool {
    tokio::process::Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_profiles() {
        let output = r#"Gateways:
  ✓ default (current)        — PID 155967
    secondary                 — PID 155968
    tertiary                  — stopped
"#;
        let profiles = parse_profiles(output);
        assert_eq!(profiles.len(), 3);
        assert_eq!(profiles[0].name, "default");
        assert!(profiles[0].is_current);
        assert_eq!(profiles[0].pid, Some(155967));
        assert_eq!(profiles[1].name, "secondary");
        assert!(!profiles[1].is_current);
        assert_eq!(profiles[1].pid, Some(155968));
        assert_eq!(profiles[2].name, "tertiary");
        assert!(!profiles[2].is_current);
        assert_eq!(profiles[2].pid, None);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1_048_576), "1.0 MiB");
        assert_eq!(format_bytes(262_291_456), "250.1 MiB");
    }

    #[test]
    fn test_compute_cpu_percent() {
        assert_eq!(compute_cpu_percent(0, 0, Duration::from_secs(5)), 0.0);
        let pct = compute_cpu_percent(1_000_000_000, 1_250_000_000, Duration::from_secs(5));
        assert!((pct - 5.0).abs() < 0.001);
        // Counter reset / lower curr_ns should saturate to 0.0 rather than underflow
        assert_eq!(
            compute_cpu_percent(500, 100, Duration::from_secs(1)),
            0.0
        );
    }

    #[test]
    fn test_parse_connected_platforms() {
        let output = r#"
2026-11-19 15:30:10,840 WARNING hermes_plugins.platforms__telegram.adapter: [Telegram] Connected to Telegram (polling mode)
2026-11-19 15:30:10,855 INFO gateway.run: ✓ telegram connected
2026-11-19 15:30:11,000 INFO [Discord] Connected
2026-11-19 15:30:12,000 ERROR [WhatsApp] Connection failed
"#;
        let platforms = parse_connected_platforms(output);
        assert_eq!(platforms.len(), 3);
        assert_eq!(platforms[0].name, "Telegram");
        assert!(platforms[0].connected);
        assert_eq!(platforms[0].detail.as_deref(), Some("polling"));
        assert_eq!(platforms[1].name, "Discord");
        assert!(platforms[1].connected);
        assert_eq!(platforms[2].name, "WhatsApp");
        assert!(!platforms[2].connected);
    }
}
