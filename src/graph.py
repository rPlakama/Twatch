import glob
import matplotlib.pyplot as plt
import pandas as pd


sessions = glob.glob("../sessions/session_*")
last_session = sessions[-1]
df = pd.read_csv(f"{last_session}")


plt.figure(figsize=(12, 6))
for (t, label), group in df.groupby(["Type", "Label"]):
    plt.plot(group.index, group["Temp"], label=f"{t}-{label}")

plt.xlabel("Captures")
plt.ylabel("Temperature (°C)")
plt.title("HWMON Devices Temperature")
plt.legend(loc="upper left", fontsize="small")
plt.grid(True)
plt.tight_layout()
plt.show()
