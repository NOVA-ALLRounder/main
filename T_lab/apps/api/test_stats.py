
import asyncio
import numpy as np
import scipy.stats as stats

def test_calc():
    # Mock data N=50
    # Control: Mean=50, Std=10
    # Exp: Mean=55, Std=10 (Effect size should be ~0.5)
    np.random.seed(42)  # Fix seed for reproducibility
    control = np.random.normal(50, 10, 50)
    experimental = np.random.normal(55, 10, 50)
    
    current_n = 50
    
    # 1. Effect Size (Cohen's d)
    mean_diff = np.mean(experimental) - np.mean(control)
    pooled_std = np.sqrt((np.std(control, ddof=1)**2 + np.std(experimental, ddof=1)**2) / 2)
    current_d = mean_diff / pooled_std if pooled_std != 0 else 0
    
    print(f"Mean Diff: {mean_diff:.4f}")
    print(f"Pooled Std: {pooled_std:.4f}")
    print(f"Effect Size (d): {current_d:.4f}")
    
    # 2. Power
    try:
        # Non-centrality parameter
        ncp = current_d * np.sqrt(current_n / 2)
        # Critical t-value
        df = 2*current_n - 2
        crit_t = stats.t.ppf(1 - 0.05/2, df=df)
        
        # Power using nct (Non-central t distribution)
        # We need to import nct if available, or check if stats.t handles 'nc' param
        # scipy.stats.nct is the correct distribution for non-central t
        
        power = 1 - stats.nct.cdf(crit_t, df, nc=ncp) + stats.nct.cdf(-crit_t, df, nc=ncp)
        print(f"Power: {power:.4f}")
        
    except Exception as e:
        print(f"Power Calc Error: {e}")

test_calc()
