"""
Synthetic Data Generator

가설 기반 합리적인 실험 데이터 생성
- 메트릭 극성(higher/lower is better) 인식
- 명확한 베이스라인 정의
- 도메인별 합리적인 수치 생성
"""

import random
from typing import Dict, List, Any, Optional, Tuple
from dataclasses import dataclass, field


@dataclass
class MetricInfo:
    """메트릭 정보"""
    name: str
    value: float
    std: float
    higher_is_better: bool  # True: 높을수록 좋음, False: 낮을수록 좋음
    unit: str = ""


@dataclass
class BaselineInfo:
    """베이스라인 정보"""
    name: str
    description: str  # 예: "Traditional MIMO without IRS"
    short_name: str   # 테이블용 약칭


@dataclass
class ExperimentMetrics:
    """실험 메트릭"""
    primary_metric: MetricInfo
    secondary_metrics: List[MetricInfo] = field(default_factory=list)
    baselines: List[BaselineInfo] = field(default_factory=list)
    baseline_values: Dict[str, Dict[str, float]] = field(default_factory=dict)  # baseline_name -> {metric_name: value}
    improvement_text: str = ""


class SyntheticDataGenerator:
    """
    가설과 도메인에 맞는 합리적인 실험 데이터 생성
    """
    
    # 도메인별 메트릭 (이름, 범위, higher_is_better, 단위)
    METRIC_DEFINITIONS = {
        # Machine Learning
        "Accuracy": ((0.88, 0.97), True, "%"),
        "Precision": ((0.85, 0.96), True, ""),
        "Recall": ((0.83, 0.95), True, ""),
        "F1-Score": ((0.84, 0.95), True, ""),
        "AUC-ROC": ((0.90, 0.99), True, ""),
        "AUC": ((0.88, 0.98), True, ""),
        
        # Lower is better metrics
        "MSE": ((0.001, 0.02), False, ""),
        "RMSE": ((0.02, 0.08), False, ""),
        "MAE": ((0.015, 0.06), False, ""),
        "Perplexity": ((12, 35), False, ""),
        "FID": ((8, 40), False, ""),
        
        # Communication metrics
        "BER": ((0.0001, 0.005), False, ""),  # Bit Error Rate - LOWER is better
        "SNR": ((18, 32), True, "dB"),
        "Throughput": ((150, 800), True, "Mbps"),
        "Latency": ((2, 30), False, "ms"),  # LOWER is better
        "Capacity": ((8, 18), True, "bits/s/Hz"),
        
        # Vision metrics
        "mAP": ((0.78, 0.94), True, ""),
        "IoU": ((0.72, 0.91), True, ""),
        "FPS": ((35, 100), True, ""),
        
        # NLP metrics
        "BLEU": ((0.38, 0.62), True, ""),
        "ROUGE-L": ((0.42, 0.68), True, ""),
        
        # Medical metrics
        "Sensitivity": ((0.88, 0.97), True, ""),
        "Specificity": ((0.90, 0.98), True, ""),
    }
    
    # 도메인별 메트릭 리스트
    DOMAIN_METRICS = {
        "Machine Learning": ["Accuracy", "Precision", "Recall", "F1-Score"],
        "Computer Vision": ["Accuracy", "mAP", "IoU", "FPS"],
        "NLP": ["BLEU", "ROUGE-L", "Perplexity", "F1-Score"],
        "통신": ["BER", "SNR", "Throughput", "Capacity"],
        "의료": ["Sensitivity", "Specificity", "AUC", "Precision"],
    }
    
    # 키워드 기반 문맥 인식 메트릭 매핑 (가설에 직접 연결된 구체적 지표)
    KEYWORD_METRIC_MAPPING = {
        # 우주 기상 관련
        "space weather": [
            ("GPS Position Error", (0.5, 3.0), False, "m"),  # 낮을수록 좋음
            ("Geomagnetic Storm Prediction Accuracy", (0.85, 0.96), True, ""),
            ("Power Grid Load Forecast RMSE", (0.02, 0.08), False, "MW"),
            ("Satellite Communication Availability", (0.95, 0.995), True, ""),
        ],
        "gps": [
            ("Positioning Error Reduction", (15, 45), True, "%"),
            ("Signal Reacquisition Time", (0.5, 3.0), False, "sec"),
            ("GDOP Improvement", (0.8, 2.5), False, ""),
        ],
        "smart office": [
            ("HVAC Energy Savings", (12, 28), True, "%"),
            ("Occupancy Prediction Accuracy", (0.88, 0.96), True, ""),
            ("Peak Load Reduction", (8, 22), True, "%"),
        ],
        "building": [
            ("Energy Consumption Reduction", (10, 25), True, "%"),
            ("Thermal Comfort Index", (0.85, 0.95), True, ""),
            ("CO2 Level Prediction MAE", (15, 50), False, "ppm"),
        ],
        # 통신 관련
        "irs": [
            ("IRS Phase Optimization Convergence", (50, 200), False, "iterations"),
            ("Received Signal Power Gain", (3, 12), True, "dB"),
            ("Coverage Extension", (15, 40), True, "%"),
        ],
        "mimo": [
            ("Spectral Efficiency", (5, 15), True, "bits/s/Hz"),
            ("Beam Alignment Accuracy", (0.92, 0.99), True, ""),
            ("Inter-user Interference Reduction", (8, 20), True, "dB"),
        ],
        "covert": [
            ("Detection Probability", (0.001, 0.05), False, ""),  # 낮을수록 좋음
            ("Covert Rate", (0.5, 2.5), True, "bits/s/Hz"),
            ("Secrecy Capacity", (1.0, 5.0), True, "bits/s/Hz"),
        ],
        # 의료 관련
        "diagnosis": [
            ("Diagnostic Sensitivity", (0.90, 0.98), True, ""),
            ("False Positive Rate", (0.02, 0.10), False, ""),
            ("Time to Diagnosis", (5, 30), False, "min"),
        ],
        "cancer": [
            ("Tumor Detection Rate", (0.92, 0.99), True, ""),
            ("Lesion Localization Accuracy", (0.85, 0.95), True, ""),
            ("False Negative Rate", (0.01, 0.08), False, ""),
        ],
        # 자율주행/로봇
        "autonomous": [
            ("Obstacle Detection Rate", (0.95, 0.995), True, ""),
            ("Path Planning Efficiency", (0.88, 0.96), True, ""),
            ("Collision Avoidance Success Rate", (0.99, 0.9999), True, ""),
        ],
        "drone": [
            ("Flight Path Deviation", (0.1, 0.8), False, "m"),
            ("Battery Efficiency Improvement", (8, 20), True, "%"),
            ("Object Tracking Accuracy", (0.90, 0.98), True, ""),
        ],
    }
    
    # 도메인별 베이스라인 정의 (실제 알고리즘명 사용, 개념적 중복 방지)
    DOMAIN_BASELINES = {
        "Machine Learning": [
            BaselineInfo("SVM Baseline", "Support Vector Machine with RBF kernel (Cortes & Vapnik, 1995)", "SVM"),
            BaselineInfo("Random Forest", "Ensemble of 100 decision trees (Breiman, 2001)", "RF"),
            BaselineInfo("XGBoost", "Gradient boosting classifier (Chen & Guestrin, 2016)", "XGBoost"),
        ],
        "통신": [
            BaselineInfo("OFDM-MIMO", "Orthogonal Frequency Division Multiplexing without IRS (IEEE 802.11n)", "OFDM"),
            BaselineInfo("ZF Beamforming", "Zero-Forcing precoding for interference cancellation", "ZF"),
            BaselineInfo("MMSE Receiver", "Minimum Mean Square Error detection (Kay, 1993)", "MMSE"),
        ],
        "Computer Vision": [
            BaselineInfo("ResNet-50", "Deep residual network with 50 layers (He et al., 2016)", "ResNet-50"),
            BaselineInfo("EfficientNet-B0", "Compound scaling CNN (Tan & Le, 2019)", "EfficientNet"),
            BaselineInfo("ViT-Base", "Vision Transformer base model (Dosovitskiy et al., 2021)", "ViT"),
        ],
        "NLP": [
            BaselineInfo("BERT-base", "Bidirectional encoder with 12 layers (Devlin et al., 2019)", "BERT"),
            BaselineInfo("RoBERTa", "Robustly optimized BERT (Liu et al., 2019)", "RoBERTa"),
            BaselineInfo("T5-small", "Text-to-Text Transfer Transformer (Raffel et al., 2020)", "T5"),
        ],
        "의료": [
            BaselineInfo("U-Net", "Encoder-decoder for medical image segmentation (Ronneberger, 2015)", "U-Net"),
            BaselineInfo("DenseNet-121", "Dense connections for feature reuse (Huang et al., 2017)", "DenseNet"),
            BaselineInfo("Radiologist Consensus", "Average of 3 board-certified radiologists", "Human"),
        ],
    }
    
    # 도메인별 실제 알고리즘 풀 (제안 방법명으로 사용)
    PROPOSED_ALGORITHMS = {
        "Machine Learning": [
            "Attention-Augmented Neural Network",
            "Hybrid Ensemble with Self-Training",
            "Adaptive Regularization Network (ARN)",
        ],
        "통신": [
            "Alternating Optimization (AO) based IRS Control",
            "Deep Reinforcement Learning for Phase Shift",
            "Successive Convex Approximation (SCA) Method",
        ],
        "Computer Vision": [
            "Multi-Scale Feature Pyramid Network",
            "Attention-Guided Region Proposal",
            "Cross-Layer Feature Aggregation Module",
        ],
        "NLP": [
            "Context-Aware Attention Mechanism",
            "Hierarchical Document Encoder",
            "Multi-Task Pre-training Framework",
        ],
        "의료": [
            "Dual-Attention U-Net",
            "Multi-Scale Residual Learning",
            "Uncertainty-Aware Segmentation Network",
        ],
    }
    
    # 도메인별 인과관계 설명 템플릿 (블랙박스 방지)
    CAUSAL_MECHANISMS = {
        "Machine Learning": [
            "The attention mechanism allows the model to focus on relevant features, thereby improving prediction accuracy.",
            "By leveraging ensemble learning, we reduce variance and increase robustness against noisy samples.",
            "The regularization term prevents overfitting by penalizing large weights, leading to better generalization.",
        ],
        "통신": [
            "The IRS reflects signals with optimized phase shifts, which constructively combines multipath components and increases received signal strength.",
            "By alternating between optimizing transmit beamforming and IRS phase configuration, we avoid local optima and improve overall capacity.",
            "Power allocation based on channel state information ensures resources are directed to users with favorable channel conditions.",
        ],
        "Computer Vision": [
            "Multi-scale feature extraction captures both fine-grained details and global context, enabling accurate detection across object sizes.",
            "Skip connections preserve spatial information lost during downsampling, improving segmentation boundary accuracy.",
            "Attention modules suppress background noise and highlight salient regions, reducing false positives.",
        ],
        "NLP": [
            "Self-attention enables the model to capture long-range dependencies between words, overcoming RNN limitations.",
            "Pre-training on large corpora provides rich contextual representations, which transfer well to downstream tasks.",
            "Multi-task learning shares representations across related tasks, improving sample efficiency and performance.",
        ],
        "의료": [
            "The encoder-decoder structure first compresses features to extract semantics, then reconstructs spatial details for precise segmentation.",
            "Dense connections reuse features from all preceding layers, improving gradient flow and reducing parameter count.",
            "Uncertainty quantification identifies ambiguous regions, allowing clinicians to focus on areas requiring expert judgment.",
        ],
    }
    
    def __init__(self, seed: int = None):
        if seed:
            random.seed(seed)
    
    def _find_contextual_metrics(
        self,
        keywords: List[str],
        hypothesis_title: str = ""
    ) -> List[Tuple[str, Tuple[float, float], bool, str]]:
        """
        키워드와 가설 제목에서 문맥 인식 메트릭 탐색
        
        Returns:
            매칭된 메트릭 리스트: [(name, range, higher_is_better, unit), ...]
        """
        # 검색할 텍스트 (소문자로 통일)
        search_text = " ".join(keywords).lower() + " " + hypothesis_title.lower()
        
        matched_metrics = []
        
        # KEYWORD_METRIC_MAPPING에서 매칭되는 키워드 찾기
        for keyword, metrics in self.KEYWORD_METRIC_MAPPING.items():
            if keyword.lower() in search_text:
                matched_metrics.extend(metrics)
        
        # 중복 제거 (이름 기준)
        seen_names = set()
        unique_metrics = []
        for metric in matched_metrics:
            if metric[0] not in seen_names:
                seen_names.add(metric[0])
                unique_metrics.append(metric)
        
        return unique_metrics
    
    def generate_metrics(
        self,
        domain: str,
        hypothesis_keywords: List[str] = None,
        num_metrics: int = 4,
        hypothesis_title: str = ""
    ) -> ExperimentMetrics:
        """도메인과 키워드에 맞는 문맥 인식 메트릭 생성"""
        
        hypothesis_keywords = hypothesis_keywords or []
        
        # 1. 키워드 기반 문맥 인식 메트릭 우선 탐색
        contextual_metrics = self._find_contextual_metrics(hypothesis_keywords, hypothesis_title)
        
        if contextual_metrics:
            # 키워드 매칭된 문맥 메트릭 사용
            metrics_to_use = contextual_metrics[:num_metrics]
        else:
            # 폴백: 도메인 기본 메트릭 사용
            metric_names = self.DOMAIN_METRICS.get(domain, self.DOMAIN_METRICS["Machine Learning"])[:num_metrics]
            metrics_to_use = [(name, self.METRIC_DEFINITIONS.get(name, ((0.85, 0.95), True, "")))
                             for name in metric_names]
            # 튜플 형식 맞추기
            metrics_to_use = [(name, defn[0], defn[1], defn[2] if len(defn) > 2 else "") 
                             for name, defn in metrics_to_use]
        
        # 베이스라인 선택
        baselines = self.DOMAIN_BASELINES.get(domain, self.DOMAIN_BASELINES["Machine Learning"])[:3]
        
        # Primary 메트릭 생성
        pm_name, pm_range, pm_higher, pm_unit = metrics_to_use[0]
        primary_value = round(random.uniform(*pm_range), 4)
        primary_std = round(random.uniform(0.005, 0.015), 4)
        
        primary_metric = MetricInfo(
            name=pm_name,
            value=primary_value,
            std=primary_std,
            higher_is_better=pm_higher,
            unit=pm_unit
        )
        
        # Secondary 메트릭 생성
        secondary_metrics = []
        for metric_info in metrics_to_use[1:]:
            m_name, m_range, m_higher, m_unit = metric_info
            value = round(random.uniform(*m_range), 4)
            std = round(random.uniform(0.005, 0.02), 4)
            
            secondary_metrics.append(MetricInfo(
                name=m_name,
                value=value,
                std=std,
                higher_is_better=m_higher,
                unit=m_unit
            ))
        
        # 베이스라인 값 생성 (Proposed보다 성능이 낮도록)
        baseline_values = {}
        for baseline in baselines:
            baseline_values[baseline.short_name] = {}
            
            # Primary metric
            if primary_metric.higher_is_better:
                # 높을수록 좋음 -> baseline은 낮은 값
                bl_value = round(primary_value * random.uniform(0.88, 0.95), 4)
            else:
                # 낮을수록 좋음 -> baseline은 높은 값
                bl_value = round(primary_value * random.uniform(1.05, 1.15), 4)
            baseline_values[baseline.short_name][pm_name] = bl_value
            
            # Secondary metrics
            for sm in secondary_metrics:
                if sm.higher_is_better:
                    bl_val = round(sm.value * random.uniform(0.88, 0.95), 4)
                else:
                    bl_val = round(sm.value * random.uniform(1.05, 1.15), 4)
                baseline_values[baseline.short_name][sm.name] = bl_val
        
        # 개선 텍스트 생성
        improvement_text = self._generate_improvement_text(primary_metric, baselines[0], baseline_values)
        
        return ExperimentMetrics(
            primary_metric=primary_metric,
            secondary_metrics=secondary_metrics,
            baselines=baselines,
            baseline_values=baseline_values,
            improvement_text=improvement_text
        )
    
    def _generate_improvement_text(
        self,
        primary: MetricInfo,
        main_baseline: BaselineInfo,
        baseline_values: Dict[str, Dict[str, float]]
    ) -> str:
        """개선율 텍스트 생성 (극성 인식)"""
        bl_value = baseline_values[main_baseline.short_name][primary.name]
        
        if primary.higher_is_better:
            # 높을수록 좋음: (proposed - baseline) / baseline * 100
            improvement = ((primary.value - bl_value) / bl_value) * 100
            direction = "improvement"
        else:
            # 낮을수록 좋음: (baseline - proposed) / baseline * 100
            improvement = ((bl_value - primary.value) / bl_value) * 100
            direction = "reduction"
        
        return f"{abs(improvement):.1f}% {direction} in {primary.name} compared to {main_baseline.name}"
    
    def generate_table_data(self, metrics: ExperimentMetrics) -> Dict[str, Any]:
        """비교 테이블 데이터 생성"""
        
        # 헤더 구성
        headers = ["Method", metrics.primary_metric.name]
        for sm in metrics.secondary_metrics:
            header = sm.name
            if sm.unit:
                header += f" ({sm.unit})"
            headers.append(header)
        
        rows = []
        
        # Proposed Method (best)
        row = ["\\textbf{Proposed}"]
        pm = metrics.primary_metric
        
        # Primary metric - bold 처리
        if pm.higher_is_better:
            row.append(f"\\textbf{{{pm.value:.4f}}} $\\pm$ {pm.std:.3f}")
        else:
            row.append(f"\\textbf{{{pm.value:.4f}}} $\\pm$ {pm.std:.3f}")
        
        # Secondary metrics
        for sm in metrics.secondary_metrics:
            row.append(f"\\textbf{{{sm.value:.4f}}} $\\pm$ {sm.std:.3f}")
        
        rows.append(row)
        
        # Baseline methods
        for baseline in metrics.baselines:
            row = [baseline.short_name]
            
            # Primary metric
            bl_val = metrics.baseline_values[baseline.short_name][pm.name]
            row.append(f"{bl_val:.4f}")
            
            # Secondary metrics
            for sm in metrics.secondary_metrics:
                bl_val = metrics.baseline_values[baseline.short_name][sm.name]
                row.append(f"{bl_val:.4f}")
            
            rows.append(row)
        
        return {"headers": headers, "rows": rows}
    
    def format_results_text(self, metrics: ExperimentMetrics) -> str:
        """Results 섹션용 텍스트 생성 (극성 인식)"""
        pm = metrics.primary_metric
        
        text = f"Our proposed method achieved a {pm.name} of {pm.value:.4f} ($\\pm${pm.std:.3f})"
        if pm.unit:
            text += f" {pm.unit}"
        text += ". "
        
        # Secondary metrics
        if metrics.secondary_metrics:
            parts = []
            for sm in metrics.secondary_metrics:
                part = f"{sm.name} of {sm.value:.4f}"
                if sm.unit:
                    part += f" {sm.unit}"
                parts.append(part)
            text += "Additionally, we achieved " + ", ".join(parts) + ". "
        
        # Baseline comparisons with correct polarity
        text += "\n\nCompared to baseline methods:\n"
        for baseline in metrics.baselines:
            bl_primary = metrics.baseline_values[baseline.short_name][pm.name]
            
            if pm.higher_is_better:
                improvement = ((pm.value - bl_primary) / bl_primary) * 100
                verb = "outperformed"
            else:
                improvement = ((bl_primary - pm.value) / bl_primary) * 100
                verb = "achieved lower"
            
            text += f"- {verb} {baseline.name} ({baseline.description}) by {improvement:.1f}\\%\n"
        
        return text
    
    def generate_baseline_definitions(self, metrics: ExperimentMetrics) -> str:
        """베이스라인 정의 섹션용 텍스트"""
        text = "We compare our proposed method with the following baseline approaches:\n\n"
        
        for i, baseline in enumerate(metrics.baselines, 1):
            text += f"\\textbf{{{baseline.short_name}}}: {baseline.description}.\n\n"
        
        return text
    
    def generate_latex_table(self, table_data: Dict[str, Any]) -> str:
        """LaTeX 테이블 코드 생성"""
        headers = table_data["headers"]
        rows = table_data["rows"]
        
        col_format = "|l|" + "c|" * (len(headers) - 1)
        
        latex = f"""\\begin{{table}}[h]
\\centering
\\caption{{Comparison of experimental results. Best results are shown in bold.}}
\\label{{tab:results}}
\\begin{{tabular}}{{{col_format}}}
\\hline
{" & ".join(headers)} \\\\
\\hline
"""
        
        for row in rows:
            latex += " & ".join(row) + " \\\\\n"
        
        latex += """\\hline
\\end{tabular}
\\end{table}"""
        
        return latex
    
    def generate_causal_explanation(self, domain: str) -> str:
        """도메인별 인과관계 설명 생성"""
        import random
        mechanisms = self.CAUSAL_MECHANISMS.get(domain, self.CAUSAL_MECHANISMS["Machine Learning"])
        return random.choice(mechanisms)
    
    def get_proposed_algorithm(self, domain: str) -> str:
        """도메인별 제안 알고리즘명 반환"""
        import random
        algorithms = self.PROPOSED_ALGORITHMS.get(domain, self.PROPOSED_ALGORITHMS["Machine Learning"])
        return random.choice(algorithms)
    
    def generate_methodology_context(self, domain: str) -> str:
        """방법론 섹션용 컨텍스트 생성"""
        proposed = self.get_proposed_algorithm(domain)
        causal = self.generate_causal_explanation(domain)
        
        return f"""
Proposed Method: {proposed}

Technical Explanation:
{causal}

This method addresses the limitations of existing approaches by providing a principled optimization framework.
"""

