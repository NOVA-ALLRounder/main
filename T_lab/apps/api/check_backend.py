import sys
import os

try:
    print("Attempting to import main...")
    import main
    print("Import successful!")
except Exception as e:
    print(f"Import failed: {e}")
    import traceback
    traceback.print_exc()
