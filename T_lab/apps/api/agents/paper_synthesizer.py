# =============================================================================
# T_lab Agent - Paper Synthesizer (Multi-Experiment Merge)
# From v_lab - combines multiple experiments into review paper
# =============================================================================

from datetime import datetime
from typing import Any, List, Dict

try:
    from openai import OpenAI
    OPENAI_AVAILABLE = True
except ImportError:
    OPENAI_AVAILABLE = False

from core import get_logger, get_settings

logger = get_logger("agents.paper_synthesizer")
settings = get_settings()

client = None
if OPENAI_AVAILABLE and settings.openai_api_key:
    client = OpenAI(api_key=settings.openai_api_key)


SYNTHESIS_PROMPT = """당신은 국제 학술지에 게재 가능한 수준의 종합 리뷰 논문을 작성하는 과학 편집자입니다.

## 📝 논문 형식 (학술지 스타일 - 한글)

여러 실험 결과를 종합하여 **고품질 학술 리뷰 논문**을 작성하세요.

### 필수 섹션:

**제목 (Title)**
- 모든 실험을 아우르는 학술적 제목
- 예: "무선 통신 시스템의 에너지 효율성과 채널 용량 최적화에 관한 통합 연구"

**초록 (Abstract)** (200-300 단어)
- 연구 배경, 목적, 통합 방법론, 주요 발견, 종합 결론
- **학술적 구조**: 배경(Background) → 목적(Objective) → 방법(Methods) → 결과(Results) → 결론(Conclusion)

**핵심어 (Keywords)**
- 5-7개의 핵심 키워드 (한글, 영문 병기)

---

**1. 서론 (Introduction)**
- 연구 분야의 전반적 배경
- 각 실험의 연구 목적 및 가설 소개
- 본 통합 분석의 의의 및 목적

**2. 연구 방법론 (Methodology)**
- 각 실험에서 사용된 연구 설계 요약
- **필수 포함**: 각 실험의 구체적인 수학적 모델, 수식, 또는 시뮬레이션 파라미터(표본 크기, 반복 횟수 등)를 명시
- 데이터 기반 분석의 경우 독립/종속 변수 및 회귀 모델 설명
- 통계 분석 방법 (몬테카를로 시뮬레이션, t-검정, 회귀 분석 등)

**3. 통합 결과 (Integrated Results)**
- 각 실험의 주요 결과 요약 (표 형식 포함)
- **상세 수치 포함**: 
  - 이론적 분석: 이론적 예측값 및 신뢰도
  - 시뮬레이션: 대조군/실험군 평균, 표준편차, 효과 크기
  - 데이터 분석: 회귀 계수(Slope), 결정계수($R^2$)
- 통계 수치 포함: $p = 0.001$, Cohen's $d = 1.5$ 형식
- 결과 간의 상관관계 또는 패턴

**4. 종합 고찰 (General Discussion)**
- 결과의 이론적/실용적 의미
- 실험 간의 일관성 또는 모순점 분석
- 기존 문헌과의 비교
- 연구의 제한점

**5. 결론 및 향후 연구 (Conclusion & Future Work)**
- 핵심 발견 요약
- 학문적/실용적 함의
- 향후 연구 방향 제언

---

## ⚠️ 작성 규칙

1. **학술적 문체**: 3인칭 수동형 사용 ("본 연구에서는...이 관찰되었다")
2. **객관적 서술**: 감정적 표현 배제, 데이터 기반 서술
3. **표 활용**: 실험 결과 비교 시 Markdown 표 사용
4. **수식 포함**: 핵심 통계량은 LaTeX 스타일로 표기
5. **한글 작성**: 모든 내용을 학술적인 한글로 작성
6. **참고문헌 제외**: References 섹션은 생성하지 마세요 (자동 추가됨)

## 📊 표 형식 예시

| 실험 | 가설 | p-value | 효과 크기 | 유의성 |
|------|------|---------|----------|--------|
| 실험 1 | MIMO 용량 증가 | 0.001 | d=1.52 | ✅ 유의미 |
| 실험 2 | ZIGBEE 전력 효율 | 0.003 | d=0.89 | ✅ 유의미 |

반드시 마크다운(Markdown) 형식으로 출력하세요.
"""


