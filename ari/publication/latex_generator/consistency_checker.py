"""
Consistency Checker

논문 전체의 기술적 일관성 검증
"""

import re
from typing import Dict, List, Set, Tuple, Optional
from dataclasses import dataclass, field


@dataclass
class ConsistencyIssue:
    """일관성 문제"""
    issue_type: str  # "algorithm_mismatch", "term_inconsistency", etc.
    section: str
    description: str
    suggestion: str


@dataclass 
class ConsistencyReport:
    """일관성 검사 보고서"""
    is_consistent: bool
    issues: List[ConsistencyIssue] = field(default_factory=list)
    extracted_terms: Dict[str, Set[str]] = field(default_factory=dict)


class ConsistencyChecker:
    """
    논문 내용의 기술적 일관성 검증
    - 알고리즘/모델명 일치 확인
    - 핵심 용어 통일성 확인
    - 수치 범위 합리성 확인
    """
    
    # 일반적인 알고리즘/모델 패턴
    ALGORITHM_PATTERNS = [
        # Deep Learning
        r'\b(CNN|RNN|LSTM|GRU|Transformer|BERT|GPT|ResNet|VGG|U-Net)\b',
        r'\b(DQN|PPO|A3C|SAC|DDPG|TD3)\b',  # RL algorithms
        r'\b(GAN|VAE|Diffusion|Autoencoder)\b',  # Generative
        r'\b(Adam|SGD|RMSprop|AdamW)\b',  # Optimizers
        
        # ML Algorithms
        r'\b(Random Forest|XGBoost|LightGBM|SVM|KNN|Naive Bayes)\b',
        r'\b(K-means|DBSCAN|PCA|t-SNE|UMAP)\b',
        
        # Communication/Signal Processing
        r'\b(MIMO|OFDM|CDMA|LTE|5G|6G)\b',
        r'\b(IRS|RIS|Beamforming|Precoding)\b',
        
        # Optimization
        r'\b(Genetic Algorithm|GA|PSO|Simulated Annealing)\b',
        r'\b(Gradient Descent|Newton|BFGS)\b'
    ]
    
    # 섹션별 핵심 용어 추출용 패턴
    KEY_TERM_PATTERNS = [
        r'\b([A-Z][a-z]+(?:\s+[A-Z][a-z]+)+)\b',  # Title Case phrases
        r'\b([A-Z]{2,}(?:-[A-Z]+)?)\b',  # Acronyms
    ]
    
    def __init__(self):
        self.compiled_algo_patterns = [re.compile(p, re.IGNORECASE) for p in self.ALGORITHM_PATTERNS]
    
    def check_consistency(
        self,
        sections: Dict[str, str]
    ) -> ConsistencyReport:
        """
        전체 논문 일관성 검사
        
        Args:
            sections: 섹션명 -> 내용 딕셔너리
        
        Returns:
            ConsistencyReport
        """
        issues = []
        extracted_terms = {}
        
        # 1. 각 섹션에서 알고리즘/모델명 추출
        for section_name, content in sections.items():
            terms = self._extract_algorithms(content)
            extracted_terms[section_name] = terms
        
        # 2. Methodology와 다른 섹션 비교
        methodology_terms = extracted_terms.get("Methodology", set())
        if not methodology_terms:
            methodology_terms = extracted_terms.get("methodology", set())
        
        if methodology_terms:
            for section_name, terms in extracted_terms.items():
                if section_name.lower() in ["methodology", "abstract"]:
                    continue
                
                # 다른 섹션에만 있는 알고리즘 검출
                extra_terms = terms - methodology_terms
                for term in extra_terms:
                    # 일반적인 단어는 제외
                    if term.upper() in ["THE", "AND", "FOR", "WITH"]:
                        continue
                    if len(term) < 3:
                        continue
                    
                    issues.append(ConsistencyIssue(
                        issue_type="algorithm_mismatch",
                        section=section_name,
                        description=f"Term '{term}' appears in {section_name} but not in Methodology",
                        suggestion=f"Add '{term}' to Methodology or remove from {section_name}"
                    ))
        
        # 3. 결과 반환
        is_consistent = len(issues) == 0
        
        return ConsistencyReport(
            is_consistent=is_consistent,
            issues=issues,
            extracted_terms=extracted_terms
        )
    
    def fix_inconsistencies(
        self,
        sections: Dict[str, str],
        report: ConsistencyReport
    ) -> Dict[str, str]:
        """
        일관성 문제 자동 수정
        
        현재는 간단한 규칙 기반 수정만 수행:
        - Methodology의 알고리즘으로 다른 섹션의 불일치 알고리즘 대체
        """
        if report.is_consistent:
            return sections
        
        fixed_sections = dict(sections)
        
        # Methodology에서 주요 알고리즘 추출
        methodology_terms = report.extracted_terms.get("Methodology", set())
        if not methodology_terms:
            methodology_terms = report.extracted_terms.get("methodology", set())
        
        # 가장 자주 나오는 알고리즘을 "주요 알고리즘"으로 간주
        if methodology_terms:
            primary_algo = list(methodology_terms)[0]  # 첫 번째 것 사용
            
            for issue in report.issues:
                if issue.issue_type == "algorithm_mismatch":
                    section_name = issue.section
                    # 해당 용어를 primary_algo로 대체
                    problematic_term = issue.description.split("'")[1]
                    
                    if section_name in fixed_sections:
                        fixed_sections[section_name] = fixed_sections[section_name].replace(
                            problematic_term, primary_algo
                        )
        
        return fixed_sections
    
    def _extract_algorithms(self, text: str) -> Set[str]:
        """텍스트에서 알고리즘/모델명 추출"""
        found = set()
        
        for pattern in self.compiled_algo_patterns:
            matches = pattern.findall(text)
            for match in matches:
                if isinstance(match, tuple):
                    match = match[0]
                found.add(match.upper())
        
        return found
    
    def get_methodology_summary(self, methodology_content: str) -> Dict[str, any]:
        """
        Methodology 섹션에서 핵심 정보 추출
        (다른 섹션 생성 시 참조용)
        """
        algorithms = self._extract_algorithms(methodology_content)
        
        # 데이터셋 언급 추출
        datasets = set()
        dataset_patterns = [
            r'\b(MNIST|CIFAR|ImageNet|COCO|VOC|SQuAD|GLUE)\b',
            r'\b([A-Z][a-z]+(?:Net|Set|Data|Corpus))\b'
        ]
        for pattern in dataset_patterns:
            matches = re.findall(pattern, methodology_content, re.IGNORECASE)
            datasets.update(m.upper() if isinstance(m, str) else m[0].upper() for m in matches)
        
        return {
            "algorithms": list(algorithms),
            "datasets": list(datasets),
            "raw_excerpt": methodology_content[:500]
        }


def ensure_section_consistency(
    all_sections_content: Dict[str, str],
    methodology_summary: Dict[str, any]
) -> str:
    """
    새 섹션 생성 시 일관성 확보를 위한 프롬프트 보강 텍스트 생성
    """
    algos = methodology_summary.get("algorithms", [])
    datasets = methodology_summary.get("datasets", [])
    
    constraint_text = "\n\nIMPORTANT CONSISTENCY REQUIREMENTS:\n"
    
    if algos:
        constraint_text += f"- Use ONLY these algorithms/models: {', '.join(algos)}\n"
        constraint_text += "- Do NOT introduce new algorithms not mentioned in Methodology\n"
    
    if datasets:
        constraint_text += f"- Reference ONLY these datasets: {', '.join(datasets)}\n"
    
    constraint_text += "- Maintain consistent terminology throughout the paper\n"
    
    return constraint_text
