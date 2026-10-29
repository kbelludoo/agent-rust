# Agent-Rust Sentinel // Autonomous VPS Fleet Watchdog & Telemetry

Ultra-lightweight autonomous sentinel written in Rust 2024 for monitoring, self-healing, and alerting across a fleet of 11 game bots on Oracle Cloud Always-Free tier.

## 🚀 Capabilities
- **Zero-GC & Micro-Footprint:** Consumes less than 10 MB RAM compared to 50MB+ in Go or 1GB in Node/Electron.
- **Stall & Anomaly Sentinel:** Tracks deltas for `Gold`, `EXP`, `Stamina`, and `Kills`. If any bot experiences complete inactivity for 10 minutes (`Δ = 0`), it marks the bot as stalled.
- **Auto-Healing Recovery:** Automatically executes graceful restarts of stalled systemd services (`sudo systemctl restart bot-<id>`).
- **Telegram Incident Channel:** If auto-healing fails or repeats, dispatches an alert with diagnostics and links to the Telegram Bot API using `.secrets.json`.
- **Live Fleet Command Center:** Serves REST API (`/api/fleet`, `/api/health`) and hosts GitHub Pages dashboard.

## 📊 Live Dashboard
- **GitHub Pages:** [https://kbelludoo.github.io/agent-rust/](https://kbelludoo.github.io/agent-rust/)

## 🛠️ Build & Test
```powershell
# Run Unit Tests (Including Stall Detection Simulation)
cargo test --all-features -- --nocapture

# Strict Linting
cargo clippy --all-targets --all-features -- -D warnings
```
