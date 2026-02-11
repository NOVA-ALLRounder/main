
import sys
import os
sys.path.insert(0, os.path.abspath("src"))

from collector.pattern_analyzer import PatternAnalyzer
from collections import namedtuple

# Mock ActionRecord
ActionRecord = namedtuple('ActionRecord', ['action_type', 'app', 'control_name', 'control_type', 'final_value'])

# Mock Store
class MockStore:
    def insert_pattern(self, data): pass

analyzer = PatternAnalyzer(MockStore())

# Test Case 1: Same input (should match)
a1 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "daily report")
a2 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "daily report")
res1 = analyzer._refine_action(a1)
res2 = analyzer._refine_action(a2)
print(f"Test 1 (daily vs daily): {res1 == res2}")
print(f"  Result: {res1}")

# Test Case 2: Different input (should NOT match now)
b1 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "daily report")
b2 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "weekly report")
res3 = analyzer._refine_action(b1)
res4 = analyzer._refine_action(b2)
print(f"Test 2 (daily vs weekly): {res3 == res4}") 
print(f"  Result 1: {res3}")
print(f"  Result 2: {res4}")

# Test Case 3: Sensitive input (should mask)
c1 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "user@gmail.com")
res5 = analyzer._refine_action(c1)
print(f"Test 3 (email masking): {res5}")
