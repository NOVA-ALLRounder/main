import matplotlib.pyplot as plt
import numpy as np

# Sample data
np.random.seed(0)
time = np.arange(0, 10, 0.1)
metric1 = np.sin(time) + np.random.normal(0, 0.1, len(time))
metric2 = np.cos(time) + np.random.normal(0, 0.1, len(time))
metric3 = np.sin(time / 2) + np.random.normal(0, 0.1, len(time))

# Create a figure and a set of subplots
fig, axs = plt.subplots(3, 1, figsize=(8, 10), sharex=True)

# Plot each metric in a separate subplot
axs[0].plot(time, metric1, label='Metric 1', color='tab:blue')
axs[0].set_title('Experiment Results Over Time')
axs[0].set_ylabel('Metric 1')
axs[0].legend(loc='upper right')
axs[0].grid(True)

axs[1].plot(time, metric2, label='Metric 2', color='tab:orange')
axs[1].set_ylabel('Metric 2')
axs[1].legend(loc='upper right')
axs[1].grid(True)

axs[2].plot(time, metric3, label='Metric 3', color='tab:green')
axs[2].set_xlabel('Time')
axs[2].set_ylabel('Metric 3')
axs[2].legend(loc='upper right')
axs[2].grid(True)

# Adjust layout for better spacing
plt.tight_layout()

# Save the plot to a file
plt.savefig('experiment_results.png', dpi=300)

# Show the plot
plt.show()