def synthesize_paper(sessions: List[Dict[str, Any]]) -> Dict[str, Any]:
    """
    Synthesize a review paper from multiple research sessions.
    """
    if not sessions:
        return {"error": "No sessions provided"}

    # Extract relevant data
    summaries = []
    all_refs = set()
    
    for i, session in enumerate(sessions, 1):
        hypothesis = session.get("user_input", "")
        report = session.get("final_report", "")
        sim = session.get("simulation_results", {})
        sim_params = session.get("simulation_params", {})
        method = session.get("selected_method", {})
        
        method_type = sim_params.get("method_type", "simulation")
        
        # Build method-specific details
        details = ""
        if method_type == "analytical":
            details = f"""- 분석 유형: Analytical (이론적 분석)
- 모델: {sim_params.get('model_name', 'N/A')}
- 수식: {sim_params.get('formula', 'N/A')}
- 이론적 예측: {sim.get('theoretical_prediction', 'N/A')}
- 신뢰도: {sim.get('confidence', 'N/A')}"""
        elif method_type == "data_driven":
            details = f"""- 분석 유형: Data-driven (데이터 기반 분석)
- 독립변수: {sim_params.get('independent_var', 'X')}
- 종속변수: {sim_params.get('dependent_var', 'Y')}
- 회귀 계수: {sim.get('regression_coefficient', 'N/A')}
- 결정계수 (R²): {sim.get('r_squared', 'N/A')}
- 결론: {sim.get('conclusion', 'N/A')}"""
        else:  # simulation
            details = f"""- 분석 유형: Simulation (몬테카를로 시뮬레이션)
- 대조군: mean={sim_params.get('control_group_mean', 'N/A')}, std={sim_params.get('control_group_std', 'N/A')}
- 실험군: mean={sim_params.get('experimental_group_mean', 'N/A')}, std={sim_params.get('experimental_group_std', 'N/A')}
- 효과 크기: {sim_params.get('effect_size', 'N/A')}
- 반복 횟수: {sim_params.get('sample_size', '10000')}"""

        summary_text = f"""### 실험 {i}
- 가설: {hypothesis}
- 방법론: {method.get('title', 'N/A')}
{details}
- p-value: {sim.get('p_value', 'N/A')}
- 유의성: {'유의미함' if sim.get('significant_difference', False) else '유의미하지 않음'}
"""
        summaries.append(summary_text)
        
        for citation in session.get("verified_citations", []):
            all_refs.add(citation.get("id", ""))

    context = "\n".join(summaries)
    
    # Use LLM if available
    paper_content = ""
    if client:
        try:
            completion = client.chat.completions.create(
                model="gpt-4o",
                messages=[
                    {
                        "role": "system",
                        "content": SYNTHESIS_PROMPT
                    },
                    {
                        "role": "user",
                        "content": f"다음 {len(sessions)}개의 실험 결과를 종합하여 학술 리뷰 논문을 작성하세요:\n\n{context}"
                    }
                ],
                temperature=0.7
            )
            paper_content = completion.choices[0].message.content
        except Exception as e:
            logger.error(f"Paper synthesis with LLM failed: {e}", source="paper_synthesizer")
            paper_content = _get_fallback_paper(sessions)
    else:
        paper_content = _get_fallback_paper(sessions)

    # Append references
    if all_refs:
        paper_content += "\n\n## 참고문헌 (References)\n"
        for ref in sorted(list(all_refs)):
            if ref and "10." in ref:
                paper_content += f"- [DOI: {ref}](https://doi.org/{ref})\n"
            elif ref:
                paper_content += f"- [arXiv: {ref}](https://arxiv.org/abs/{ref})\n"

    # Generate Korean title
    hypotheses = [s.get("user_input", "")[:30] for s in sessions[:2]]
    title = f"{len(sessions)}개 연구의 통합 분석: {' 및 '.join(hypotheses)}..."
    
    return {
        "title": title,
        "content": paper_content,
        "created_at": datetime.now().strftime("%Y-%m-%d"),
        "source_sessions": [s.get("session_id", "") for s in sessions]
    }


