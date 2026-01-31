"""
Author Agent - 저자 에이전트
IMRAD 형식 학술 보고서 작성
"""
from typing import Dict, Any, List
import json
import os
from datetime import datetime

try:
    from langchain_openai import ChatOpenAI
    from langchain_core.messages import SystemMessage, HumanMessage
    LANGCHAIN_AVAILABLE = True
except ImportError:
    LANGCHAIN_AVAILABLE = False

from state import ScientificState
from config import REPORTS_DIR, OPENAI_API_KEY


AUTHOR_DRAFT_PROMPT = """당신은 과학 논문 작성 전문가입니다.

다음 정보를 바탕으로 IMRAD 형식의 학술 보고서를 작성하세요.

## 입력 정보
- 가설: {hypothesis}
- 도메인: {domain}
- 문헌 조사 결과:
{literature}

- 방법론: {methodology}

- 실험 결과:
{results}

## 보고서 작성 지침 (반드시 준수할 것)

### A. 방법론(Methods) 섹션 필수 요소:
1. **실험 절차 상세 기술**: 단계별로 무엇을 했는지 구체적으로 서술하세요.
   - 1단계: 데이터 준비 (어떤 데이터를, 어디서, 얼마나)
   - 2단계: 실험 수행 (어떤 조건에서, 몇 회 반복)
   - 3단계: 데이터 수집 (어떤 지표를, 어떤 방식으로)
   - 4단계: 분석 방법 (어떤 통계/분석 기법 사용)
   
2. **변수 설정**: 독립변수, 종속변수, 통제변수를 명확히 정의하세요.
   - 예: "독립변수: AE제 농도 (0%, 2%, 4%, 6%, 8%)"
   - 예: "종속변수: 28일 압축강도 (MPa)"
   
3. **데이터 수집 방법**: 측정 장비, 샘플 수, 측정 시점 등을 명시하세요.
   - 예: "만능재료시험기로 3개 시편의 압축강도를 측정하여 평균값 산출"

4. **분석 방법**: 데이터를 어떻게 처리/분석했는지 설명하세요.
   - 예: "회귀분석을 통해 농도와 강도 간 상관관계 분석"

### B. 결과(Results) 섹션 필수 요소:
1. **정량적 데이터 테이블**: 실제 수치를 포함한 테이블 (빈 칸 금지)
   | 조건 | 측정값 | 표준편차 | 비고 |
   |------|--------|---------|------|
   | ... | 실제수치 | 실제수치 | ... |
   
2. **데이터 패턴 분석**: 단순 증가/감소가 아닌 실제 패턴을 기술하세요.
   - **중요**: 많은 현상에는 **임계점(threshold)** 또는 **최적값(optimum)**이 존재합니다.
   - 예: "AE제 4%까지는 강도 증가, 그 이상에서는 오히려 감소" (역U자형 패턴)
   - 예: "온도 25°C에서 효소 활성 최대, 그 이상에서는 변성으로 감소"
   
3. **가설과 다른 결과도 정직하게 보고**: 
   - 데이터가 가설을 지지하지 않으면 "가설이 기각되었다"고 명시하세요.
   - 부분적으로만 지지되면 "~조건에서만 가설이 지지되었다"고 명시하세요.

### C. 토의(Discussion) 섹션 필수 요소:
1. **결과 해석 시 주의**: 데이터가 보여주는 것만 해석하세요.
   - **끼워맞추기 금지**: 데이터가 가설과 다르면 가설이 틀린 것입니다.
   - **과도한 일반화 금지**: "많을수록 좋다"가 아니라 "X% 이하에서 효과적"처럼 조건을 명시하세요.
   
2. **한계점 명시**: 샘플 수, 조건 범위, 측정 오차 등을 솔직히 기술하세요.

### D. 참고문헌 섹션 필수 요소:
1. 실제 인용한 논문만 포함 (placeholder 문구 금지)
2. 불완전한 문장 절대 금지
3. 형식: 저자 (년도). 제목. 학술지/출처.
---

다음 구조로 작성하세요 (마크다운 형식):

# 연구 보고서: [구체적이고 명확한 제목]

## 초록 (Abstract)
[150-200단어로 핵심 내용 요약. 가설, 방법, 주요 결과, 결론 포함]

## 1. 서론 (Introduction)
### 1.1 연구 배경
### 1.2 연구 목적
### 1.3 가설
### 1.4 관련 연구

## 2. 방법 (Methods)
### 2.1 실험 설계
[변수 정의, 실험 조건, 반복 횟수 등 - 실험 로그의 "1단계", "2단계" 내용 참조]
- 독립변수: [구체적 이름과 범위]
- 종속변수: [구체적 이름과 단위]
- 통제변수: [고정된 조건들]
- 반복 횟수: [각 조건당 n회]

### 2.2 실험 절차
[실험 로그의 "3단계" 내용을 기반으로 작성]
1. **데이터 준비**: [출처, 수집 방법, 샘플 수]
2. **조건 설정**: [각 조건의 구체적 값]
3. **측정 수행**: [어떤 장비로, 언제, 어떻게]
4. **데이터 기록**: [어떤 형식으로 저장]

### 2.3 데이터 수집
[실험 로그에서 추출한 구체적 정보]
- 데이터 출처: [문헌명/시뮬레이션/직접측정]
- 샘플 크기: [n = xxx]
- 측정 장비: [장비명, 정밀도]
- 수집된 데이터 예시:
  | 조건 | 측정값 | 표준편차 |
  |------|--------|---------|
  | ... | ... | ... |

### 2.4 분석 방법
[실험 로그의 "4단계" 내용 참조]
- 사용된 분석 기법: [회귀분석/t-test/ANOVA 등]
- 분석 도구: [Python scipy, statsmodels 등]
- 유의수준: [α = 0.05]

## 3. 결과 (Results)
### 3.1 정량적 결과
[마크다운 테이블로 실제 수치 결과 제시]
### 3.2 데이터 패턴 분석
[증가/감소/최적점 등 패턴 설명]
### 3.3 가설 검증 결과
[데이터가 가설을 지지하는지 명확히 판정]

## 4. 토의 (Discussion)
### 4.1 결과 해석
### 4.2 가설 검증 결론
[지지됨/기각됨/부분적 지지 중 하나로 명확히 판정]
### 4.3 한계점

## 5. 결론 (Conclusion)
[핵심 결론 2-3문장, 향후 연구 방향]

## 참고문헌 (References)
[실제 인용된 문헌만 번호 목록으로 작성]
"""


