# twatch

A modern, zero-dependency, btop-grade hardware temperature monitoring and interactive graphing TUI built with Rust and Ratatui.

---

## Features

- **Real-Time btop-Style Monitor:** Live scrolling Braille temperature charts with smooth multi-color gradients (Cyan → Green → Amber → Hot Red).
- **Zero External Dependencies:** 100% native Rust. No Python, no Matplotlib, no TkAgg. Runs seamlessly in any terminal.
- **Native Interactive Session Grapher:** Pan, zoom, toggle series, and detect heat spikes directly in your terminal.
- **Multi-Vendor & Sub-Degree Precision:** Accurate 0.1°C resolution across CPU (AMD/Intel), GPU (AMD/NVIDIA/Intel), NVMe SSDs, DDR5 SPD RAM, and Motherboards.
- **XDG Compliance:** Sessions stored cleanly in `$XDG_DATA_HOME/twatch/session` with automatic fallback to legacy directories.
- **Live Statistics:** Per-sensor Min / Max / Average tracking and rate-of-change indicators (°C/s).

---

## Commands

| Command | Description |
|---|---|
| `twatch` | List recorded sessions with detailed metrics (default) |
| `twatch live` | Launch real-time btop-style thermal dashboard without recording |
| `twatch live -r` | Live dashboard + simultaneously record session to CSV |
| `twatch run` | Record a session (250 captures by default, then opens interactive graph) |
| `twatch run -t` | Temperature-triggered recording (stops at `--end` threshold) |
| `twatch run -c N` | Record session limited to N captures |
| `twatch graph [ID...]` | View interactive multi-series terminal chart (latest session if omitted) |
| `twatch list` | Detailed session list with duration, sample count, and peak temperature |
| `twatch delete <ID...>` | Delete recorded session files |
| `twatch temp` | Show current temperatures across all hardware sensors |
| `twatch completions <shell>` | Generate shell completions (`bash`, `zsh`, `fish`) |

---

## Command Options

| Option | Description |
|---|---|
| `-d, --delay <ms>` | Milliseconds between captures (default: 250) |
| `-i, --initial <C>` | Start temperature threshold for `--by-temperature` (default: 40.0) |
| `-e, --end <C>` | Stop temperature threshold for `--by-temperature` (default: 70.0) |
| `--sensor <cpu\|gpu\|nvme>` | Target trigger sensor for `--by-temperature` (default: `cpu`) |
| `--json` | Stream JSON records to stdout instead of launching TUI |
| `--no-graph` | Skip automatically opening graph viewer after recording completes |
| `--max-temp <C>` | Maximum temperature (°C) on the plot Y-axis (default: 110) |
| `--temp-steps <N>` | Grid step interval (°C) on the plot Y-axis (default: 5) |

---

## Interactive Keybindings

### In Live Monitor (`twatch live` / `twatch run`):
- `q` / `Esc`: Quit and save session
- `p` / `Space`: Pause / resume polling
- `c`: Clear live chart history buffer
- `h` / `?`: Toggle help overlay

### In Interactive Graph Viewer (`twatch graph`):
- `←` / `→` or `h` / `l`: Pan / scroll horizontally along the time axis
- `+` / `-`: Zoom in / out along the time axis
- `↑` / `↓` or `k` / `j`: Zoom in / out on the temperature axis
- `1` .. `9`: Toggle visibility of individual sensor series
- `a`: Toggle all sensor series on / off
- `s`: Toggle heat-spike markers (`▼`)
- `m`: Cycle chart markers (`Braille` → `Dot` → `Block`)
- `r`: Reset graph zoom & pan
- `q` / `Esc`: Exit graph viewer

---

## Temperature Gradient Legend

| Temperature | Color | State |
|---|---|---|
| `< 40°C` | Cyan | Idle / Cool |
| `40°C – 55°C` | Green | Normal operating temperature |
| `55°C – 70°C` | Yellow / Amber | Moderate load |
| `70°C – 85°C` | Orange | High load / Warm |
| `> 85°C` | Bold Red | Thermal stress / Throttle danger |
