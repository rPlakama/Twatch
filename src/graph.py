import matplotlib.pyplot as plt
import pandas as pd


df = pd.read_csv("../session_0.csv")


plt.figure(figsize=(12,6))
for (t, label), group in df.groupby(['Type', 'Label']):
    plt.plot(group.index, group['Temp'], label=f"{t}-{label}")

plt.xlabel('Sample Index')
plt.ylabel('Temperature (°C)')
plt.title('Temperature Trends by Sensor Type and Label')
plt.legend(loc='upper left', fontsize='small')
plt.grid(True)
plt.tight_layout()
plt.show()

