# =============================================================================
# T_lab Agent - Engineer (Code Execution)
# Generates and executes Python code for experiments
# =============================================================================

from typing import Dict, Any
import json
import sys
from io import StringIO
import numpy as np

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from core import get_settings, get_logger
from domain_knowledge import domain_store

settings = get_settings()
logger = get_logger("agents.engineer")


def convert_numpy_types(obj):
    """Convert numpy types to Python native types for JSON serialization."""
    if isinstance(obj, dict):
        return {k: convert_numpy_types(v) for k, v in obj.items()}
    elif isinstance(obj, list):
        return [convert_numpy_types(item) for item in obj]
    elif isinstance(obj, tuple):
        return tuple(convert_numpy_types(item) for item in obj)
    elif isinstance(obj, (np.bool_, bool)):  # np.bool is deprecated/removed in 2.0, use native bool or np.bool_
        return bool(obj)
    elif isinstance(obj, np.integer):  # Covers int8...int64 etc.
        return int(obj)
    elif isinstance(obj, np.floating): # Covers float16...float64 etc.
        return float(obj)
    elif isinstance(obj, np.ndarray):
        return obj.tolist()
    elif isinstance(obj, np.complexfloating): # Covers complex64, complex128
        return complex(obj)
    return obj


ENGINEER_PROMPT = """You are a computational scientist who writes Python code for experiments.

Given a hypothesis and selected methodology, write Python code that:

1. **SELECTS THE APPROPRIATE MATHEMATICAL MODEL based on the domain:**
   - Chemistry: Arrhenius equation (k = A*exp(-Ea/RT)), reaction kinetics
   - Physics: ODE solver for motion equations, energy conservation
   - Biology: Logistic growth model (dN/dt = rN(1-N/K)), population dynamics
   - Social Science: Agent-based model, network simulation
   - Wireless/Communication: Channel capacity (Shannon), MIMO capacity formula
   - Statistics/General: Monte Carlo simulation, t-test

2. **MUST define a `SIMULATION_RESULTS` dictionary at the end with this EXTENDED structure:**
   ```python
   SIMULATION_RESULTS = {
       # === Experiment Design (실험 설계) ===
       "experiment_design": {
           "title": "실험 제목 (예: MIMO vs SISO 채널 용량 비교)",
           "methodology": "사용한 수학적 모델/방법론 설명",
           "variables": {
               "independent": ["변수1", "변수2"],  # 독립변수
               "dependent": "측정값",               # 종속변수
               "controlled": ["상수1", "상수2"]     # 통제변수
           },
           "parameters": {
               "param1": value1,  # 실제 사용한 파라미터들
               "param2": value2
           }
       },
       
       # === Metrics (도메인별 맞춤 측정 결과) ===
       "metrics": [
           {"name": "측정항목1", "value": 값, "unit": "단위"},
           {"name": "측정항목2", "value": 값, "unit": "단위"},
           # 예: {"name": "SISO Capacity", "value": 3.3, "unit": "bps/Hz"}
           # 예: {"name": "반응 속도 (300K)", "value": 0.01, "unit": "mol/L/s"}
       ],
       
       # === Steps (실험 진행 단계) ===
       "steps": [
           {"step": 1, "description": "단계 설명", "status": "completed"},
           {"step": 2, "description": "단계 설명", "status": "completed"},
       ],
       
       # === Statistical Results (통계 결과) ===
       "significant": True/False,
       "p_value": 0.0XX,                     
       "effect_size": 0.XX,                  
       "test_statistic": X.XX,
       "conclusion": "Brief conclusion in Korean"
   }
   ```

3. **SAVES visualization to `static/{SESSION_ID}_result.png`**

## DOMAIN-SPECIFIC EXAMPLES:

### Wireless (MIMO Capacity):
```python
SIMULATION_RESULTS = {
    "experiment_design": {
        "title": "MIMO vs SISO 채널 용량 비교 실험",
        "methodology": "Shannon 용량 공식 및 MIMO 용량 공식 적용",
        "variables": {
            "independent": ["안테나 수 (Nt, Nr)", "SNR (dB)"],
            "dependent": "채널 용량 (bps/Hz)",
            "controlled": ["채널 모델 (Rayleigh)", "대역폭"]
        },
        "parameters": {"Nt": 4, "Nr": 4, "SNR_range": "0-20 dB", "trials": 1000}
    },
    "metrics": [
        {"name": "SISO 평균 용량", "value": 3.32, "unit": "bps/Hz"},
        {"name": "MIMO 평균 용량", "value": 11.45, "unit": "bps/Hz"},
        {"name": "용량 증가율", "value": 245, "unit": "%"}
    ],
    "steps": [
        {"step": 1, "description": "채널 행렬 H 생성 (Rayleigh fading)", "status": "completed"},
        {"step": 2, "description": "SISO 용량 계산: C = log2(1 + SNR)", "status": "completed"},
        {"step": 3, "description": "MIMO 용량 계산: C = log2(det(I + SNR/Nt * HH†))", "status": "completed"},
        {"step": 4, "description": "1000회 Monte Carlo 시뮬레이션", "status": "completed"}
    ],
    "significant": True,
    "p_value": 0.0001,
    "effect_size": 2.45,
    "test_statistic": 45.2,
    "conclusion": "MIMO 기술이 SISO 대비 약 3.5배의 채널 용량 증가를 보임"
}
```

### Chemistry (Arrhenius):
```python
SIMULATION_RESULTS = {
    "experiment_design": {
        "title": "온도에 따른 반응 속도 변화 분석",
        "methodology": "Arrhenius 방정식 k = A·exp(-Ea/RT) 적용",
        "variables": {
            "independent": ["온도 (K)"],
            "dependent": "반응 속도 상수 k",
            "controlled": ["활성화 에너지", "빈도 인자"]
        },
        "parameters": {"A": 1e13, "Ea": 50000, "R": 8.314, "T_range": "300-500K"}
    },
    "metrics": [
        {"name": "k (300K)", "value": 0.0012, "unit": "s⁻¹"},
        {"name": "k (400K)", "value": 0.089, "unit": "s⁻¹"},
        {"name": "k (500K)", "value": 2.34, "unit": "s⁻¹"}
    ],
    ...
}
```

CRITICAL RULES:
- Use only: numpy, scipy, pandas, matplotlib
- DO NOT use plt.show()
- MUST define SIMULATION_RESULTS with the extended structure
- All text in Korean where applicable

Return JSON:
{
    "code": "# Python code here\\nimport numpy as np\\n...",
    "description": "What this code does",
    "expected_output": "What results to expect"
}
"""


