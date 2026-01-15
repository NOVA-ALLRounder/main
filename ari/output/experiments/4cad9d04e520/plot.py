# Import necessary libraries
import matplotlib.pyplot as plt
import numpy as np

# Sample data
# Time points
x = np.linspace(0, 10, 100)
# Metrics over time
metric1 = np.sin(x)
metric2 = np.cos(x)
metric3 = np.tan(x)

# Create a figure and axis objects
fig, axs = plt.subplots(3, 1, figsize=(8, 12), sharex=True)

# Plot data on each subplot
axs[0].plot(x, metric1, label='Metric 1', color='b')
axs[0].set_title('Metric 1 over Time')
axs[0].set_ylabel('Metric 1')
axs[0].legend()
axs[0].grid(True)

axs[1].plot(x, metric2, label='Metric 2', color='g')
axs[1].set_title('Metric 2 over Time')
axs[1].set_ylabel('Metric 2')
axs[1].legend()
axs[1].grid(True)

axs[2].plot(x, metric3, label='Metric 3', color='r')
axs[2].set_title('Metric 3 over Time')
axs[2].set_xlabel('Time')
axs[2].set_ylabel('Metric 3')
axs[2].legend()
axs[2].grid(True)

# Adjust layout for better spacing
plt.tight_layout()

# Save the plot to a file
plt.savefig('experiment_results.png', dpi=300)

# Show the plot
plt.show()