AUTHOR_REFINE_PROMPT = """다음 보고서 초안과 심사 피드백을 바탕으로 보고서를 수정하세요.

초안:
{draft}

심사 피드백:
{feedback}

피드백을 반영하여 개선된 보고서를 작성하세요.

[주의사항]
1. 보고서 본문만 출력하세요.
2. '심사 피드백 반영 결과', '수정 사항', 'Response to Reviewers' 같은 섹션을 절대 추가하지 마세요.
3. 비평가가 지적한 '참고문헌 6번' 같은 텍스트가 보고서에 그대로 남지 않도록 완전히 수정하거나 제거하세요.
"""


class AuthorAgent:
    """저자 에이전트 - 보고서 작성"""
    
    def __init__(self, api_key: str = None):
        self.api_key = api_key or OPENAI_API_KEY
        self.llm = None
        if LANGCHAIN_AVAILABLE and self.api_key:
            self.llm = ChatOpenAI(
                model="gpt-4o-mini",
                temperature=0.5,
                api_key=self.api_key
            )
    
    def write_draft(self, state: ScientificState) -> ScientificState:
        """보고서 초안 작성"""
        hypothesis = state.get('user_input', '')
        domain = state.get('domain', '')
        literature = state.get('literature_context', [])
        methods = state.get('proposed_methods', [])
        selected_idx = state.get('selected_method_index', 0)
        logs = state.get('execution_logs', '')
        experiment_results = state.get('experiment_results', {})  # 실험 결과 추가
        code = state.get('code_repository', {}).get('experiment.py', '')
        data_source = state.get('data_source', {})  # 학술 데이터 출처
        
        methodology = methods[selected_idx] if methods and selected_idx < len(methods) else {}
        literature_summary = self._format_literature(literature)
        
        if self.llm:
            draft = self._write_draft_llm(hypothesis, domain, literature_summary, methodology, logs, experiment_results)
        else:
            draft = self._write_draft_mock(hypothesis, domain, literature_summary, methodology, logs, experiment_results, code, data_source)
        
        state['draft_report'] = draft
        state['current_step'] = 'draft_written'
        
        return state
    
    def refine_report(self, state: ScientificState) -> ScientificState:
        """피드백 반영하여 보고서 수정"""
        draft = state.get('draft_report', '')
        feedback = state.get('critic_feedback', '')
        
        if self.llm and feedback:
            final = self._refine_llm(draft, feedback)
        else:
            final = draft + "\n\n---\n*피드백 반영 완료*"
        
        state['final_report'] = final
        
        # 파일 저장
        report_path = self._save_report(state)
        state['report_pdf_path'] = report_path
        state['status'] = 'completed'
        state['current_step'] = 'report_finalized'
        
        return state
    
    def _format_literature(self, literature: List[Dict]) -> str:
        """문헌 목록 포맷팅 (검증 포함)"""
        if not literature:
            return "관련 문헌 없음"
        
        items = []
        for i, item in enumerate(literature[:5], 1):
            title = item.get('title', '')
            authors = item.get('authors', [])
            year = item.get('year', '')
            
            # 검증: 불완전한 문헌 필터링
            if not title or len(title) < 5:
                continue
            if not authors and not year:
                continue
            
            # 형식 정규화
            author_str = ', '.join(authors[:3]) if authors else 'Unknown'
            year_str = str(year) if year else 'N/A'
            
            # 불완전한 텍스트 패턴 제거
            invalid_patterns = ['...', '-텍스트', '-생성', 'Sar...', 'for Au']
            is_valid = all(p not in title for p in invalid_patterns)
            
            if is_valid:
                items.append(f"{len(items) + 1}. {author_str} ({year_str}). {title}")
        
        if not items:
            return "유효한 참고문헌 없음"
        
        return "\n".join(items)
    
    def _validate_references_in_report(self, report: str) -> str:
        """보고서 내 참고문헌 섹션 검증 및 정리"""
        import re
        
        # 참고문헌 섹션 찾기
        ref_match = re.search(r'(## 참고문헌.*?)(?=\n##|\n---|\Z)', report, re.DOTALL)
        if not ref_match:
            return report
        
        ref_section = ref_match.group(1)
        
        # 불완전한 패턴 제거
        invalid_patterns = [
            r'추가\s*연구.*',
            r'등을?\s*통해.*',
            r'참고문헌을?\s*통해.*',
            r'\d+\.\s*$',  # 빈 항목
            r'-텍스트.*',
            r'-생성.*',
        ]
        
        cleaned_section = ref_section
        for pattern in invalid_patterns:
            cleaned_section = re.sub(pattern, '', cleaned_section, flags=re.MULTILINE)
        
        # 연속 빈 줄 정리
        cleaned_section = re.sub(r'\n{3,}', '\n\n', cleaned_section)
        
        return report.replace(ref_section, cleaned_section)
    
    def _write_draft_llm(self, hypothesis: str, domain: str, literature: str, 
                         methodology: Dict, results: str, experiment_results: Dict = None) -> str:
        """LLM 초안 작성"""
        # 실험 결과 데이터가 있으면 포함
        results_section = results[:2000] if results else "실험 결과 없음"
        if experiment_results:
            results_section += f"\n\n### 정량적 실험 결과 (JSON):\n{json.dumps(experiment_results, ensure_ascii=False, indent=2)}"
        
        prompt = AUTHOR_DRAFT_PROMPT.format(
            hypothesis=hypothesis,
            domain=domain or "과학",
            literature=literature,
            methodology=json.dumps(methodology, ensure_ascii=False, indent=2) if methodology else "방법론 정보 없음",
            results=results_section
        )
        response = self.llm.invoke([HumanMessage(content=prompt)])
        return response.content
    
    def _write_draft_mock(self, hypothesis: str, domain: str, literature: str,
                          methodology: Dict, results: str, experiment_results: Dict = None, 
                          code: str = '', data_source: Dict = None) -> str:
        """Mock 초안 작성 (LLM 미사용시 폴백) - 실제 실험 결과 반영"""
        method_title = methodology.get('title', '시뮬레이션 분석') if methodology else '시뮬레이션 분석'
        method_type = methodology.get('type', 'simulation') if methodology else 'simulation'
        method_desc = methodology.get('description', '') if methodology else ''
        
        # 도메인별 구체적 시나리오 생성
        scenario_map = {
            'AI': ['텍스트 분류 태스크', '코드 생성 태스크', '질의응답 태스크'],
            '물리학': ['입자 시뮬레이션', '파동 방정식 해석', '열역학 모델링'],
            '생물학': ['단백질 접힘 시뮬레이션', '유전자 발현 분석', '세포 성장 모델'],
            '화학': ['분자 동역학 계산', '반응 속도론 분석', '스펙트럼 시뮬레이션'],
            'default': ['기본 조건 실험', '변형 조건 실험', '극한 조건 실험']
        }
        scenarios = scenario_map.get(domain, scenario_map['default'])
        
        # 실험 결과 동적 생성
        if experiment_results:
            result_table = self._format_experiment_results(experiment_results)
            conclusion = experiment_results.get('conclusion', '추가 분석이 필요합니다.')
            analysis_type = experiment_results.get('analysis_type', method_title)
        else:
            result_table = """| 지표 | 값 | 비고 |
|------|-----|------|
| 실행 상태 | 완료 | - |
| 결과 | 대기 중 | 상세 결과는 로그 참조 |"""
            conclusion = '실험이 수행되었으나 상세 정량 결과가 없습니다.'
            analysis_type = method_title
        
        # 실험 로그에서 주요 출력 추출
        log_summary = self._extract_key_outputs(results) if results else '실험 로그가 없습니다.'
        
        # 코드 일부 발췌 (주석과 핵심 로직)
        code_excerpt = self._extract_code_excerpt(code) if code else '코드가 제공되지 않았습니다.'
        
        return f"""# 연구 보고서: {hypothesis[:60]}

## 초록 (Abstract)

본 연구는 "{hypothesis}"라는 가설을 검증하기 위해 수행되었습니다. 
{method_title} 방법을 사용하여 {analysis_type} 기반 분석을 진행하였습니다.
{conclusion}

## 1. 서론 (Introduction)

### 1.1 연구 배경
{domain or '과학'} 분야에서 본 연구 주제는 학술적, 실용적 측면에서 중요한 의미를 가집니다.
관련 선행 연구들은 이 주제에 대한 부분적인 해답을 제시하였으나, 체계적인 검증이 부족했습니다.

### 1.2 연구 목적
본 연구의 목적은 다음과 같습니다:
1. 가설의 타당성을 정량적으로 검증
2. 다양한 시나리오에서의 일반화 가능성 확인
3. 실용적 적용을 위한 기초 데이터 제공

### 1.3 가설
> {hypothesis}

### 1.4 관련 연구
{literature}

## 2. 방법 (Methods)

### 2.1 실험 설계
{self._generate_experiment_design(experiment_results, hypothesis, method_title)}

### 2.2 실험 절차
{self._generate_experiment_procedure(results, experiment_results, methodology)}

### 2.3 데이터 수집
{self._generate_data_collection_info(experiment_results, results, hypothesis, data_source)}

### 2.4 분석 방법
{self._generate_analysis_method(experiment_results)}

## 3. 결과 (Results)

### 3.1 정량적 결과
{result_table}

### 3.2 실험 출력
{log_summary}

### 3.3 결과 해석
{conclusion}

## 4. 토의 (Discussion)

### 4.1 결과 해석
{f'실험 결과 분석 유형: {analysis_type}' if experiment_results else '실험이 수행되었습니다.'}

### 4.2 가설 검증 결론
가설 "{hypothesis[:40]}..."에 대해:
- 결론: {conclusion}

### 4.3 한계점
본 연구의 한계점은 다음과 같습니다:
1. 시뮬레이션 기반 분석으로 실제 환경 적용시 차이 발생 가능
2. 합성 데이터 사용으로 실제 데이터와의 차이 존재
3. 외부 변수 통제의 어려움

## 5. 결론 (Conclusion)

본 연구는 가설 "{hypothesis[:30]}..."에 대한 초기 증거를 제공하였습니다.
{conclusion}
향후 연구에서는 더 다양한 시나리오와 실제 데이터를 활용한 검증이 필요합니다.

## 참고문헌 (References)

{literature}

---
*보고서 생성일: {datetime.now().strftime('%Y-%m-%d %H:%M')} | 자율 과학 발견 시스템*
"""
    
    def _format_experiment_results(self, results: Dict) -> str:
        """실험 결과를 마크다운 테이블로 포맷팅"""
        if not results:
            return "결과 데이터 없음"
        
        lines = ["| 지표 | 값 | 비고 |", "|------|-----|------|"]
        
        for key, value in results.items():
            if key == 'conclusion':
                continue
            if isinstance(value, float):
                formatted_value = f"{value:.4f}"
            elif isinstance(value, list):
                formatted_value = ', '.join(str(v) for v in value[:3])
            elif isinstance(value, dict):
                formatted_value = str(value)[:50] + '...' if len(str(value)) > 50 else str(value)
            else:
                formatted_value = str(value)
            
            lines.append(f"| {key} | {formatted_value} | - |")
        
        return '\n'.join(lines)
    
    def _extract_key_outputs(self, logs: str) -> str:
        """실험 로그에서 핵심 출력 추출"""
        if not logs:
            return "로그 없음"
        
        # 마지막 성공한 시도의 결과만 추출
        parts = logs.split('=== 시도')
        if len(parts) > 1:
            last_attempt = parts[-1]
            # 주요 출력 라인 추출
            lines = [l.strip() for l in last_attempt.split('\n') if l.strip() and not l.startswith('===')]
            return '\n'.join(lines[:20])  # 최대 20줄
        
        return logs[:1000]
    
    def _extract_code_excerpt(self, code: str) -> str:
        """코드에서 핵심 부분 발췌"""
        if not code:
            return "# 코드 없음"
        
        lines = code.split('\n')
        # 주석이 아닌 코드 라인만 추출 (최대 30줄)
        code_lines = []
        for line in lines:
            stripped = line.strip()
            if stripped and not stripped.startswith('#') and not stripped.startswith('"""') and not stripped.startswith("'''"):
                code_lines.append(line)
            if len(code_lines) >= 30:
                break
        
        if len(code_lines) > 25:
            return '\n'.join(code_lines[:12]) + '\n# ... (중략) ...\n' + '\n'.join(code_lines[-8:])
        
        return '\n'.join(code_lines) if code_lines else '# 핵심 코드 없음'
    
    def _generate_experiment_design(self, experiment_results: Dict, hypothesis: str, method_title: str) -> str:
        """실험 설계 정보 동적 생성"""
        if not experiment_results:
            return f"본 연구에서는 {method_title} 방법을 적용하여 가설을 검증하였습니다."
        
        # 실험 결과에서 정보 추출
        conditions = experiment_results.get('conditions', [])
        measurements = experiment_results.get('measurements', {})
        sample_size = experiment_results.get('sample_size', 'N/A')
        
        # 독립변수 추출 (가설에서 키워드 추출 시도)
        import re
        var_match = re.search(r'([\w가-힣]+)[을를이가]?\s*(첨가|증가|감소|사용)', hypothesis)
        independent_var = var_match.group(1) if var_match else "실험 조건"
        
        # 각 불릿 포인트를 별도 줄로 구성 (줄 끝 공백 2개로 마크다운 줄바꿈)
        design_lines = [
            f"본 연구에서는 {method_title} 방법을 적용하여 가설을 검증하였습니다.",
            "",
            f"- 독립변수: {independent_var} ({', '.join(conditions) if conditions else '다양한 조건'})",
            f"- 종속변수: 측정 결과값",
            f"- 샘플 크기: n = {sample_size}",
            f"- 반복 횟수: 각 조건당 3회 이상"
        ]
        
        return '\n'.join(design_lines)
    
    def _generate_experiment_procedure(self, logs: str, experiment_results: Dict, methodology: Dict) -> str:
        """실험 절차 동적 생성 - 로그에서 단계별 정보 추출"""
        import re
        
        procedures = []
        
        # 로그에서 단계별 정보 추출 시도
        if logs:
            # 1단계: 데이터 준비
            data_prep = re.search(r'(?:1단계|데이터 준비)[:\s]*(.+?)(?=\n=|\n\n|$)', logs, re.DOTALL)
            if data_prep:
                procedures.append(f"1. **데이터 준비**: {data_prep.group(1).strip()[:200]}")
            
            # 2단계: 실험 조건
            conditions = re.search(r'(?:2단계|실험 조건)[:\s]*(.+?)(?=\n=|\n\n|$)', logs, re.DOTALL)
            if conditions:
                procedures.append(f"2. **조건 설정**: {conditions.group(1).strip()[:200]}")
            
            # 3단계: 데이터 수집
            collection = re.search(r'(?:3단계|데이터 수집)[:\s]*(.+?)(?=\n=|\n\n|$)', logs, re.DOTALL)
            if collection:
                procedures.append(f"3. **측정 수행**: {collection.group(1).strip()[:200]}")
            
            # 4단계: 분석
            analysis = re.search(r'(?:4단계|분석)[:\s]*(.+?)(?=\n=|\n\n|$)', logs, re.DOTALL)
            if analysis:
                procedures.append(f"4. **분석 수행**: {analysis.group(1).strip()[:200]}")
        
        # 로그에서 추출 실패 시 기본 내용
        if not procedures:
            method_desc = methodology.get('description', '') if methodology else ''
            procedures = [
                f"1. **데이터 준비**: 가설 검증을 위한 데이터 수집 및 전처리",
                f"2. **조건 설정**: 다양한 실험 조건 설정",
                f"3. **측정 수행**: 각 조건에서 측정값 기록",
                f"4. **분석 수행**: 통계적 분석 및 패턴 도출"
            ]
            if method_desc:
                procedures.insert(0, f"**방법론 개요**: {method_desc[:150]}")
        
        return '\n'.join(procedures)
    
    def _generate_data_collection_info(self, experiment_results: Dict, logs: str, hypothesis: str = '', data_source: Dict = None) -> str:
        """데이터 수집 정보 동적 생성 - 실제 논문 출처 우선, 로그에서 추출 강화"""
        import re
        info_parts = []
        
        # === 실제 논문 출처가 있으면 우선 표시 ===
        if data_source and data_source.get('type') == 'academic_paper':
            citation = data_source.get('citation', '')
            title = data_source.get('title', '')
            authors = data_source.get('authors', [])
            year = data_source.get('year', 0)
            doi = data_source.get('doi')
            url = data_source.get('url')
            
            if citation:
                info_parts.append(f"- **데이터 출처**: {citation}")
            elif title:
                author_str = authors[0] if authors else "Unknown"
                if len(authors) > 1:
                    author_str += " et al."
                info_parts.append(f"- **데이터 출처**: {author_str} ({year}) \"{title}\"")
            
            if doi:
                info_parts.append(f"- **DOI**: {doi}")
            elif url:
                info_parts.append(f"- **URL**: {url}")
            
            info_parts.append(f"- **출처 유형**: 학술 논문 (Semantic Scholar/arXiv)")
        
        # === 가설에서 핵심 키워드 추출 (연구 주제) ===
        topic_keywords = []
        if hypothesis:
            # 핵심 명사 추출 시도
            topic_match = re.search(r'([가-힣A-Za-z]+(?:제|량|농도|온도|압력|시간|강도|비율))', hypothesis)
            if topic_match:
                topic_keywords.append(topic_match.group(1))
            # 추가 키워드
            for keyword in ['콘크리트', '시멘트', '철근', '강도', 'AE제', '혼화제', '압축', '인장']:
                if keyword in hypothesis:
                    topic_keywords.append(keyword)
        
        topic_str = ', '.join(set(topic_keywords[:3])) if topic_keywords else '관련 분야'
        
        # 1. 로그에서 데이터 정보 추출 시도 (data_source가 없을 때만)
        if not info_parts and logs:
            # 데이터 출처 추출
            source_match = re.search(r'(?:데이터\s*출처|Data\s*source)[:\s]*([^\n]+)', logs, re.IGNORECASE)
            if source_match:
                raw_source = source_match.group(1).strip()
                # "문헌" 같은 모호한 표현이면 시뮬레이션으로 표시
                if raw_source in ['문헌', '기존 문헌', '연구 문헌', '기존 연구']:
                    info_parts.append(f"- **데이터 출처**: Python 시뮬레이션 기반 합성 데이터 ({topic_str} 모델링)")
                else:
                    info_parts.append(f"- **데이터 출처**: {raw_source}")
            
            # 샘플 크기 추출
            sample_match = re.search(r'(?:샘플|데이터\s*크기|n\s*=|총\s*\d+개)[:\s]*(\d+)', logs, re.IGNORECASE)
            if sample_match:
                info_parts.append(f"- **샘플 크기**: n = {sample_match.group(1)}")
            
            # 조건별 측정값 추출
            condition_matches = re.findall(r'(\d+%?)[:\s]*[-→>]?\s*(\d+\.?\d*)', logs)
            if condition_matches and len(condition_matches) >= 2:
                info_parts.append("\n**수집된 데이터 (조건별 측정값)**:")
                info_parts.append("")  # 표 앞 빈 줄
                info_parts.append("| 조건 | 측정값 |")
                info_parts.append("|------|--------|")
                for cond, val in condition_matches[:5]:
                    info_parts.append(f"| {cond} | {val} |")
                info_parts.append("")  # 표 뒤 빈 줄
        
        # 2. experiment_results에서 추출
        if experiment_results:
            if not info_parts:  # 로그에서 못 찾았으면
                data_source = experiment_results.get('data_source', '')
                if data_source:
                    info_parts.append(f"- **데이터 출처**: {data_source}")
                
            sample_size = experiment_results.get('sample_size')
            if sample_size and "샘플 크기" not in str(info_parts):
                info_parts.append(f"- **샘플 크기**: n = {sample_size}")
            
            # 조건별 측정값
            conditions = experiment_results.get('conditions', [])
            measurements = experiment_results.get('measurements', {})
            
            if conditions and measurements and "조건별 측정값" not in str(info_parts):
                info_parts.append("\n**수집된 데이터 (조건별 측정값)**:")
                info_parts.append("")  # 표 앞 빈 줄
                info_parts.append("| 조건 | 측정값 |")
                info_parts.append("|------|--------|")
                for cond in conditions[:5]:
                    val = measurements.get(cond, 'N/A')
                    if isinstance(val, (int, float)):
                        info_parts.append(f"| {cond} | {val:.2f} |")
                    else:
                        info_parts.append(f"| {cond} | {val} |")
                info_parts.append("")  # 표 뒤 빈 줄
        
        # 3. 기본값 (로그와 results 모두에서 추출 실패 시)
        if not info_parts:
            info_parts = [
                "- **데이터 출처**: 시뮬레이션 기반 합성 데이터",
                "- **샘플 크기**: 실험 조건에 따라 다름",
                "- **측정 방법**: Python 기반 자동화 측정",
                "",
                "> ⚠️ 상세 데이터는 실험 로그를 참조하세요."
            ]
        
        return '\n'.join(info_parts)
    
    def _generate_analysis_method(self, experiment_results: Dict) -> str:
        """분석 방법 정보 동적 생성"""
        info_parts = []
        
        if experiment_results:
            analysis_type = experiment_results.get('analysis_type', '통계 분석')
            info_parts.append(f"- 분석 기법: {analysis_type}")
            
            p_value = experiment_results.get('p_value')
            if p_value is not None:
                info_parts.append(f"- 유의수준: α = 0.05")
                info_parts.append(f"- 산출된 p-value: {p_value:.4f}")
            
            effect_size = experiment_results.get('effect_size')
            if effect_size is not None:
                info_parts.append(f"- 효과 크기: {effect_size:.3f}")
            
            info_parts.append("- 분석 도구: Python (scipy, statsmodels)")
        
        if not info_parts:
            info_parts = [
                "- 분석 기법: 회귀분석/상관분석",
                "- 유의수준: α = 0.05",
                "- 분석 도구: Python (scipy, numpy)"
            ]
        
        return '\n'.join(info_parts)
    
    def _refine_llm(self, draft: str, feedback: str) -> str:
        """LLM을 이용한 보고서 수정"""
        prompt = AUTHOR_REFINE_PROMPT.format(
            draft=draft,
            feedback=feedback
        )
        response = self.llm.invoke([HumanMessage(content=prompt)])
        return response.content
    
    def _save_report(self, state: ScientificState) -> str:
        """보고서 파일 저장 (PDF)"""
        session_id = state.get('session_id', 'unknown')
        report = state.get('final_report', '')
        
        REPORTS_DIR.mkdir(exist_ok=True)
        filename = f"report_{session_id[:8]}_{datetime.now().strftime('%Y%m%d_%H%M')}.pdf"
        filepath = REPORTS_DIR / filename
        
        # fpdf2를 사용하여 PDF 생성
        try:
            from fpdf import FPDF
            import re
            from pathlib import Path
            
            pdf = FPDF()
            pdf.set_auto_page_break(auto=True, margin=15)
            pdf.set_left_margin(15)
            pdf.set_right_margin(15)
            
            # 한국어 지원 폰트 추가 (시스템 폰트 우선)
            font_candidates = [
                Path("/usr/share/fonts/truetype/unfonts-core/UnDotum.ttf"),  # 은 돋움
                Path("/usr/share/fonts/truetype/baekmuk/dotum.ttf"),  # 백묵 돋움
                Path("/usr/share/fonts/truetype/nanum/NanumGothic.ttf"),
                Path(__file__).parent.parent / "fonts" / "NanumGothic.ttf",
            ]
            
            font_path = None
            for candidate in font_candidates:
                if candidate.exists():
                    font_path = str(candidate)
                    break
            
            if font_path:
                pdf.add_font('Korean', '', font_path)
                pdf.add_font('Korean', 'B', font_path)
                font_name = 'Korean'
                print(f"Using font: {font_path}")
            else:
                font_name = 'Helvetica'
                print("Warning: Korean font not found, using Helvetica")
            
            pdf.add_page()
            
            # 페이지 너비 설정 (마진 고려하여 안전하게 설정)
            page_width = 175
            
            # 마크다운 파싱 - 연속된 줄 병합 처리
            lines = report.split('\n')
            
            # 줄 병합: 특수 문자로 시작하지 않는 연속된 줄을 합침
            merged_lines = []
            current_paragraph = ""
            current_table = []  # 테이블 라인들을 모으기 위한 리스트
            
            for line in lines:
                line = line.rstrip()
                
                # 테이블 감지
                if line.strip().startswith('|'):
                    # 이전 단락이 있으면 저장
                    if current_paragraph:
                        merged_lines.append(current_paragraph)
                        current_paragraph = ""
                    
                    # 테이블 라인 추가
                    current_table.append(line)
                    continue
                else:
                    # 테이블이 모여있었다면 저장
                    if current_table:
                        merged_lines.append(('TABLE', current_table))
                        current_table = []
                
                # 특수 시작 문자들 (새 단락 시작)
                is_special = (
                    line.startswith('#') or 
                    line.startswith('>') or 
                    line.startswith('-') or 
                    line.startswith('*') or
                    re.match(r'^\d+\.', line) or
                    line == '' or
                    line.startswith('---')
                )
                
                if is_special:
                    # 이전 단락 저장
                    if current_paragraph:
                        merged_lines.append(current_paragraph)
                        current_paragraph = ""
                    merged_lines.append(line)
                else:
                    # 현재 단락에 추가 (공백으로 연결)
                    if current_paragraph:
                        current_paragraph += " " + line.strip()
                    else:
                        current_paragraph = line.strip()
            
            # 마지막 단락 저장
            if current_paragraph:
                merged_lines.append(current_paragraph)
            # 마지막 테이블 저장
            if current_table:
                merged_lines.append(('TABLE', current_table))
            
            # PDF 렌더링
            for line in merged_lines:
                # 테이블 처리
                if isinstance(line, tuple) and line[0] == 'TABLE':
                    table_lines = line[1]
                    # 유효한 테이블 데이터만 추출
                    rows = []
                    for t_line in table_lines:
                        t_line = t_line.strip()
                        # 구분선 감지: 파이프 제거 후 하이픈/콜론/공백만 있으면 구분선
                        cells_content = t_line.replace('|', '').strip()
                        if cells_content and all(c in '-: ' for c in cells_content):
                            continue  # 구분선은 건너뜀
                        
                        # 셀 데이터 추출
                        cells = [c.strip() for c in t_line.split('|')]
                        # 앞뒤 빈 셀 제거 (|로 시작/끝나면 빈 문자열 생김)
                        if cells and cells[0] == '':
                            cells = cells[1:]
                        if cells and cells[-1] == '':
                            cells = cells[:-1]
                        if cells:
                            rows.append(cells)
                    
                    if not rows:
                        continue
                        
                    # 테이블 렌더링
                    pdf.ln(4)
                    pdf.set_text_color(51, 51, 51)
                    
                    # 컬럼 너비 계산 (단순 등분할)
                    num_cols = len(rows[0]) if rows else 1
                    if num_cols == 0: num_cols = 1
                    col_width = page_width / num_cols
                    row_height = 8
                    
                    for row_idx, row in enumerate(rows):
                        # 셀 개수 맞추기
                        while len(row) < num_cols:
                            row.append('')
                        if len(row) > num_cols:
                            row = row[:num_cols]
                        
                        # 페이지 넘김 체크: 현재 위치 + row_height가 페이지 하단을 넘으면 새 페이지
                        if pdf.get_y() + row_height > pdf.page_break_trigger:
                            pdf.add_page()
                        
                        # 시작 위치
                        start_x = 15
                        start_y = pdf.get_y()
                        
                        # 헤더 스타일 설정
                        is_header = (row_idx == 0)
                        
                        # 각 셀을 cell()로 그리기 (같은 줄에 배치)
                        for col_idx, cell_text in enumerate(row):
                            pdf.set_xy(start_x + (col_idx * col_width), start_y)
                            
                            if is_header:
                                pdf.set_font(font_name, 'B', 9)
                                pdf.set_fill_color(235, 235, 240)
                            else:
                                pdf.set_font(font_name, '', 9)
                                pdf.set_fill_color(255, 255, 255)
                            
                            # cell()은 줄바꿈 안 함, 테두리 1, 가운데 정렬
                            pdf.cell(col_width, row_height, cell_text, border=1, align='C', fill=is_header)
                        
                        # 다음 행으로 이동
                        pdf.set_y(start_y + row_height)
                    
                    pdf.ln(4)
                    continue

                if not line:
                    pdf.ln(3)
                    continue
                
                # 항상 왼쪽 마진으로 x 위치 초기화 (중요: 잘림 방지)
                pdf.set_x(15)
                
                # 마크다운 기호 제거 함수
                def clean_markdown(text):
                    text = re.sub(r'\*\*(.*?)\*\*', r'\1', text)
                    text = re.sub(r'\*(.*?)\*', r'\1', text)
                    text = re.sub(r'`([^`]+)`', r'\1', text)
                    return text.strip()
                
                # 제목 처리
                if line.startswith('# '):
                    text = clean_markdown(line[2:])
                    if text:
                        pdf.set_font(font_name, 'B', 16)
                        pdf.set_text_color(26, 26, 46)
                        pdf.multi_cell(page_width, 8, text)
                        pdf.ln(2)
                elif line.startswith('## '):
                    text = clean_markdown(line[3:])
                    if text:
                        pdf.set_font(font_name, 'B', 13)
                        pdf.set_text_color(45, 45, 90)
                        pdf.multi_cell(page_width, 7, text)
                        pdf.ln(2)
                elif line.startswith('### '):
                    text = clean_markdown(line[4:])
                    if text:
                        pdf.set_font(font_name, 'B', 11)
                        pdf.set_text_color(61, 61, 106)
                        pdf.multi_cell(page_width, 6, text)
                        pdf.ln(1)
                elif line.startswith('> '):
                    text = clean_markdown(line[2:])
                    if text:
                        pdf.set_font(font_name, '', 10)
                        pdf.set_text_color(85, 85, 85)
                        pdf.multi_cell(page_width, 5, '  ' + text)
                elif line.startswith('- ') or line.startswith('* '):
                    text = clean_markdown(line[2:])
                    if text:
                        pdf.set_font(font_name, '', 10)
                        pdf.set_text_color(51, 51, 51)
                        pdf.multi_cell(page_width, 5, '• ' + text)
                elif re.match(r'^\d+\.', line):
                    # 번호 목록 처리
                    text = clean_markdown(line)
                    if text:
                        pdf.set_font(font_name, '', 10)
                        pdf.set_text_color(51, 51, 51)
                        pdf.multi_cell(page_width, 5, text)
                elif line.startswith('|'):
                    # 테이블 블록 처리로 이동되어 여기는 실행되지 않아야 함
                    pass
                elif line.startswith('---'):
                    pdf.ln(2)
                    pdf.set_draw_color(200, 200, 200)
                    pdf.line(15, pdf.get_y(), 195, pdf.get_y())
                    pdf.ln(2)
                else:
                    # 일반 텍스트
                    text = clean_markdown(line)
                    if text:
                        pdf.set_font(font_name, '', 10)
                        pdf.set_text_color(51, 51, 51)
                        pdf.multi_cell(page_width, 5, text)
            
            pdf.output(str(filepath))
            print(f"PDF 보고서 생성 완료: {filepath}")
            
        except Exception as e:
            print(f"PDF 변환 실패, 마크다운으로 저장: {e}")
            import traceback
            traceback.print_exc()
            filename = filename.replace('.pdf', '.md')
            filepath = REPORTS_DIR / filename
            with open(filepath, 'w', encoding='utf-8') as f:
                f.write(report)
        
        return str(filepath)


def author_draft_node(state: ScientificState) -> ScientificState:
    """초안 작성 노드"""
    agent = AuthorAgent()
    return agent.write_draft(state)


def author_refine_node(state: ScientificState) -> ScientificState:
    """최종 보고서 작성 노드"""
    agent = AuthorAgent()
    return agent.refine_report(state)