class EngineerAgent:
    """Engineer agent for code generation and execution."""
    
    def __init__(self):
        self.llm = None
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model=settings.default_model,
                temperature=0.3,
                api_key=settings.openai_api_key
            )
    
    def execute(self, state: ScientificState) -> ScientificState:
        """Generate and execute experiment code."""
        hypothesis = state.get('user_input', '')
        method = state.get('selected_method', {})
        literature = state.get('literature_context', [])
        domain = state.get('domain', '')
        
        logger.info("Executing experiment", method=method.get('title', 'Unknown'), source="engineer")
        
        # Generate code with domain knowledge injection
        if self.llm:
            code_result = self._generate_code_with_llm(hypothesis, method, literature, domain)
        else:
            code_result = self._generate_mock_code(hypothesis, method)
        
        state['experiment_code'] = code_result.get('code', '')
        
        # Execute code safely
        execution_result = self._execute_code(state['experiment_code'], state.get('session_id', 'unknown'))
        
        # Build experiment_results with extracted SIMULATION_RESULTS
        experiment_results = {
            "code": state['experiment_code'],
            "output": execution_result.get('output', ''),
            "error": execution_result.get('error'),
            "success": execution_result.get('success', False)
        }
        
        # Include SIMULATION_RESULTS from executed code if available
        # Convert numpy types to native Python types for JSON serialization
        if execution_result.get('simulation_results'):
            experiment_results['simulation_results'] = convert_numpy_types(
                execution_result['simulation_results']
            )
            logger.info("Engineer code produced SIMULATION_RESULTS", source="engineer")
        
        state['experiment_results'] = convert_numpy_types(experiment_results)
        state['execution_logs'] = [execution_result.get('output', '')]
        state['current_step'] = 'experiment_executed'
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "engineer",
            "code_generated": bool(state['experiment_code']),
            "execution_success": execution_result.get('success', False),
            "has_simulation_results": bool(execution_result.get('simulation_results'))
        })
        
        return state
    
    def _generate_code_with_llm(self, hypothesis: str, method: Dict, literature: list, domain: str = "") -> Dict[str, Any]:
        """Generate code using LLM with dynamic domain knowledge injection."""
        
        # Get domain-specific knowledge
        domain_injection = ""
        if domain:
            domain_injection = domain_store.get_prompt_injection(domain, hypothesis)
            logger.info(f"Injected domain knowledge for: {domain}", source="engineer")
        
        prompt = f"""Hypothesis: {hypothesis}

Selected Method: {method.get('title', 'Unknown')}
Methodology: {method.get('methodology', '')}
{domain_injection}

Generate Python code to test this hypothesis using the specified methodology."""
        
        messages = [
            SystemMessage(content=ENGINEER_PROMPT),
            HumanMessage(content=prompt)
        ]
        
        response = self.llm.invoke(messages)
        
        try:
            content = response.content
            if '```json' in content:
                content = content.split('```json')[1].split('```')[0]
            return json.loads(content.strip())
        except:
            # Extract code block if JSON fails
            if '```python' in response.content:
                code = response.content.split('```python')[1].split('```')[0]
                return {"code": code, "description": "Generated code"}
            return {"code": "", "description": "Code generation failed"}
    
    def _generate_mock_code(self, hypothesis: str, method: Dict) -> Dict[str, Any]:
        """Generate mock experiment code."""
        method_type = method.get('type', 'simulation')
        
        if method_type == 'simulation':
            code = '''
import numpy as np
from scipy import stats

# Experiment Parameters
n_samples = 100
n_iterations = 1000

# Hypothesis: Effect exists
control_mean = 50.0
control_std = 10.0
effect_size = 0.5  # Cohen's d
experimental_mean = control_mean + effect_size * control_std
experimental_std = control_std

# Monte Carlo Simulation
significant_count = 0
results = []
for _ in range(n_iterations):
    control = np.random.normal(control_mean, control_std, n_samples)
    experimental = np.random.normal(experimental_mean, experimental_std, n_samples)
    _, p_value = stats.ttest_ind(control, experimental)
    if p_value < 0.05:
        significant_count += 1
    results.append(np.mean(experimental) - np.mean(control))

power = significant_count / n_iterations
print(f"Statistical Power: {power:.2%}")
print(f"Effect Size (Cohen's d): {effect_size}")

# Visualization
import matplotlib.pyplot as plt
import seaborn as sns

plt.figure(figsize=(10, 6))
# Create dummy data for visualization
plt.hist(np.random.normal(control_mean, control_std, 1000), alpha=0.5, label='Control', color='blue')
plt.hist(np.random.normal(experimental_mean, experimental_std, 1000), alpha=0.5, label='Experimental', color='red')
plt.title("Control vs Experimental Group Distribution (Simulation)")
plt.xlabel("Value")
plt.ylabel("Frequency")
plt.legend()
plt.grid(True, alpha=0.3)

# Save plot
session_id = "{SESSION_ID}"
if session_id and session_id != "test":
    plt.savefig(f"static/{session_id}_result.png")
    print(f"Plot saved to static/{session_id}_result.png")
else:
    # Fallback if session_id not injected properly (though _execute_code does inject it)
    plt.savefig(f"static/{SESSION_ID}_result.png") 
plt.close()
'''
        else:
            code = '''
# Analytical approach
print("Analytical method placeholder")
print("Result: Hypothesis requires further investigation")
# Placeholder visual
import matplotlib.pyplot as plt
plt.figure()
plt.text(0.5, 0.5, "Analytical Result", ha='center')
plt.savefig(f"static/{SESSION_ID}_result.png")
plt.close()
'''
        
        return {"code": code, "description": "Mock experiment code"}
    
    def _execute_code(self, code: str, session_id: str = "test") -> Dict[str, Any]:
        """Execute Python code safely and extract SIMULATION_RESULTS."""
        if not code:
            return {"output": "", "error": "No code to execute", "success": False}
        
        # Capture stdout
        old_stdout = sys.stdout
        sys.stdout = StringIO()
        
        try:
            # Headless Plotting Setup
            import matplotlib
            matplotlib.use('Agg')
            import matplotlib.pyplot as plt
            
            # Restricted globals for safety
            restricted_globals = {
                "SESSION_ID": session_id,
                "__builtins__": {
                    "print": print,
                    "range": range,
                    "len": len,
                    "sum": sum,
                    "min": min,
                    "max": max,
                    "abs": abs,
                    "round": round,
                    "float": float,
                    "int": int,
                    "str": str,
                    "list": list,
                    "dict": dict,
                    "tuple": tuple,
                    "enumerate": enumerate,
                    "zip": zip,
                    "sorted": sorted,
                    "bool": bool,
                    "True": True,
                    "False": False,
                    "None": None,
                    "__import__": __import__,
                }
            }
            
            # Allow common imports
            import numpy as np
            from scipy import stats
            from scipy import integrate
            import pandas as pd
            
            restricted_globals['np'] = np
            restricted_globals['numpy'] = np
            restricted_globals['stats'] = stats
            restricted_globals['integrate'] = integrate
            restricted_globals['pd'] = pd
            restricted_globals['pandas'] = pd
            restricted_globals['plt'] = plt
            restricted_globals['matplotlib'] = matplotlib
            
            # Local variables namespace for capturing SIMULATION_RESULTS
            local_vars = {}
            exec(code, restricted_globals, local_vars)
            
            # Safety Net: Force save plot if figure exists
            if plt.get_fignums():
                try:
                    save_path = f"static/{session_id}_result.png"
                    plt.savefig(save_path)
                    logger.info(f"Auto-saved plot to {save_path}", source="engineer")
                except Exception as e:
                    logger.warning(f"Failed to auto-save plot: {e}", source="engineer")
            
            output = sys.stdout.getvalue()
            
            # Extract SIMULATION_RESULTS if defined
            simulation_results = local_vars.get('SIMULATION_RESULTS', None)
            if simulation_results is None:
                # Also check in restricted_globals (in case code defined it there)
                simulation_results = restricted_globals.get('SIMULATION_RESULTS', None)
            
            result = {
                "output": output, 
                "error": None, 
                "success": True
            }
            
            if simulation_results:
                result["simulation_results"] = simulation_results
                logger.info(f"Extracted SIMULATION_RESULTS: {simulation_results}", source="engineer")
            
            return result
            
        except Exception as e:
            return {"output": sys.stdout.getvalue(), "error": str(e), "success": False}
        finally:
            sys.stdout = old_stdout


def execute_experiment(state: ScientificState) -> ScientificState:
    """Entry point for engineer agent."""
    agent = EngineerAgent()
    return agent.execute(state)