def _get_fallback_paper(sessions: List[Dict[str, Any]]) -> str:
    """Fallback paper generation without LLM."""
    lines = [
        f"# {len(sessions)}개 연구 세션의 통합 분석",
        f"",
        f"**작성일:** {datetime.now().strftime('%Y년 %m월 %d일')}",
        f"",
        f"---",
        f"",
        f"## 초록 (Abstract)",
        f"",
        f"본 논문은 {len(sessions)}개의 가상 실험 결과를 종합하여 분석한 통합 리뷰이다. "
        f"각 실험은 몬테카를로 시뮬레이션을 통해 가설을 검증하였으며, "
        f"본 분석에서는 실험 간의 공통점과 차이점을 비교하고 종합적인 결론을 도출하였다.",
        f"",
        f"**핵심어:** 가상 실험, 몬테카를로 시뮬레이션, 가설 검정, 통합 분석",
        f"",
        f"---",
        f"",
        f"## 1. 서론 (Introduction)",
        f"",
        f"본 연구는 다양한 가설에 대한 시뮬레이션 기반 검증 결과를 통합하여 "
        f"종합적인 결론을 도출하는 것을 목적으로 한다. "
        f"총 {len(sessions)}개의 독립적인 실험이 분석에 포함되었다.",
        f"",
        f"## 2. 연구 방법론 (Methodology)",
        f"",
        f"각 실험은 다음과 같은 공통된 방법론을 사용하였다:",
        f"- 몬테카를로 시뮬레이션 기반 가설 검정",
        f"- 유의수준 α = 0.05",
        f"- Cohen's d를 통한 효과 크기 측정",
        f"",
        f"## 3. 통합 결과 (Integrated Results)",
        f"",
        f"| 실험 | 가설 | p-value | 유의성 |",
        f"|:----:|------|:-------:|:------:|",
    ]
    
    for i, session in enumerate(sessions, 1):
        hypothesis = session.get("user_input", "Unknown")[:50]
        sim = session.get("simulation_results", {})
        p_value = sim.get("p_value", "N/A")
        significant = sim.get("significant_difference", False)
        icon = "✅ 유의미" if significant else "❌ 비유의미"
        
        p_str = f"{p_value:.4f}" if isinstance(p_value, (int, float)) else str(p_value)
        lines.append(f"| {i} | {hypothesis}... | {p_str} | {icon} |")
    
    lines.extend([
        f"",
        f"## 4. 종합 고찰 (General Discussion)",
        f"",
        f"본 통합 분석 결과, 검증된 실험들은 각기 다른 수준의 통계적 유의성을 보였다. "
        f"유의미한 결과를 보인 실험들은 해당 분야의 이론적 예측과 일치하는 경향을 나타내었으며, "
        f"비유의미한 결과는 표본 크기 또는 효과 크기의 제한에 기인할 수 있다.",
        f"",
        f"## 5. 결론 및 향후 연구 (Conclusion)",
        f"",
        f"본 통합 분석을 통해 다음과 같은 결론을 도출하였다:",
        f"1. 대부분의 가설은 시뮬레이션을 통해 검증 가능하였다.",
        f"2. 유의미한 결과를 보인 가설은 추가적인 실증 연구가 권장된다.",
        f"3. 향후 연구에서는 다양한 환경 조건에서의 반복 실험이 필요하다."
    ])
    
    return "\n".join(lines)
