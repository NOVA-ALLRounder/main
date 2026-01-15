import numpy as np
import json
import logging

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(asctime)s - %(levelname)s - %(message)s')

# Constants
NUM_TRANSMITTERS = 2
NUM_RECEIVERS = 2
NUM_IRS_ELEMENTS = 4
NOISE_VARIANCE = 1.0

# Helper functions
def awgn_channel(signal, noise_variance):
    """Simulate an AWGN channel."""
    noise = np.random.normal(0, np.sqrt(noise_variance), signal.shape)
    return signal + noise

def mutual_information(signal, noise_variance):
    """Calculate mutual information given a signal and noise variance."""
    signal_power = np.mean(np.abs(signal)**2)
    snr = signal_power / noise_variance
    return np.log2(1 + snr)

def configure_irs(num_elements):
    """Randomly configure IRS elements to reflect signals."""
    return np.random.rand(num_elements) * 2 * np.pi

def simulate_mimo_noma_system(num_transmitters, num_receivers, num_irs_elements):
    """Simulate the MIMO-NOMA system with IRS."""
    try:
        logging.info('Simulating MIMO-NOMA system with IRS...')

        # Generate random signals for transmitters
        signals = np.random.randn(num_transmitters, 100)

        # Configure IRS
        irs_configuration = configure_irs(num_irs_elements)
        logging.info(f'IRS configuration: {irs_configuration}')

        # Simulate channel
        received_signals = awgn_channel(signals, NOISE_VARIANCE)

        # Calculate mutual information
        mi = mutual_information(received_signals, NOISE_VARIANCE)
        logging.info(f'Mutual Information: {mi}')

        return {
            'irs_configuration': irs_configuration.tolist(),
            'mutual_information': mi
        }

    except Exception as e:
        logging.error(f'Error during simulation: {e}')
        return None

def main():
    results = []

    # Simulate several scenarios
    for _ in range(10):
        result = simulate_mimo_noma_system(NUM_TRANSMITTERS, NUM_RECEIVERS, NUM_IRS_ELEMENTS)
        if result:
            results.append(result)

    # Save results to a JSON file
    with open('simulation_results.json', 'w') as f:
        json.dump(results, f, indent=4)
    logging.info('Results saved to simulation_results.json')

if __name__ == '__main__':
    main()