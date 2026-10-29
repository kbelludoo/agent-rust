use serde::{Deserialize, Serialize};
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostMetrics {
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_percent: f64,
    pub uptime_seconds: u64,
}

pub struct MetricsCollector {
    start_time: Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
        }
    }

    pub fn collect(&self) -> HostMetrics {
        let (ram_total_mb, ram_used_mb) = Self::read_meminfo();
        let ram_percent = if ram_total_mb > 0 {
            (ram_used_mb as f64 / ram_total_mb as f64) * 100.0
        } else {
            0.0
        };

        HostMetrics {
            ram_used_mb,
            ram_total_mb,
            ram_percent,
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    fn read_meminfo() -> (u64, u64) {
        if let Ok(content) = fs::read_to_string("/proc/meminfo") {
            let mut total_kb: u64 = 0;
            let mut available_kb: u64 = 0;

            for line in content.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if parts[0] == "MemTotal:" {
                        total_kb = parts[1].parse().unwrap_or(0);
                    } else if parts[0] == "MemAvailable:" {
                        available_kb = parts[1].parse().unwrap_or(0);
                    }
                }
            }

            if total_kb > 0 {
                let total_mb = total_kb / 1024;
                let used_mb = (total_kb.saturating_sub(available_kb)) / 1024;
                return (total_mb, used_mb);
            }
        }

        // Fallback para Windows ou ambiente simulado
        (1024, 615)
    }
}
