
import sys
import re
from collections import namedtuple

# Mock ActionRecord
ActionRecord = namedtuple('ActionRecord', ['action_type', 'app', 'control_name', 'control_type', 'final_value'])

class PatternAnalyzer:
    def _refine_action(self, action):
        parts = [action.action_type, action.app or ""]

        if action.control_name:
            parts.append(action.control_name)
        if action.control_type:
            parts.append(action.control_type)

        # 값이 있으면 placeholder로 대체 (패턴에서 변수 분리)
        if action.final_value:
            # URL, 이메일 등 변수 부분을 placeholder로
            value = action.final_value
            value = re.sub(r'[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+', '{EMAIL}', value)
            value = re.sub(r'https?://\S+', '{URL}', value)
            value = re.sub(r'\d{4}[-/]\d{1,2}[-/]\d{1,2}', '{DATE}', value)
            value = re.sub(r'\d+', '{NUM}', value)
            if value != action.final_value:
                parts.append(value[:50])
        
        return "|".join(parts)

analyzer = PatternAnalyzer()

# Test Case 1: Same input (should match)
a1 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "daily report")
a2 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "daily report")
print(f"Test 1 (daily vs daily): {analyzer._refine_action(a1) == analyzer._refine_action(a2)}") 
# Expected: True

# Test Case 2: Different input (should currently match because content ignored, but we want False later)
b1 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "daily report")
b2 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "weekly report")
print(f"Test 2 (daily vs weekly): {analyzer._refine_action(b1) == analyzer._refine_action(b2)}")
# Expected: True (currently broken behavior we want to fix is False)

# Test Case 3: Sensitive input (should mask)
c1 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "user@gmail.com")
c2 = ActionRecord("input", "CHROME", "AddressBar", "Edit", "{EMAIL}")
print(f"Test 3 (email masking): {analyzer._refine_action(c1)}")
