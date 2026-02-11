Write-Host "🔧 Setting up Steer Environment (DATA_C)..."
$ErrorActionPreference = "Stop"

# Check if conda installed
if (-not (Get-Command "conda" -ErrorAction SilentlyContinue)) {
    Write-Error "❌ Conda (Anaconda/Miniconda) not found. Please install it first."
    exit 1
}

# Create environment
Write-Host "📦 Creating conda environment 'DATA_C' (Python 3.10)..."
try {
    conda create -n DATA_C python=3.10 -y
}
catch {
    Write-Warning "Environment might already exist or failed. Trying to update..."
}

# Install dependencies
Write-Host "📦 Installing dependencies from requirements.txt..."
$ReqPath = Join-Path $PSScriptRoot "collector\Data-Collection-Projection\requirements.txt"

if (Test-Path $ReqPath) {
    # Activate and install
    # Note: 'conda activate' doesn't work well inside script without hook.
    # We use 'conda run' instead.
    conda run -n DATA_C python -m pip install -r $ReqPath
    
    # Also install numpy/pandas if missing (often needed for data collection)
    conda run -n DATA_C python -m pip install numpy pandas requests pyyaml pydantic
}
else {
    Write-Error "❌ requirements.txt not found at $ReqPath"
    exit 1
}

Write-Host "✅ Setup Complete! Now run '.\start_steer.ps1' again."
