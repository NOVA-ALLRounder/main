# =============================================================================
# T_lab Agent - Author (Report Writing)
# IMRAD format report generation
# =============================================================================

from typing import Dict, Any
from datetime import datetime
import json

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from core import get_settings, get_logger

settings = get_settings()
logger = get_logger("agents.author")


AUTHOR_PROMPT = """당신은 국제 학술지에 게재 가능한 수준의 연구 논문을 작성하는 과학 저술가입니다.

## 📝 논문 형식 (학술지 스타일)

제공된 가설, 방법론, 실험 결과를 바탕으로 **학술 논문 형식**의 보고서를 작성하세요.

### 필수 섹션:

**TITLE (제목)**
- 연구 내용을 명확히 반영하는 학술적 제목

**ABSTRACT (초록)** (150-200 단어)
- 연구 배경, 목적, 방법, 주요 결과, 결론을 간결하게 요약

**KEYWORDS (핵심어)**
- 5개 이내의 핵심 키워드 (한글, 영문 병기)

---

**1. 서론 (Introduction)**
- 연구 배경 및 이론적 근거
- 선행연구 검토
- 연구 목적 및 가설 진술

**2. 연구 방법 (Methods)**
- 연구 설계 및 접근법 (제공된 방법론 유형에 따라 작성)
- 실험 조건 및 파라미터 (표로 정리)
- 분석 방법 명시

**3. 결과 (Results)**
- 주요 발견 사항 (제공된 실험 결과 데이터 사용)
- 통계 수치 포함 (p-value, 효과 크기 등)
- 시각화 포함

**4. 고찰 (Discussion)**
- 결과 해석 및 이론적 의미
- 선행연구와의 비교 분석
- 연구의 제한점 및 향후 연구 방향

**5. 결론 (Conclusion)**
- 연구 결과 요약
- 학문적/실용적 함의

**REFERENCES (참고문헌)**
- APA 스타일 형식

---

## 🔬 방법론별 "연구 방법" 및 "결과" 내용 작성 가이드

형식은 동일하게 유지하되, 제공된 방법론 유형에 따라 **내용**을 다르게 작성하세요:

### Analytical (이론적 분석)
- **연구 방법**: 사용된 수학적 모델, 공식, 변수 정의, 이론적 분석 절차
- **결과**: 이론적 계산 결과, 수학적 예측값, 신뢰도 기반 결론

### Simulation (시뮬레이션)
- **연구 방법**: 몬테카를로 시뮬레이션, 대조군/실험군 설계, 파라미터 설정, t-검정
- **결과**: 대조군 vs 실험군 비교(μ, σ), p-value, Cohen's d, 유의성

### Data-driven (데이터 기반 분석)
- **연구 방법**: 데이터셋 구성, 독립/종속변수, 회귀 분석(OLS)
- **결과**: 회귀 계수(Slope), R², p-value, 변수 간 관계 해석

---

## ⚠️ 작성 규칙

1. **학술적 문체**: 3인칭 수동형 사용
2. **객관적 서술**: 감정적 표현 배제
3. **수식 포함**: LaTeX 스타일로 표기
4. **통계 보고**: $p < 0.05$, $R^2 = 0.85$ 형식

## 🇰🇷 언어 규칙 (중요!)
- **반드시 모든 내용을 한글로 작성하세요.**
- 전문 용어는 한글(영문) 형식으로 병기할 수 있습니다.
- 영어로 작성하지 마세요.

반드시 마크다운(Markdown) 형식으로 한글 출력하세요.
"""



