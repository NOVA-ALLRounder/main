
import numpy as np
import json
import logging
from sklearn.linear_model import LinearRegression
from sklearn.model_selection import train_test_split
from sklearn.metrics import mean_squared_error

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

class IRS_MIMO_NOMA_Simulation:
    def __init__(self, num_users=2, num_antennas=4, num_reflectors=8):
        self.num_users = num_users
        self.num_antennas = num_antennas
        self.num_reflectors = num_reflectors
        self.irs_phase_shifts = np.random.uniform(0, 2 * np.pi, self.num_reflectors)
        logging.info("Initialized IRS_MIMO_NOMA_Simulation with %d users, %d antennas, and %d reflectors.", 
                     self.num_users, self.num_antennas, self.num_reflectors)

    def simulate_channel(self):
        # Simulate a MIMO-NOMA channel with IRS
        try:
            channel_matrix = np.random.randn(self.num_users, self.num_antennas)
            irs_matrix = np.exp(1j * self.irs_phase_shifts)
            effective_channel = channel_matrix @ irs_matrix[:self.num_antennas]
            logging.info("Channel simulation completed.")
            return effective_channel
        except Exception as e:
            logging.error("Error during channel simulation: %s", e)
            return None

    def lmmse_detection(self, effective_channel):
        try:
            noise_variance = 0.1
            H = effective_channel
            H_Herm = np.conjugate(H.T)
            lmmse_matrix = np.linalg.inv(H_Herm @ H + noise_variance * np.eye(H.shape[1])) @ H_Herm
            logging.info("LMMSE detection completed.")
            return lmmse_matrix
        except Exception as e:
            logging.error("Error during LMMSE detection: %s", e)
            return None

    def optimize_irs(self, effective_channel):
        try:
            # Using a simple linear regression as a placeholder for ML optimization
            X, y = np.real(effective_channel), np.imag(effective_channel)
            X_train, X_test, y_train, y_test = train_test_split(X, y, test_size=0.2, random_state=42)
            model = LinearRegression()
            model.fit(X_train, y_train)
            predictions = model.predict(X_test)
            mse = mean_squared_error(y_test, predictions)
            logging.info("IRS optimization completed with MSE: %.4f", mse)
            return mse
        except Exception as e:
            logging.error("Error during IRS optimization: %s", e)
            return None

    def run_experiment(self):
        try:
            effective_channel = self.simulate_channel()
            if effective_channel is not None:
                lmmse_matrix = self.lmmse_detection(effective_channel)
                mse = self.optimize_irs(effective_channel)
                results = {
                    'effective_channel': effective_channel.tolist(),
                    'lmmse_matrix': lmmse_matrix.tolist() if lmmse_matrix is not None else None,
                    'mse': mse
                }
                with open('irs_mimo_noma_results.json', 'w') as f:
                    json.dump(results, f, indent=4)
                logging.info("Experiment completed and results saved.")
        except Exception as e:
            logging.error("Error during experiment execution: %s", e)

if __name__ == "__main__":
    simulation = IRS_MIMO_NOMA_Simulation()
    simulation.run_experiment()
