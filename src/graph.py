import glob
import matplotlib.pyplot as plt
import pandas as pd
import sys

sessions = glob.glob("session/session_*.csv")
count = len(sessions)

if count < 1:
    print(f"N of Sessions: {count}")
    print("Sessions not detected, quitting.")
    sys.exit(0)
else:
    target_file = sessions[-1]
    capture_delay = "Unknown"
    with open(target_file, "r") as f:
        first_line = f.readline().strip()
        if "Delay:" in first_line:
            capture_delay = first_line.split(":")[1].strip()

    if capture_delay != "Unknown":
        df = pd.read_csv(target_file, skiprows=1)
    else:
        df = pd.read_csv(target_file)

    plt.figure(figsize=(12, 6))

    for (t, label), group in df.groupby(["Type", "Label"]):
        plt.plot(group.index, group["Temp"], label=f"{t}-{label}")

    plt.axhspan(70, 80, alpha=0.05, color="orange", label="Warm")
    plt.axhspan(80, 100, alpha=0.05, color="red", label="Critical")

    plt.xlabel("Captures")
    plt.ylabel("Temperature (°C)")

    plt.title(f"HWMON Devices Temperature (Delay: {capture_delay}ms)")

    plt.legend(loc="upper left", fontsize="small")
    plt.grid(True)
    plt.tight_layout()
    plt.show()
