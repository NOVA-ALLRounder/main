"""
ARI (Autonomous Research Intelligence) Package
"""

__version__ = "0.1.0"
__author__ = "ARI Team"

from .config import get_config, ARIConfig

__all__ = ["get_config", "ARIConfig", "__version__"]
