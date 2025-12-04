import glob
import matplotlib.pyplot as plt
import pandas as pd
import sys

sessions = glob.glob("session/session_*.csv")
sessions.sort()

count = len(sessions)

if count < 1:
    print(f"N of Sessions: {count}")
    print("Sessions not detected, quitting.")
    sys.exit(0)

target_file = sessions[-1]
print(f"Reading file: {target_file}")

total_sec = "Unknown"
delay_val = "Unknown"

try:
    with open(target_file, "r") as f:
        lines = f.readlines()

        if lines:
            if lines[0].startswith("#"):
                if "Delay" in lines[0]:
                    parts = lines[0].split(":")
                    if len(parts) > 1:
                        delay_val = parts[1].strip()

            for line in reversed(lines):
                if line.startswith("#Total") or line.startswith("# Total"):
                    parts = line.split(":")
                    if len(parts) > 1:
                        total_sec = int(parts[1].strip())
                    break
except Exception as e:
    print(f"Warning: Could not read metadata headers: {e}")
try:
    df = pd.read_csv(target_file, comment="#")
except pd.errors.EmptyDataError:
    print("File is empty or contains no valid data.")
    sys.exit(0)

plt.figure(figsize=(12, 6))

if "Type" not in df.columns:
    print("Error: Column 'Type' not found. CSV format might be incorrect.")
    print(f"Columns found: {df.columns}")
    sys.exit(1)

for (t, label), group in df.groupby(["Type", "Label"]):
    plt.plot(group.index, group["Temp"], label=f"{t}-{label}")

plt.axhspan(70, 80, alpha=0.1, color="orange", label="Warm (70-80°C)")
plt.axhspan(80, 100, alpha=0.1, color="red", label="Critical (>80°C)")

plt.xlabel("Sample Count")
plt.ylabel("Temperature (°C)")

title_str = "Session Graphical Information"
if total_sec != "Unknown":
    if total_sec < 60:
        title_str += f"\nTotal Duration: {total_sec}s"
    else:
        title_str += f"\nTotal Duration: {total_sec // 60}m {total_sec - 60}s"

if delay_val != "Unknown":
    title_str += f" | Delay: {delay_val}ms"

plt.title(title_str)

plt.legend(loc="upper left", fontsize="small", bbox_to_anchor=(1, 1))
plt.grid(True, linestyle="--", alpha=0.7)
plt.tight_layout()

plt.show()
