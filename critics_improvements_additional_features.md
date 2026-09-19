# Twatch — Critics, Improvements & Additional Features

> **Benchmark target:** [btop](https://github.com/aristocratos/btop) — a polished, rich, responsive, and visually stunning system monitor TUI.
> **Scope:** Full analysis of the Rust (`src/`) and Python (`plot.py`) codebases.
> No code changes — analysis only.

---

## Table of Contents

- [I. High-Level Architecture Critique](#i-high-level-architecture-critique)
- [II. Rust Codebase — File-by-File Critique](#ii-rust-codebase--file-by-file-critique)
- [III. Python Codebase — `plot.py` Critique](#iii-python-codebase--plotpy-critique)
- [IV. What Sucks (Ranked)](#iv-what-sucks-ranked)
- [V. What's Missing](#v-whats-missing)
- [VI. What Can Be Removed](#vi-what-can-be-removed)
- [VII. Additional Features Proposal](#vii-additional-features-proposal)
- [VIII. btop Gap Analysis Matrix](#viii-btop-gap-analysis-matrix)

---

## I. High-Level Architecture Critique

### The Fundamental Design Problem

Twatch is split across **two languages and two rendering pipelines** for no defensible reason:

| Aspect | twatch | btop |
|---|---|---|
| TUI rendering | Rust (ratatui) — live gauges only | C++ — everything rendered natively |
| Graphing | Delegates to Python (matplotlib) | Built-in braille/block character graphs |
| Process lifecycle | Spawns `python3` as a subprocess | Single process, zero dependencies |
| User experience | Two distinct windows (TUI → matplotlib popup) | Unified, seamless, single-window experience |

**Verdict:** The Python dependency is the single largest architectural smell. btop renders *everything* — graphs, gauges, process lists, network, disks — inside one TUI. Twatch records data in a TUI then ejects you into a completely different GUI toolkit. This breaks the terminal-native promise.

### Session-Based Model vs. Real-Time Model

btop shows **live, scrolling history graphs** — no concept of "sessions" or "captures". Twatch's session/CSV model is closer to a data logger than a monitor. This isn't inherently wrong, but the TUI gives the *impression* of a live monitor while actually being a glorified CSV writer with a progress bar on top.

---

## II. Rust Codebase — File-by-File Critique

---

### `main.rs`

#### What's Wrong

1. **`Config` is a bag of unrelated concerns** (L116-121). `delay`, `no_graph`, `max_plot_temp`, `temp_steps` — half of these are plotting parameters that only matter when shelling out to Python. They're global args even though they're only relevant to `run` and `graph` subcommands.

2. **`global = true` abuse** (L17-46). Every single arg is `global = true`. `--max-temp` and `--temp-steps` make zero sense on `twatch completions bash` or `twatch temp`. btop doesn't expose irrelevant flags per subcommand — each command surface is tight.

3. **`sensor` is a raw `String`** (L84). Should be a `ValueEnum` like `Shell` already is. The match against `"gpu"`, `"nvme"`, `"cpu"` is scattered across `session.rs` as raw string comparisons — a prime candidate for an enum.

4. **No version flag**. `#[command(version)]` is trivially missing. btop has `--version`.

5. **Hardcoded capture default of 250** (L154). This magic number is buried in match arm logic, not in the arg's `default_value`. If a user runs `twatch run` with no flags, they get 250 captures with no indication of why.

6. **`print_sessions()` is in `main.rs`** (L196-210). Presentation logic for listing sessions belongs in `session.rs`. Main should only dispatch.

7. **No error handling strategy**. Some paths `.expect()`, some `eprintln!` + `process::exit(1)`, some return `Result`. There's no consistent pattern. btop has structured error recovery throughout.

#### What's Missing

- No `--version` / `--about`
- No color-theme or config file support
- No `twatch delete <ID>` command to manage sessions
- No `twatch export` (JSON, CSV, etc.)
- No `twatch live` mode that just monitors without recording
- No signal handling (Ctrl+C doesn't flush gracefully in all paths)

---

### `sensors.rs`

#### What's Wrong

1. **Panics on boot** (L15, L27, L39). Three separate `.expect()` calls that will crash twatch if:
   - `/sys/class/hwmon/` doesn't exist (containers, WSL, BSD)
   - A `name` file inside hwmon is missing
   - A temp input file can't be read
   
   btop gracefully degrades — missing sensors are just omitted. Twatch nukes itself.

2. **Linux-only, hardcoded**. The entire module reads from `/sys/class/hwmon/`. No macOS (`IOKit`), no FreeBSD (`sysctl`), no Windows. btop supports all four.

3. **No NVIDIA GPU detection** (L32). `is_amd_gpu` is the only GPU flag. NVIDIA cards use `nvidia-smi` or the `nvidia` hwmon driver (`nct6775`, etc.). Any NVIDIA user gets `Unknown` for their GPU — which means GPU temps are silently dropped.

4. **No Intel GPU detection**. Intel Arc / integrated GPUs (`i915`) are unhandled.

5. **`SensorLabel` is a flat struct with boolean flags** (L3-9). This should be an enum:
   ```
   enum DeviceKind { Cpu, AmdGpu, NvidiaGpu, IntelGpu, Nvme, Unknown }
   ```
   The current pattern of `is_cpu`, `is_amd_gpu`, `is_nvme` doesn't scale and allows invalid states (a sensor can be both CPU and GPU simultaneously).

6. **`device_type()` returns `&'static str`** (L63-73). This function exists solely because `SensorLabel` uses booleans instead of an enum. It would be unnecessary if `DeviceKind` existed.

7. **Integer division truncation** (L40). `temp / 1000` loses sub-degree precision. btop shows tenths of degrees (e.g., `52.3°C`). Twatch rounds to integers, losing useful information for tracking subtle thermal changes — which is literally the tool's purpose.

8. **No caching or rate limiting**. Every call re-reads the entire `/sys/class/hwmon/` tree. The directory structure doesn't change at runtime — sensor discovery should happen once at startup, and only the temp values should be polled per tick.

9. **No sensor sorting**. Results come back in arbitrary filesystem order. btop sorts sensors by type and label.

#### What's Missing

- Fan speed reading (`fan1_input`)
- Power draw reading (`power1_input`, `power1_average`)
- CPU frequency reading (`/proc/cpuinfo` or `scaling_cur_freq`)
- Battery/AC status
- Sensor discovery caching
- Fractional temperature precision (`f64` instead of `u32`)

---

### `session.rs`

#### What's Wrong — Data Layer

1. **CSV format is primitive** (L68-69). `Type,Label,Temp` — no timestamp per row, no session metadata header beyond delay. btop doesn't export, but if it did, it would include timestamps, hostname, kernel version, etc.

2. **No timestamps in recorded data**. The CSV has no time column. The only timing information is the delay header and the `#Total` footer. You cannot reconstruct *when* a spike happened, only *which sample number* it was.

3. **Session directory is hardcoded** (L31, L60). `~/Documents/Twatch/session/` — no `$XDG_DATA_HOME` support, no `--output-dir` flag, no config. btop respects XDG.

4. **Session ID is a sequential u16** (L63-78). The `loop { if !exists { break } }` pattern means:
   - Deleting session_5.csv and running again creates a *new* session_5, not session_N+1.
   - Maximum 65535 sessions before overflow.
   - No way to name sessions ("stress test", "gaming", etc.).

5. **`flush_interval` is hardcoded to 50** (L74). For a 250ms delay, that's a 12.5 second flush window. If the process dies in the first 12 seconds, you lose everything. btop doesn't buffer to files at all, but if it did, it would flush per-frame or use `mmap`.

#### What's Wrong — TUI Rendering

6. **The live TUI is painfully bare compared to btop.** The entire rendering is:
   - A header with mode info
   - Flat `Gauge` bars for each sensor
   - A one-line footer

   Compare to btop:
   - Braille-character real-time scrolling graphs per CPU core
   - Color gradients that shift with temperature
   - Box-drawing layout with rounded corners
   - Per-core sparklines
   - Memory, disk, network panels
   - Process table with sorting

   Twatch's TUI is functionally a progress bar list. There is **no graph in the TUI**. The entire point of a temperature monitor is to see the *trend*, not just the instant value.

7. **No live graph in TUI** (L127-216). ratatui has `widgets::Chart` with `Dataset` and `GraphType::Line`. Twatch doesn't use it. This is the single most obvious missing feature — the ratatui dependency is barely utilized.

8. **`max_temp` gauge scaling is wrong** (L168). `sensors.iter().map(|s| s.temp).max().unwrap_or(100).max(100)` — so the gauge denominator is the current max reading or 100, whichever is higher. This means if one sensor reads 105°C, all gauges rescale. btop uses fixed scales per metric type.

9. **Color scheme is simplistic** (L178-184). Three flat colors: Green < 50, Yellow 50-70, Red >= 70. btop uses smooth HSL gradients — a temperature of 65°C gets a color that's proportionally between green and red. Twatch jumps from green to yellow to red in hard steps.

10. **No per-sensor history** in the TUI. Each frame throws away the previous reading. There's no `VecDeque<f64>` or ring buffer per sensor to feed a live chart.

11. **Temperature value overlay is hacky** (L196-207). A manually positioned `Paragraph` is rendered on top of the gauge with hardcoded offset math (`saturating_sub(10)`, `saturating_add(1)`). This breaks on narrow terminals.

12. **No terminal resize handling**. btop redraws on `SIGWINCH`. Twatch's layout will silently clip if the terminal is resized during a session.

13. **Timing is unreliable** (L280-294). The delay is split: `ms_delay / 4` for event polling + `ms_delay * 3 / 4` for sleep. But `search_sensors()`, `record_frame()`, and `draw()` all take non-zero time, which isn't subtracted. The actual sample rate drifts. btop uses precise tick scheduling.

14. **`elapsed` counter is only incremented in capture-limit mode** (L303-304). In `by_temperature` mode, there's no elapsed counter at all, so the JSON output always shows `"elapsed":0`.

15. **Exit record is always `CPU,Exit,{target}`** (L299, L308). Even if the target sensor is GPU or NVMe, the exit record says `CPU`. Bug.

16. **Terminal isn't created conditionally** (L237-238). Even in `--json` mode, a `Terminal` is created with `CrosstermBackend`. It's never used, but it allocates and initializes the backend for no reason.

#### What's Missing

- Live scrolling chart (ratatui `Chart` widget)
- Per-sensor min/max/avg statistics overlay
- Pause/resume keybind during recording
- Session naming / tagging
- XDG-compliant data directory
- Proper tick timing (subtract render time from delay)
- Terminal resize handling
- Graceful Ctrl+C via signal handler (flush + exit cleanly)

---

### `plot.rs`

#### What's Wrong

1. **This entire module should not exist.** Its only job is to shell out to `python3 plot.py`. In a btop-class tool, graphing is native. This module is 76 lines of subprocess orchestration for functionality that ratatui can provide natively.

2. **`find_plot_script()` is a fragile hack** (L57-75). It searches 4 hardcoded relative paths from the executable. If the binary is installed to `/usr/bin/` and `plot.py` isn't co-located, it silently falls back to `PathBuf::from("plot.py")` which will fail. The Nix flake works around this by copying `plot.py` to `$out/bin/`, but any non-Nix installation is broken.

3. **`find_latest()` sorts by filename** (L53). `session_1.csv` sorts before `session_10.csv` lexicographically but not numerically. So "latest" is actually "last alphabetically", which is `session_9` when sessions 1-10 exist. Bug.

4. **`find_latest()` takes `&PathBuf`** (L47). Clippy would flag this — should be `&Path`.

5. **No error context on spawn failure** (L41-43). Just `"Failed to launch plot: {e}"`. Doesn't say which script path was tried, or what Python was expected.

6. **Blocking `child.wait()`** (L39). The Rust process hangs until the matplotlib window is closed. No option to fire-and-forget the graph.

#### What's Missing

- Everything. This module should be replaced by native ratatui chart rendering.
- If Python graphing is kept: embed the script as `include_str!`, write to a temp file, and execute — eliminating the path search entirely.

---

## III. Python Codebase — `plot.py` Critique

### `plot.py`

#### What's Wrong

1. **Hardcoded TkAgg backend** (L8). `matplotlib.use("TkAgg")` — requires Tk. Fails on headless servers, Wayland-only setups without XWayland, or systems without Tk. btop doesn't have this problem because it doesn't leave the terminal.

2. **Manual argument parsing** (L22-36). A hand-rolled `while` loop instead of `argparse`. No `--help`, no validation, no error messages for malformed args. `--max-temp abc` silently crashes with an unhandled `ValueError`.

3. **No error handling on file I/O** (L41). `open(path)` with no try/except. Missing or unreadable files crash with a raw traceback.

4. **CSV parsing is fragile** (L43-49). Splits on `,` with no quoting support. Labels containing commas break parsing. The header detection (`line.startswith("Type,")`) is brittle — any change to the header format breaks the plotter.

5. **Heat spike detection is statistically weak** (L63-72). "Top 10% of positive diffs" is an arbitrary heuristic. It marks spikes even in perfectly smooth ramps. A proper algorithm would use rolling standard deviation or z-scores.

6. **Color pools are small** (L16-19). 6 colors per category. With 7+ sessions, colors repeat with no visual distinction. btop uses computed HSL with session-count-aware spacing.

7. **`None` padding for unequal session lengths** (L104-105). `temps + [None] * (global_samples - len(temps))` — matplotlib handles `None` in y-data by breaking the line, which creates misleading gaps. Should use `np.nan` for proper discontinuity handling.

8. **Legend becomes unreadable with many sensors** (L126-130). With 10+ sensors across 3 sessions, the legend covers the graph. btop uses inline labels or toggleable overlays.

9. **No interactivity beyond zoom/pan**. No click-to-inspect, no hover tooltips, no time-axis (just "Sample" index). btop has keyboard-navigable focus on graph regions.

10. **The description box is static text** (L132-150). Color information is printed as English words ("CPU = red tones"), not as colored swatches. Useless for colorblind users.

11. **No dark theme support**. White background, light gray grid. Doesn't respect system theme or terminal palette. btop adapts to terminal colors.

12. **The `numpy` import is used for exactly one thing** (L11, L66) — `np.diff()`. This could be a simple list comprehension, eliminating the numpy dependency.

#### What's Missing

- `argparse` with `--help`
- Dark/light theme detection
- Interactive tooltips (mplcursors or similar)
- Time-axis (convert sample index to elapsed time using the `# Delay:` header)
- Export to PNG/SVG from CLI
- Proper statistical spike detection
- Colorblind-friendly palettes

---

## IV. What Sucks (Ranked)

| # | Issue | Severity | btop comparison |
|---|---|---|---|
| 1 | **No live graph in TUI** — the core feature of a temperature monitor is missing from the TUI | 🔴 Critical | btop has scrolling braille graphs for every metric |
| 2 | **Python/matplotlib dependency for graphing** — breaks terminal-native promise | 🔴 Critical | btop is a single binary, zero runtime deps |
| 3 | **Panicking `.expect()` throughout sensors.rs** — crashes on any non-standard Linux system | 🔴 Critical | btop gracefully handles missing sensors |
| 4 | **Linux-only sensor detection** — no macOS, FreeBSD, Windows | 🟠 High | btop supports Linux, macOS, FreeBSD |
| 5 | **No NVIDIA GPU support** — large portion of users get no GPU data | 🟠 High | btop detects all GPU vendors |
| 6 | **No timestamps in recorded data** — cannot correlate spikes to wall-clock time | 🟠 High | N/A (btop is live, not session-based) |
| 7 | **Integer temperature precision** — loses sub-degree changes | 🟡 Medium | btop shows tenths |
| 8 | **Hardcoded `~/Documents/Twatch/session/`** — no XDG | 🟡 Medium | btop uses `$XDG_CONFIG_HOME` |
| 9 | **Hard-stepped color thresholds** — jarring visual transitions | 🟡 Medium | btop uses smooth gradients |
| 10 | **`find_latest()` sorts lexicographically** — returns wrong session | 🟡 Medium | N/A |
| 11 | **No signal handling** — Ctrl+C may lose buffered data | 🟡 Medium | btop catches signals and exits cleanly |
| 12 | **Timing drift** — sample rate is unreliable | 🟡 Medium | btop uses precise tick scheduling |
| 13 | **`global = true` on all CLI args** — pollutes unrelated subcommands | 🟢 Low | N/A |
| 14 | **Manual arg parsing in plot.py** — no `--help`, no validation | 🟢 Low | N/A |
| 15 | **Exit record bug** — always writes `CPU,Exit` regardless of sensor | 🟢 Low (bug) | N/A |

---

## V. What's Missing

### Must-Have (to reach btop parity as a *temperature* tool)

- [ ] **Live scrolling temperature graph in TUI** (ratatui `Chart` widget with `GraphType::Line`)
- [ ] **Per-sensor historical ring buffer** (e.g., `VecDeque<f64>` of last N readings)
- [ ] **Smooth color gradients** based on temperature, not hard thresholds
- [ ] **Sensor discovery caching** — scan once, poll values per tick
- [ ] **Graceful error handling** — replace all `.expect()` with `.ok()` / `?` propagation
- [ ] **Fractional temperatures** (`f64` instead of `u32`)
- [ ] **XDG directory support** (`$XDG_DATA_HOME/twatch/`)
- [ ] **Timestamps per data row** in CSV
- [ ] **`--version` flag**

### Should-Have

- [ ] **NVIDIA GPU detection** (hwmon `nvidia` driver, or `nvidia-smi` fallback)
- [ ] **Intel GPU detection** (`i915`)
- [ ] **Fan speed monitoring**
- [ ] **CPU frequency monitoring**
- [ ] **Power draw monitoring** (RAPL / hwmon power sensors)
- [ ] **Live min/max/avg statistics** per sensor in the TUI
- [ ] **Session naming** (`twatch run --name "stress test"`)
- [ ] **Session management** (`twatch delete`, `twatch export`, `twatch info <ID>`)
- [ ] **Config file** (`~/.config/twatch/config.toml`)
- [ ] **Keybind help overlay** (press `?` to see controls)
- [ ] **Pause/resume recording** during session

### Nice-to-Have

- [ ] **macOS support** (IOKit / `powermetrics`)
- [ ] **Notification/alert on threshold breach** (e.g., beep or desktop notification at 90°C)
- [ ] **Mouse support** in TUI (click sensors to highlight in graph)
- [ ] **Multi-layout modes** (compact / expanded / graph-only)
- [ ] **Plugin/extension system** for custom sensor sources
- [ ] **Remote monitoring** (SSH-based or socket-based sensor polling)
- [ ] **CSV to JSON / Parquet export**

---

## VI. What Can Be Removed

| Item | Reason | Replacement |
|---|---|---|
| **`plot.py` (entire file)** | Python dependency is the biggest architectural liability | Native ratatui `Chart` rendering in Rust |
| **`plot.rs` (entire module)** | Exists only to shell out to `plot.py` | Replaced by native graphing module |
| **`numpy` dependency** (in plot.py) | Used for a single `np.diff()` call | Pure Python list comprehension |
| **`matplotlib` dependency** (in plot.py) | Heavyweight GUI library for a TUI tool | Native ratatui rendering |
| **`pythonWithMatplotlib`** (in flake.nix) | Only needed because of plot.py | Remove once graphing is native |
| **`makeBinaryWrapper`** (in flake.nix) | Only needed to inject Python into PATH | Unnecessary with native graphing |
| **`global = true`** on `max_plot_temp` / `temp_steps` | Only relevant to `run` and `graph` commands | Move to subcommand-specific args |
| **`device_type()` free function** (sensors.rs) | Only exists because `SensorLabel` uses bools | Replace with `DeviceKind` enum + `Display` impl |
| **Manual temperature overlay** (session.rs L196-207) | Hacky positioned Paragraph on top of Gauge | Use ratatui's built-in gauge label or a custom widget |

---

## VII. Additional Features Proposal

### 1. Native TUI Graphing (Priority #1)

Replace the Python plotting pipeline with ratatui's `Chart` widget. Each sensor gets a `Dataset` with a ring buffer of the last N readings. The chart scrolls in real-time, identical to btop's CPU graph but for temperature.

**Impact:** Eliminates Python dependency, `plot.py`, `plot.rs`, matplotlib, numpy, TkAgg, and the entire `find_plot_script()` hack. Reduces the project to a single Rust binary.

### 2. Sensor Abstraction Layer

Create a `trait SensorProvider` with implementations for:
- Linux hwmon (current)
- NVIDIA (`nvidia-smi` or NVML)
- macOS IOKit (future)

This makes the tool extensible and testable.

### 3. Configuration System

A `~/.config/twatch/config.toml` file for:
- Default delay
- Color theme (dark/light/custom)
- Sensor aliases
- Temperature thresholds for color bands
- Default capture limit
- Data directory

### 4. Session Management Suite

```
twatch list                    # Current behavior, but with metadata (date, duration, sensor count)
twatch info <ID>               # Show session details, min/max/avg per sensor
twatch delete <ID...>          # Delete sessions
twatch export <ID> --format json/csv/parquet
twatch rename <ID> "name"      # Tag sessions with human-readable names
twatch compare <ID> <ID>       # Side-by-side diff (currently `graph` does this, but in matplotlib)
```

### 5. Alert System

```
twatch run --alert 85          # Beep + flash when any sensor hits 85°C
twatch run --alert-cmd "notify-send 'Overheating!'"  # Custom alert command
```

### 6. Dashboard Mode

A non-recording live monitor mode:
```
twatch live                    # btop-style live dashboard, no CSV recording
twatch live --record           # Live dashboard + recording
```

---

## VIII. btop Gap Analysis Matrix

| Feature | btop | twatch | Gap |
|---|---|---|---|
| Live scrolling graphs | ✅ Braille chars, per-core | ❌ No graphs in TUI | **Critical** |
| Single binary, no deps | ✅ | ❌ Requires Python + matplotlib + Tk | **Critical** |
| Graceful error handling | ✅ | ❌ Panics on missing sensors | **Critical** |
| Smooth color gradients | ✅ HSL interpolation | ❌ 3 hard color bands | **High** |
| Multi-platform | ✅ Linux/macOS/FreeBSD | ❌ Linux only | **High** |
| GPU vendor coverage | ✅ AMD + NVIDIA + Intel | ⚠️ AMD only | **High** |
| Sub-degree precision | ✅ Shows 0.1°C | ❌ Integer only | **Medium** |
| XDG compliance | ✅ | ❌ Hardcoded ~/Documents | **Medium** |
| Resize handling | ✅ | ❌ | **Medium** |
| Mouse support | ✅ | ❌ | **Low** |
| Theming / skins | ✅ Multiple themes | ❌ | **Low** |
| Config file | ✅ `btop.conf` | ❌ | **Low** |
| Signal handling | ✅ Clean exit | ⚠️ Partial | **Medium** |
| Keyboard shortcuts overlay | ✅ Press `h` | ❌ | **Low** |
| Per-metric statistics | ✅ Min/Max/Avg | ❌ | **Medium** |
| Precise tick timing | ✅ | ❌ Drifts with render time | **Medium** |
| Fan speed | ✅ | ❌ | **Medium** |
| CPU frequency | ✅ | ❌ | **Medium** |
| Power draw | ✅ (where available) | ❌ | **Low** |
| Process list | ✅ | ❌ (out of scope) | N/A |
| Network / Disk I/O | ✅ | ❌ (out of scope) | N/A |
| Memory usage | ✅ | ❌ (out of scope) | N/A |

---

> **Summary:** Twatch has a solid foundation — the session recording model, CLI structure, and sensor polling work. But the TUI is a **data-less progress bar dashboard** that delegates its most important feature (graphing) to an external Python process. To reach btop-level quality *for its thermal monitoring niche*, the single most impactful change is bringing graphing into the TUI natively and eliminating the Python dependency entirely. Everything else follows from that.