class AuthorAgent:
    """Author agent for IMRAD report generation."""
    
    def __init__(self):
        self.llm = None
        if LANGCHAIN_AVAILABLE and settings.openai_api_key:
            self.llm = ChatOpenAI(
                model=settings.default_model,
                temperature=0.7,
                api_key=settings.openai_api_key
            )
            
    def write(self, state: ScientificState) -> ScientificState:
        """Write research report."""
        hypothesis = state.get('user_input', '')
        method = state.get('selected_method', {})
        simulation = state.get('simulation_results', {})
        simulation_params = state.get('simulation_params', {})
        experiment = state.get('experiment_results', {})
        literature = state.get('literature_context', [])
        session_id = state.get('session_id', 'unknown')
        
        logger.info("Writing report", source="author")
        
        if self.llm:
            report = self._write_with_llm(hypothesis, method, simulation, simulation_params, experiment, literature, session_id)
        else:
            report = self._write_mock_report(hypothesis, method, simulation, literature, state)
        
        state['draft_report'] = report
        state['final_report'] = report  # Will be refined by critic
        state['current_step'] = 'report_written'
        
        # Save report
        report_path = self._save_report(session_id, report)
        state['report_path'] = report_path
        
        # Add to logic chain
        state['logic_chain'] = state.get('logic_chain', [])
        state['logic_chain'].append({
            "step": "author",
            "report_length": len(report),
            "report_saved": bool(report_path)
        })
        
        return state
    
    def _write_with_llm(self, hypothesis: str, method: Dict, simulation: Dict,
                         simulation_params: Dict, experiment: Dict, literature: list, session_id: str) -> str:
        """Write report using LLM."""
        img_url = f"http://localhost:8000/static/{session_id}_result.png"
        
        # Check literature support status
        literature_supports = simulation_params.get('literature_supports', True)
        contradiction_reason = simulation_params.get('contradiction_reason', None)
        
        literature_warning = ""
        if not literature_supports and contradiction_reason:
            literature_warning = f"""
⚠️ LITERATURE WARNING:
- literature_supports: false
- contradiction_reason: {contradiction_reason}

You MUST include warnings about this in the report as per the system prompt instructions.
"""

        # Get method type
        method_type = method.get('type', 'simulation')
        
        # Build method-specific context
        method_context = ""
        if method_type == 'analytical':
            method_context = f"""
📐 방법론 유형: Analytical (이론적 분석)
- 수학적 모델: {simulation.get('model_name', 'Unknown')}
- 공식: {simulation.get('formula', 'N/A')}
- 이론적 예측: {simulation.get('theoretical_prediction', 'N/A')}
- 신뢰도: {simulation.get('confidence', 'N/A')}
- 과학적 근거: {simulation.get('scientific_basis', '')}
"""
        elif method_type == 'data_driven':
            method_context = f"""
📈 방법론 유형: Data-driven (데이터 기반 분석)
- 독립변수: {simulation.get('independent_var', 'X')}
- 종속변수: {simulation.get('dependent_var', 'Y')}
- 표본 크기: N={simulation.get('sample_size', simulation.get('n', 'N/A'))}
- 회귀 계수 (Slope): {simulation.get('regression_coefficient', 'N/A')}
- 결정계수 (R²): {simulation.get('r_squared', 'N/A')}
- p-value: {simulation.get('p_value', 'N/A')}
"""
        else:  # simulation
            method_context = f"""
🎲 방법론 유형: Simulation (몬테카를로 시뮬레이션)
- 대조군: μ={simulation_params.get('control_group_mean', 'N/A')}, σ={simulation_params.get('control_group_std', 'N/A')}
- 실험군: μ={simulation_params.get('experimental_group_mean', 'N/A')}, σ={simulation_params.get('experimental_group_std', 'N/A')}
- 효과 크기 (Cohen's d): {simulation_params.get('effect_size', 'N/A')}
- 표본 크기: N={simulation_params.get('sample_size', 'N/A')}
- p-value: {simulation.get('p_value', 'N/A')}
"""
        
        context = f"""
가설: {hypothesis}

방법론: {method.get('title', 'Unknown')} - {method.get('methodology', '')}

{method_context}

실험 결과:
- P-value: {simulation.get('p_value', 'N/A')}
- 유의미함: {simulation.get('significant_difference', 'N/A')}
{literature_warning}

코드 실행 결과:
{experiment.get('output', 'No output')}

관련 문헌:
{chr(10).join([f"- {p.get('title', '')}" for p in literature[:5]])}

지시사항: 반드시 '결과' 섹션에 다음 마크다운을 사용하여 시각화를 포함하세요:
![실험 결과]({img_url})
"""
        
        messages = [
            SystemMessage(content=AUTHOR_PROMPT),
            HumanMessage(content=context)
        ]
        
        response = self.llm.invoke(messages)
        return response.content
    
    def _write_mock_report(self, hypothesis: str, method: Dict, 
                           simulation: Dict, literature: list, state: ScientificState) -> str:
        """Generate mock IMRAD report."""
        p_value = simulation.get('p_value', 0.5)
        significant = simulation.get('significant_difference', False)
        control = simulation.get('control_stats', {})
        experimental = simulation.get('experimental_stats', {})
        
        result_text = "supports" if significant else "does not support"
        conclusion = "warrants further investigation" if significant else "requires alternative approaches"
        
        report = f"""# Research Report

**Date:** {datetime.now().strftime('%Y-%m-%d')}

## Abstract

This study investigated the hypothesis: "{hypothesis}"
Using {method.get('title', 'simulation')} methodology, we found that the evidence {result_text} the proposed hypothesis.

## 1. Introduction

The current study aims to evaluate the following hypothesis:

> {hypothesis}

Based on {len(literature)} related studies in the literature, we designed an experiment to test this claim.

## 2. Methods

### 2.1 Approach
{method.get('title', 'Monte Carlo Simulation')}

### 2.2 Methodology
{method.get('methodology', 'Statistical simulation with randomized parameters')}

### 2.3 Parameters
- Sample Size: {simulation.get('control_stats', {}).get('mean', 100)} per group
- Iterations: {simulation.get('iterations', 1000)}

## 3. Results

### 3.1 Statistical Analysis
| Group | Mean | Std Dev |
|-------|------|---------|
| Control | {control.get('mean', 50):.2f} | {control.get('std', 10):.2f} |
| Experimental | {experimental.get('mean', 55):.2f} | {experimental.get('std', 10):.2f} |

**P-value:** {p_value:.5f}
**Significance:** {'✅ Statistically significant (p < 0.05)' if significant else '❌ Not significant (p ≥ 0.05)'}

### 3.2 Visualization
![Experiment Result](http://localhost:8000/static/{state.get('session_id', 'unknown')}_result.png)

## 4. Discussion

The statistical analysis {result_text} the original hypothesis. 
{'The observed effect size suggests a meaningful difference between groups.' if significant else 'No meaningful difference was observed between the control and experimental groups.'}

### 4.1 Limitations
- This is a virtual experiment based on simulated data
- Real-world validation is recommended

## 5. Conclusion

Based on our analysis, the hypothesis "{hypothesis[:50]}..." {conclusion}.

## References

"""
        for i, paper in enumerate(literature[:5], 1):
            doi = paper.get('doi') or paper.get('arxiv_id') or 'N/A'
            report += f"{i}. {paper.get('title', 'Unknown')} ({paper.get('year', 'N/A')}) - {doi}\n"
        
        return report
    
    def _save_report(self, session_id: str, report: str) -> str:
        """Save report to file."""
        import os
        
        reports_dir = "reports"
        os.makedirs(reports_dir, exist_ok=True)
        
        filename = f"{reports_dir}/report_{session_id[:8]}_{datetime.now().strftime('%Y%m%d_%H%M%S')}.md"
        
        try:
            with open(filename, 'w', encoding='utf-8') as f:
                f.write(report)
            return filename
        except Exception as e:
            logger.warning(f"Failed to save report: {e}", source="author")
            return ""


def write_report(state: ScientificState) -> ScientificState:
    """Entry point for author agent."""
    agent = AuthorAgent()
    return agent.write(state)
