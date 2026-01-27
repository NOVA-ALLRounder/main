import matplotlib.pyplot as plt
import numpy as np

# Sample data
np.random.seed(0)
time = np.arange(0, 10, 0.1)
metric1 = np.sin(time) + np.random.normal(0, 0.1, len(time))
metric2 = np.cos(time) + np.random.normal(0, 0.1, len(time))
metric3 = np.tan(time) + np.random.normal(0, 0.1, len(time))

# Create a multi-panel plot
plt.style.use('seaborn-whitegrid')
fig, axs = plt.subplots(3, 1, figsize=(10, 8), sharex=True)

# Plot each metric
axs[0].plot(time, metric1, label='Metric 1', color='b')
axs[0].set_ylabel('Metric 1')
axs[0].legend(loc='upper right')

axs[1].plot(time, metric2, label='Metric 2', color='g')
axs[1].set_ylabel('Metric 2')
axs[1].legend(loc='upper right')

axs[2].plot(time, metric3, label='Metric 3', color='r')
axs[2].set_ylabel('Metric 3')
axs[2].set_xlabel('Time')
axs[2].legend(loc='upper right')

# Set the main title
fig.suptitle('Experiment Results: Metrics Over Time', fontsize=14)

# Adjust layout
plt.tight_layout(rect=[0, 0.03, 1, 0.95])

# Save the plot to a file
plt.savefig('experiment_results.png')

# Show the plot
plt.show()