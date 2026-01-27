"""
Engineer Agent - 실험 코드 생성 및 실행
샌드박스 환경에서 가상 실험 수행
"""
from typing import Dict, Any, List
import json
import os

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from config import MAX_RETRY_COUNT
from tools.code_executor import execute_code


ENGINEER_CODE_PROMPT = """당신은 과학 실험을 위한 Python 코드를 작성하는 전문가입니다.

다음 실험 방법론을 구현하는 Python 코드를 작성하세요.

가설: {hypothesis}
도메인: {domain}
선택된 방법론: {methodology}

요구사항:
1. 완전하고 실행 가능한 Python 코드를 작성하세요
2. 필요한 import문을 모두 포함하세요
3. 결과를 출력하고 가능하면 시각화하세요 (matplotlib)
4. 결과 그래프는 'results.png'로 저장하세요
5. 주요 결과는 'results.json'으로 저장하세요

코드만 반환하세요 (마크다운 코드 블록 없이):
"""


ENGINEER_DEBUG_PROMPT = """다음 Python 코드에서 에러가 발생했습니다.

원본 코드:
```python
{original_code}
```

에러 메시지:
{error_message}

에러를 수정한 완전한 코드를 반환하세요 (마크다운 코드 블록 없이):
"""


class EngineerAgent:
    """엔지니어 에이전트 - 코드 생성 및 실험 실행"""
    
    def __init__(self, api_key: str = None):
        self.api_key = api_key or os.getenv("OPENAI_API_KEY", "")
        self.llm = None
        if LANGCHAIN_AVAILABLE and self.api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.2,  # 코드는 deterministic 하게
                api_key=self.api_key
            )
    
    def run_experiment(self, state: ScientificState) -> ScientificState:
        """실험 코드 생성 및 실행"""
        hypothesis = state.get('user_input', '')
        domain = state.get('domain', '')
        methods = state.get('proposed_methods', [])
        selected_idx = state.get('selected_method_index', 0)
        
        if not methods:
            state['errors'].append("방법론이 제안되지 않았습니다.")
            state['experiment_success'] = False
            return state
        
        # 선택된 방법론
        methodology = methods[selected_idx] if selected_idx < len(methods) else methods[0]
        
        # 코드 생성
        code = self._generate_code(hypothesis, domain, methodology)
        state['code_repository'] = {'experiment.py': code}
        
        # 코드 실행 (자가 치유 루프)
        success, logs, figures = self._execute_with_retry(code, state)
        
        state['execution_logs'] = logs
        state['experiment_success'] = success
        state['figures'] = figures
        state['current_step'] = 'experiment_completed'
        
        return state
    
    def _generate_code(self, hypothesis: str, domain: str, methodology: Dict) -> str:
        """실험 코드 생성"""
        if self.llm:
            return self._generate_code_llm(hypothesis, domain, methodology)
        else:
            return self._generate_code_mock(methodology)
    
    def _generate_code_llm(self, hypothesis: str, domain: str, methodology: Dict) -> str:
        """LLM을 이용한 코드 생성"""
        prompt = ENGINEER_CODE_PROMPT.format(
            hypothesis=hypothesis,
            domain=domain or "과학",
            methodology=json.dumps(methodology, ensure_ascii=False, indent=2)
        )
        response = self.llm.invoke([HumanMessage(content=prompt)])
        
        code = response.content.strip()
        # 마크다운 코드 블록 제거
        if code.startswith('```python'):
            code = code[9:]
        if code.startswith('```'):
            code = code[3:]
        if code.endswith('```'):
            code = code[:-3]
        
        return code.strip()
    
    def _generate_code_mock(self, methodology: Dict) -> str:
        """Mock 코드 생성"""
        method_type = methodology.get('type', 'simulation')
        
        if method_type == 'analytical':
            return self._mock_analytical_code()
        elif method_type == 'data_driven':
            return self._mock_ml_code()
        else:
            return self._mock_simulation_code()
    
    def _mock_simulation_code(self) -> str:
        """시뮬레이션 Mock 코드"""
        return '''"""
수치 시뮬레이션: 가설 검증을 위한 미분방정식 모델
"""
import numpy as np
import matplotlib.pyplot as plt
from scipy.integrate import odeint
import json

# 모델 파라미터
k1, k2 = 0.5, 0.3
initial_state = [10, 0]
t = np.linspace(0, 20, 200)

# 미분방정식 정의
def model(y, t, k1, k2):
    A, B = y
    dAdt = -k1 * A
    dBdt = k1 * A - k2 * B
    return [dAdt, dBdt]

# 시뮬레이션 실행
solution = odeint(model, initial_state, t, args=(k1, k2))

# 결과 시각화
plt.figure(figsize=(10, 6))
plt.plot(t, solution[:, 0], 'b-', label='물질 A', linewidth=2)
plt.plot(t, solution[:, 1], 'r--', label='물질 B', linewidth=2)
plt.xlabel('시간')
plt.ylabel('농도')
plt.title('가설 검증: 반응 시뮬레이션 결과')
plt.legend()
plt.grid(True, alpha=0.3)
plt.savefig('results.png', dpi=150, bbox_inches='tight')
plt.close()

# 결과 저장
results = {
    "simulation_type": "ODE simulation",
    "final_A": float(solution[-1, 0]),
    "final_B": float(solution[-1, 1]),
    "max_B": float(max(solution[:, 1])),
    "conclusion": "가설이 시뮬레이션 결과에 의해 지지됨"
}
with open('results.json', 'w') as f:
    json.dump(results, f, ensure_ascii=False, indent=2)

print("=== 시뮬레이션 완료 ===")
print(f"최종 A 농도: {results['final_A']:.4f}")
print(f"최종 B 농도: {results['final_B']:.4f}")
print(f"B 최대값: {results['max_B']:.4f}")
print("결론:", results['conclusion'])
'''
    
    def _mock_analytical_code(self) -> str:
        """분석적 접근 Mock 코드"""
        return '''"""
통계적 메타 분석: 기존 데이터 기반 가설 검증
"""
import numpy as np
import matplotlib.pyplot as plt
from scipy import stats
import json

# 합성 데이터 생성 (실제로는 문헌에서 추출)
np.random.seed(42)
n_studies = 10
effect_sizes = np.random.normal(0.5, 0.2, n_studies)
std_errors = np.random.uniform(0.1, 0.3, n_studies)

# 가중 평균 계산 (역분산 가중치)
weights = 1 / (std_errors ** 2)
weighted_mean = np.sum(effect_sizes * weights) / np.sum(weights)
pooled_se = np.sqrt(1 / np.sum(weights))

# 통계 검정
z_score = weighted_mean / pooled_se
p_value = 2 * (1 - stats.norm.cdf(abs(z_score)))

# Forest Plot
plt.figure(figsize=(10, 8))
y_pos = np.arange(n_studies)
plt.errorbar(effect_sizes, y_pos, xerr=std_errors*1.96, fmt='o', color='blue', capsize=3)
plt.axvline(x=weighted_mean, color='red', linestyle='--', label=f'통합 효과 크기: {weighted_mean:.3f}')
plt.axvline(x=0, color='gray', linestyle='-', alpha=0.5)
plt.fill_betweenx(y_pos, weighted_mean-pooled_se*1.96, weighted_mean+pooled_se*1.96, alpha=0.2, color='red')
plt.xlabel('효과 크기')
plt.ylabel('연구 번호')
plt.title('메타 분석 Forest Plot')
plt.legend()
plt.savefig('results.png', dpi=150, bbox_inches='tight')
plt.close()

# 결과 저장
results = {
    "analysis_type": "Meta-analysis",
    "n_studies": n_studies,
    "pooled_effect_size": float(weighted_mean),
    "95_CI": [float(weighted_mean - 1.96*pooled_se), float(weighted_mean + 1.96*pooled_se)],
    "z_score": float(z_score),
    "p_value": float(p_value),
    "significant": p_value < 0.05,
    "conclusion": "가설 지지됨" if (weighted_mean > 0 and p_value < 0.05) else "추가 연구 필요"
}
with open('results.json', 'w') as f:
    json.dump(results, f, ensure_ascii=False, indent=2)

print("=== 메타 분석 완료 ===")
print(f"통합 효과 크기: {weighted_mean:.4f} (95% CI: [{results['95_CI'][0]:.4f}, {results['95_CI'][1]:.4f}])")
print(f"Z-score: {z_score:.4f}, p-value: {p_value:.4f}")
print("결론:", results['conclusion'])
'''
    
    def _mock_ml_code(self) -> str:
        """ML 기반 Mock 코드"""
        return '''"""
머신러닝 기반 예측: 합성 데이터로 가설 검증
"""
import numpy as np
import matplotlib.pyplot as plt
from sklearn.ensemble import RandomForestClassifier
from sklearn.model_selection import cross_val_score
import json

# 합성 데이터 생성
np.random.seed(42)
n_samples = 500

# 특성 생성 (가설 관련 변수)
X1 = np.random.normal(0, 1, n_samples)  # 독립변수 1
X2 = np.random.normal(0, 1, n_samples)  # 독립변수 2
X3 = np.random.normal(0, 1, n_samples)  # 독립변수 3

# 목표변수 (가설: X1이 결과에 영향을 미침)
y = (X1 * 0.6 + X2 * 0.3 + np.random.normal(0, 0.5, n_samples) > 0).astype(int)

X = np.column_stack([X1, X2, X3])
feature_names = ['가설 변수', '통제 변수 1', '통제 변수 2']

# 모델 학습
model = RandomForestClassifier(n_estimators=100, random_state=42)
cv_scores = cross_val_score(model, X, y, cv=5)
model.fit(X, y)

# 특성 중요도
importances = model.feature_importances_

# 시각화
fig, axes = plt.subplots(1, 2, figsize=(12, 5))

# 특성 중요도 플롯
axes[0].barh(feature_names, importances, color=['#e74c3c', '#3498db', '#2ecc71'])
axes[0].set_xlabel('중요도')
axes[0].set_title('특성 중요도 분석')

# CV 점수 플롯
axes[1].bar(range(1, 6), cv_scores, color='#9b59b6', alpha=0.7)
axes[1].axhline(y=np.mean(cv_scores), color='red', linestyle='--', label=f'평균: {np.mean(cv_scores):.3f}')
axes[1].set_xlabel('Fold')
axes[1].set_ylabel('정확도')
axes[1].set_title('교차 검증 결과')
axes[1].legend()

plt.tight_layout()
plt.savefig('results.png', dpi=150, bbox_inches='tight')
plt.close()

# 결과 저장
results = {
    "analysis_type": "Random Forest Classification",
    "n_samples": n_samples,
    "cv_accuracy_mean": float(np.mean(cv_scores)),
    "cv_accuracy_std": float(np.std(cv_scores)),
    "feature_importances": {name: float(imp) for name, imp in zip(feature_names, importances)},
    "hypothesis_variable_importance": float(importances[0]),
    "conclusion": "가설 변수가 가장 중요한 예측 인자로 확인됨" if importances[0] == max(importances) else "다른 요인이 더 중요할 수 있음"
}
with open('results.json', 'w') as f:
    json.dump(results, f, ensure_ascii=False, indent=2)

print("=== ML 분석 완료 ===")
print(f"교차 검증 정확도: {np.mean(cv_scores):.4f} ± {np.std(cv_scores):.4f}")
print(f"가설 변수 중요도: {importances[0]:.4f}")
print("결론:", results['conclusion'])
'''
    
    def _execute_with_retry(self, code: str, state: ScientificState) -> tuple:
        """자가 치유 디버깅 루프"""
        current_code = code
        all_logs = []
        figures = []
        
        for attempt in range(MAX_RETRY_COUNT):
            state['retry_count'] = attempt + 1
            
            success, stdout, stderr, generated_files = execute_code(current_code)
            all_logs.append(f"=== 시도 {attempt + 1} ===\n{stdout}\n{stderr}")
            
            if success:
                figures = [f for f in generated_files if f.endswith('.png')]
                return True, "\n".join(all_logs), figures
            
            # 에러 발생 시 수정 시도
            if self.llm and stderr:
                current_code = self._fix_code(current_code, stderr)
                state['code_repository']['experiment.py'] = current_code
            else:
                break
        
        return False, "\n".join(all_logs), figures
    
    def _fix_code(self, original_code: str, error_message: str) -> str:
        """LLM을 이용한 코드 수정"""
        prompt = ENGINEER_DEBUG_PROMPT.format(
            original_code=original_code,
            error_message=error_message[:500]
        )
        response = self.llm.invoke([HumanMessage(content=prompt)])
        
        code = response.content.strip()
        if code.startswith('```'):
            code = code.split('\n', 1)[1] if '\n' in code else code[3:]
        if code.endswith('```'):
            code = code[:-3]
        
        return code.strip()


def engineer_node(state: ScientificState) -> ScientificState:
    """LangGraph 노드 함수"""
    agent = EngineerAgent()
    return agent.run_experiment(state)
