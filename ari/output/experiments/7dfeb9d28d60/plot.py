import matplotlib.pyplot as plt
import numpy as np

# Sample data
np.random.seed(0)
time = np.arange(0, 10, 0.1)
metric1 = np.sin(time) + np.random.normal(0, 0.1, len(time))
metric2 = np.cos(time) + np.random.normal(0, 0.1, len(time))

# Create a figure and axis objects
fig, axs = plt.subplots(2, 1, figsize=(10, 8), sharex=True)

# Plot Metric 1
axs[0].plot(time, metric1, label='Metric 1', color='b')
axs[0].set_title('Experiment Results: Metric 1')
axs[0].set_ylabel('Value')
axs[0].legend(loc='upper right')
axs[0].grid(True)

# Plot Metric 2
axs[1].plot(time, metric2, label='Metric 2', color='r')
axs[1].set_title('Experiment Results: Metric 2')
axs[1].set_xlabel('Time')
axs[1].set_ylabel('Value')
axs[1].legend(loc='upper right')
axs[1].grid(True)

# Adjust layout for better appearance
plt.tight_layout()

# Save the plot to a file
plt.savefig('experiment_results.png', dpi=300)

# Show the plot
plt.show()