#!/usr/bin/env python3
"""Twatch plotter — interactive matplotlib graph with heat-spike detection."""
import sys
import os
from collections import defaultdict

import matplotlib
matplotlib.use("TkAgg")
import matplotlib.pyplot as plt
import matplotlib.ticker as mticker
import numpy as np

MAX_TEMP = 110
TEMP_STEPS = 5

CPU_COLORS = ["crimson", "darkorange", "salmon", "indianred", "orangered", "lightcoral"]
GPU_COLORS = ["forestgreen", "limegreen", "darkgreen", "mediumseagreen", "springgreen", "seagreen"]
OTHER_COLORS = ["dimgray", "darkgray", "slategray", "lightslategray", "silver", "gray"]
SPIKE_COLORS = ["darkviolet", "mediumorchid", "indigo", "blueviolet", "purple", "darkmagenta"]


def parse_args():
    args = sys.argv[1:]
    kwargs = {"paths": [], "max_temp": MAX_TEMP, "temp_steps": TEMP_STEPS}
    i = 0
    while i < len(args):
        if args[i] == "--max-temp" and i + 1 < len(args):
            kwargs["max_temp"] = int(args[i + 1])
            i += 2
        elif args[i] == "--temp-steps" and i + 1 < len(args):
            kwargs["temp_steps"] = int(args[i + 1])
            i += 2
        else:
            kwargs["paths"].append(args[i])
            i += 1
    return kwargs


def load_csv(path):
    series = defaultdict(list)
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#") or line.startswith("Type,"):
                continue
            parts = line.split(",")
            if len(parts) >= 3:
                typ, label, temp = parts[0], parts[1], float(parts[2])
                series[(typ, label)].append(temp)
    return dict(series)


def session_color(si, typ):
    """Distinct color per session per device type."""
    if typ == "CPU":
        return CPU_COLORS[si % len(CPU_COLORS)]
    elif typ == "GPU":
        return GPU_COLORS[si % len(GPU_COLORS)]
    else:
        return OTHER_COLORS[si % len(OTHER_COLORS)]


def find_heat_spikes(temps, margin_pct=10):
    if len(temps) < 3:
        return []
    diffs = np.diff(temps)
    positive = [(i + 1, d) for i, d in enumerate(diffs) if d > 0]
    if not positive:
        return []
    positive.sort(key=lambda x: -x[1])
    n = max(1, int(len(temps) * margin_pct / 100))
    return sorted(idx for idx, _ in positive[:n])


def plot(paths, max_temp=MAX_TEMP, temp_steps=TEMP_STEPS):
    if not paths:
        print("No session files provided")
        sys.exit(1)

    multi = len(paths) > 1

    fig, ax = plt.subplots(figsize=(14, 7))
    suffix = " vs ".join(f"S{i}" for i in range(len(paths))) if multi else ""
    title = f"Sessions [{suffix}]" if multi else os.path.basename(paths[0])
    fig.canvas.manager.set_window_title(f"Twatch — {title}")

    all_series = [load_csv(p) for p in paths]
    global_samples = max(
        max((len(v) for v in s.values()), default=0) for s in all_series
    )
    xs = list(range(global_samples))

    for si, data in enumerate(all_series):
        for (typ, label), temps in data.items():
            c = session_color(si, typ)
            alpha = 1.0 if typ in ("CPU", "GPU") else 0.3
            lw = 2.0 if typ in ("CPU", "GPU") else 1.0

            if multi:
                lbl = f"S{si + 1} {typ}.{label}"
            else:
                lbl = f"{typ}.{label}"

            pad = [None] * (global_samples - len(temps))
            y = temps + pad
            ax.plot(xs, y, color=c, alpha=alpha, linewidth=lw, label=lbl)

            if typ in ("CPU", "GPU"):
                spike_idx = find_heat_spikes(temps)
                spike_x = [i for i in spike_idx if i < len(temps)]
                spike_y = [temps[i] for i in spike_idx]
                if spike_x:
                    sc = SPIKE_COLORS[si % len(SPIKE_COLORS)]
                    ax.scatter(spike_x, spike_y, color=sc, marker="v",
                               s=60, zorder=10, alpha=0.85, edgecolors="black",
                               linewidths=0.4)

    ax.set_xlabel("Sample")
    ax.set_ylabel("Temperature (°C)")
    ax.set_title(title)
    ax.set_ylim(0, max_temp)
    ax.yaxis.set_major_locator(mticker.MultipleLocator(temp_steps))
    ax.grid(True, alpha=0.3)

    if multi:
        legend = ax.legend(fontsize=7, ncol=2, framealpha=0.9,
                           title="Session — Sensor")
        legend.get_title().set_fontsize(8)
    else:
        ax.legend(fontsize=8)

    desc = ["▼ = heat spike (fastest 10% rise)"]
    desc.append("CPU = red tones  ·  GPU = green tones  ·  other = gray")
    if multi:
        parts = []
        for si in range(len(paths)):
            cp = CPU_COLORS[si % len(CPU_COLORS)]
            gp = GPU_COLORS[si % len(GPU_COLORS)]
            parts.append(f"S{si + 1} ~ {cp} / {gp}")
        desc.append("  |  ".join(parts))

    desc_text = "\n".join(desc)
    ax.text(
        0.99, 0.01, desc_text,
        transform=ax.transAxes,
        fontsize=6.5, color="dimgray",
        verticalalignment="bottom", horizontalalignment="right",
        bbox=dict(boxstyle="round,pad=0.4", facecolor="white", alpha=0.85,
                  edgecolor="lightgray", linewidth=0.5)
    )

    plt.tight_layout()
    plt.show()


if __name__ == "__main__":
    kwargs = parse_args()
    if not kwargs["paths"]:
        print("Usage: plot.py [--max-temp N] [--temp-steps N] <session.csv> ...")
        sys.exit(1)
    plot(kwargs["paths"], kwargs["max_temp"], kwargs["temp_steps"])
