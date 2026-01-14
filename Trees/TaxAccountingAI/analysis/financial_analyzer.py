"""
재무제표 분석 모듈

주요 기능:
1. 재무비율 계산 (유동비율, 부채비율, ROE, ROA 등)
2. 업종 평균 대비 분석
3. 재무 트렌드 분석
4. 재무 건전성 진단
"""

from typing import Dict, List, Optional
from dataclasses import dataclass
import math

@dataclass
class FinancialData:
    """재무제표 기본 항목"""
    # 재무상태표 (Balance Sheet)
    current_assets: float = 0       # 유동자산
    non_current_assets: float = 0   # 비유동자산
    total_assets: float = 0         # 총자산
    current_liabilities: float = 0  # 유동부채
    non_current_liabilities: float = 0  # 비유동부채
    total_liabilities: float = 0    # 총부채
    equity: float = 0               # 자본총계
    
    # 손익계산서 (Income Statement)
    revenue: float = 0              # 매출액
    cost_of_sales: float = 0        # 매출원가
    gross_profit: float = 0         # 매출총이익
    operating_income: float = 0     # 영업이익
    net_income: float = 0           # 당기순이익
    
    # 기타
    interest_expense: float = 0     # 이자비용
    inventory: float = 0            # 재고자산
    receivables: float = 0          # 매출채권
    payables: float = 0             # 매입채무


class FinancialAnalyzer:
    """재무제표 분석기"""
    
    # 업종별 평균 비율 (표준 참고값)
    INDUSTRY_BENCHMARKS = {
        "startup": {
            "current_ratio": 1.5,
            "debt_ratio": 150,
            "roe": 15,
            "operating_margin": 10,
            "net_margin": 5
        },
        "hospital": {
            "current_ratio": 1.8,
            "debt_ratio": 100,
            "roe": 12,
            "operating_margin": 8,
            "net_margin": 6
        },
        "commerce": {
            "current_ratio": 1.3,
            "debt_ratio": 180,
            "roe": 20,
            "operating_margin": 5,
            "net_margin": 3
        },
        "general": {
            "current_ratio": 1.5,
            "debt_ratio": 150,
            "roe": 10,
            "operating_margin": 8,
            "net_margin": 5
        }
    }
    
    def __init__(self, data: FinancialData = None, industry: str = "general"):
        self.data = data
        self.industry = industry
        self.benchmark = self.INDUSTRY_BENCHMARKS.get(industry, self.INDUSTRY_BENCHMARKS["general"])
    
    def calculate_ratios(self) -> Dict:
        """주요 재무비율 계산"""
        if not self.data:
            return {}
        
        d = self.data
        ratios = {}
        
        # 1. 유동성 비율 (Liquidity Ratios)
        ratios["current_ratio"] = {
            "name": "유동비율",
            "value": round(self._safe_divide(d.current_assets, d.current_liabilities) * 100, 1),
            "unit": "%",
            "desc": "단기 채무 상환 능력",
            "benchmark": self.benchmark["current_ratio"] * 100,
            "interpretation": self._interpret_current_ratio(d.current_assets, d.current_liabilities)
        }
        
        ratios["quick_ratio"] = {
            "name": "당좌비율",
            "value": round(self._safe_divide(d.current_assets - d.inventory, d.current_liabilities) * 100, 1),
            "unit": "%",
            "desc": "재고 제외 즉시 상환 능력",
            "interpretation": "100% 이상이면 양호"
        }
        
        # 2. 안정성 비율 (Leverage Ratios)
        ratios["debt_ratio"] = {
            "name": "부채비율",
            "value": round(self._safe_divide(d.total_liabilities, d.equity) * 100, 1),
            "unit": "%",
            "desc": "재무 안정성",
            "benchmark": self.benchmark["debt_ratio"],
            "interpretation": self._interpret_debt_ratio(d.total_liabilities, d.equity)
        }
        
        ratios["equity_ratio"] = {
            "name": "자기자본비율",
            "value": round(self._safe_divide(d.equity, d.total_assets) * 100, 1),
            "unit": "%",
            "desc": "총자산 중 자기자본 비중"
        }
        
        # 3. 수익성 비율 (Profitability Ratios)
        ratios["gross_margin"] = {
            "name": "매출총이익률",
            "value": round(self._safe_divide(d.gross_profit, d.revenue) * 100, 1),
            "unit": "%",
            "desc": "매출 대비 총이익"
        }
        
        ratios["operating_margin"] = {
            "name": "영업이익률",
            "value": round(self._safe_divide(d.operating_income, d.revenue) * 100, 1),
            "unit": "%",
            "desc": "영업 효율성",
            "benchmark": self.benchmark["operating_margin"],
            "interpretation": self._interpret_operating_margin(d.operating_income, d.revenue)
        }
        
        ratios["net_margin"] = {
            "name": "순이익률",
            "value": round(self._safe_divide(d.net_income, d.revenue) * 100, 1),
            "unit": "%",
            "desc": "최종 수익성",
            "benchmark": self.benchmark["net_margin"]
        }
        
        ratios["roe"] = {
            "name": "자기자본이익률(ROE)",
            "value": round(self._safe_divide(d.net_income, d.equity) * 100, 1),
            "unit": "%",
            "desc": "투자 수익성",
            "benchmark": self.benchmark["roe"],
            "interpretation": self._interpret_roe(d.net_income, d.equity)
        }
        
        ratios["roa"] = {
            "name": "총자산이익률(ROA)",
            "value": round(self._safe_divide(d.net_income, d.total_assets) * 100, 1),
            "unit": "%",
            "desc": "자산 활용 효율"
        }
        
        # 4. 활동성 비율 (Efficiency Ratios)
        ratios["asset_turnover"] = {
            "name": "총자산회전율",
            "value": round(self._safe_divide(d.revenue, d.total_assets), 2),
            "unit": "회",
            "desc": "자산 활용도"
        }
        
        ratios["receivables_turnover"] = {
            "name": "매출채권회전율",
            "value": round(self._safe_divide(d.revenue, d.receivables), 2),
            "unit": "회",
            "desc": "채권 회수 속도"
        }
        
        return ratios
    
    def get_health_score(self) -> Dict:
        """재무 건전성 점수 (100점 만점)"""
        ratios = self.calculate_ratios()
        
        score = 0
        details = []
        
        # 유동비율 (25점)
        cr = ratios.get("current_ratio", {}).get("value", 0)
        if cr >= 200:
            score += 25
            details.append({"item": "유동비율", "score": 25, "status": "excellent"})
        elif cr >= 150:
            score += 20
            details.append({"item": "유동비율", "score": 20, "status": "good"})
        elif cr >= 100:
            score += 15
            details.append({"item": "유동비율", "score": 15, "status": "warning"})
        else:
            score += 5
            details.append({"item": "유동비율", "score": 5, "status": "critical"})
        
        # 부채비율 (25점)
        dr = ratios.get("debt_ratio", {}).get("value", 0)
        if dr <= 100:
            score += 25
            details.append({"item": "부채비율", "score": 25, "status": "excellent"})
        elif dr <= 150:
            score += 20
            details.append({"item": "부채비율", "score": 20, "status": "good"})
        elif dr <= 200:
            score += 15
            details.append({"item": "부채비율", "score": 15, "status": "warning"})
        else:
            score += 5
            details.append({"item": "부채비율", "score": 5, "status": "critical"})
        
        # 영업이익률 (25점)
        om = ratios.get("operating_margin", {}).get("value", 0)
        if om >= 15:
            score += 25
            details.append({"item": "영업이익률", "score": 25, "status": "excellent"})
        elif om >= 10:
            score += 20
            details.append({"item": "영업이익률", "score": 20, "status": "good"})
        elif om >= 5:
            score += 15
            details.append({"item": "영업이익률", "score": 15, "status": "warning"})
        else:
            score += 5
            details.append({"item": "영업이익률", "score": 5, "status": "critical"})
        
        # ROE (25점)
        roe = ratios.get("roe", {}).get("value", 0)
        if roe >= 15:
            score += 25
            details.append({"item": "ROE", "score": 25, "status": "excellent"})
        elif roe >= 10:
            score += 20
            details.append({"item": "ROE", "score": 20, "status": "good"})
        elif roe >= 5:
            score += 15
            details.append({"item": "ROE", "score": 15, "status": "warning"})
        else:
            score += 5
            details.append({"item": "ROE", "score": 5, "status": "critical"})
        
        # 등급 판정
        if score >= 90:
            grade = "A+"
            grade_desc = "매우 우수"
        elif score >= 80:
            grade = "A"
            grade_desc = "우수"
        elif score >= 70:
            grade = "B+"
            grade_desc = "양호"
        elif score >= 60:
            grade = "B"
            grade_desc = "보통"
        elif score >= 50:
            grade = "C"
            grade_desc = "주의"
        else:
            grade = "D"
            grade_desc = "위험"
        
        return {
            "total_score": score,
            "max_score": 100,
            "grade": grade,
            "grade_desc": grade_desc,
            "details": details,
            "industry": self.industry
        }
    
    def compare_to_benchmark(self) -> List[Dict]:
        """업종 평균 대비 비교"""
        ratios = self.calculate_ratios()
        comparisons = []
        
        key_ratios = ["current_ratio", "debt_ratio", "operating_margin", "roe"]
        
        for key in key_ratios:
            if key in ratios and "benchmark" in ratios[key]:
                value = ratios[key]["value"]
                benchmark = ratios[key]["benchmark"]
                diff = value - benchmark
                
                # 부채비율은 낮을수록 좋음
                if key == "debt_ratio":
                    status = "better" if value < benchmark else "worse" if value > benchmark * 1.2 else "similar"
                else:
                    status = "better" if value > benchmark else "worse" if value < benchmark * 0.8 else "similar"
                
                comparisons.append({
                    "name": ratios[key]["name"],
                    "value": value,
                    "benchmark": benchmark,
                    "diff": round(diff, 1),
                    "status": status,
                    "unit": ratios[key]["unit"]
                })
        
        return comparisons
    
    def get_recommendations(self) -> List[Dict]:
        """재무 개선 권고사항"""
        ratios = self.calculate_ratios()
        recommendations = []
        
        # 유동비율 분석
        cr = ratios.get("current_ratio", {}).get("value", 0)
        if cr < 100:
            recommendations.append({
                "priority": "high",
                "category": "유동성",
                "issue": "유동비율이 100% 미만입니다",
                "recommendation": "단기 채무 상환 능력 부족. 유동자산 확보 또는 단기 부채 감축 필요.",
                "impact": "부도 위험 증가"
            })
        
        # 부채비율 분석
        dr = ratios.get("debt_ratio", {}).get("value", 0)
        if dr > 200:
            recommendations.append({
                "priority": "high",
                "category": "안정성",
                "issue": "부채비율이 200%를 초과합니다",
                "recommendation": "과도한 부채로 재무 위험 증가. 자본 확충 또는 부채 상환 필요.",
                "impact": "이자 부담 증가, 추가 대출 어려움"
            })
        
        # 영업이익률 분석
        om = ratios.get("operating_margin", {}).get("value", 0)
        if om < 5:
            recommendations.append({
                "priority": "medium",
                "category": "수익성",
                "issue": "영업이익률이 5% 미만입니다",
                "recommendation": "원가 절감 또는 판매가 인상 검토. 운영 효율화 필요.",
                "impact": "장기적 성장 제한"
            })
        
        # ROE 분석
        roe = ratios.get("roe", {}).get("value", 0)
        if roe < 5:
            recommendations.append({
                "priority": "medium",
                "category": "투자효율",
                "issue": "ROE가 5% 미만입니다",
                "recommendation": "자기자본 대비 수익성 개선 필요. 비효율 자산 정리 또는 수익 구조 개선.",
                "impact": "투자자 매력도 감소"
            })
        
        if not recommendations:
            recommendations.append({
                "priority": "info",
                "category": "종합",
                "issue": "재무 건전성 양호",
                "recommendation": "현재 재무 상태가 양호합니다. 성장 투자 검토 가능.",
                "impact": "긍정적"
            })
        
        return recommendations
    
    def _safe_divide(self, a: float, b: float) -> float:
        if b == 0 or b is None:
            return 0
        return a / b
    
    def _interpret_current_ratio(self, assets, liabilities) -> str:
        ratio = self._safe_divide(assets, liabilities)
        if ratio >= 2:
            return "매우 양호 - 단기 채무 상환 능력 충분"
        elif ratio >= 1.5:
            return "양호 - 적정 수준"
        elif ratio >= 1:
            return "주의 - 유동성 관리 필요"
        else:
            return "위험 - 단기 자금 부족 우려"
    
    def _interpret_debt_ratio(self, liabilities, equity) -> str:
        ratio = self._safe_divide(liabilities, equity) * 100
        if ratio <= 100:
            return "매우 양호 - 재무 안정성 우수"
        elif ratio <= 150:
            return "양호 - 적정 수준"
        elif ratio <= 200:
            return "주의 - 부채 관리 필요"
        else:
            return "위험 - 과다 부채 상태"
    
    def _interpret_operating_margin(self, income, revenue) -> str:
        ratio = self._safe_divide(income, revenue) * 100
        if ratio >= 15:
            return "매우 양호 - 우수한 수익성"
        elif ratio >= 10:
            return "양호 - 적정 수준"
        elif ratio >= 5:
            return "보통 - 개선 여지 있음"
        else:
            return "주의 - 수익성 개선 필요"
    
    def _interpret_roe(self, income, equity) -> str:
        ratio = self._safe_divide(income, equity) * 100
        if ratio >= 15:
            return "매우 양호 - 효율적인 자본 활용"
        elif ratio >= 10:
            return "양호 - 적정 수준"
        elif ratio >= 5:
            return "보통 - 개선 여지 있음"
        else:
            return "주의 - 자본 효율 저하"


# 테스트 및 데모용
def create_sample_analysis(revenue: int = 500000000, industry: str = "startup") -> Dict:
    """샘플 재무 분석 생성 (데모용)"""
    
    # 매출 기반 추정치 생성
    data = FinancialData(
        revenue=revenue,
        cost_of_sales=int(revenue * 0.6),
        gross_profit=int(revenue * 0.4),
        operating_income=int(revenue * 0.12),
        net_income=int(revenue * 0.08),
        
        current_assets=int(revenue * 0.5),
        non_current_assets=int(revenue * 0.8),
        total_assets=int(revenue * 1.3),
        
        current_liabilities=int(revenue * 0.3),
        non_current_liabilities=int(revenue * 0.4),
        total_liabilities=int(revenue * 0.7),
        equity=int(revenue * 0.6),
        
        inventory=int(revenue * 0.1),
        receivables=int(revenue * 0.15),
        payables=int(revenue * 0.12)
    )
    
    analyzer = FinancialAnalyzer(data, industry)
    
    return {
        "ratios": analyzer.calculate_ratios(),
        "health_score": analyzer.get_health_score(),
        "benchmark_comparison": analyzer.compare_to_benchmark(),
        "recommendations": analyzer.get_recommendations()
    }


if __name__ == "__main__":
    # 테스트 실행
    result = create_sample_analysis(500000000, "startup")
    
    print("\n=== 재무비율 분석 ===")
    for key, ratio in result["ratios"].items():
        print(f"{ratio['name']}: {ratio['value']}{ratio['unit']}")
    
    print(f"\n=== 재무 건전성 점수 ===")
    hs = result["health_score"]
    print(f"총점: {hs['total_score']}/{hs['max_score']} (등급: {hs['grade']} - {hs['grade_desc']})")
    
    print(f"\n=== 업종 평균 대비 ===")
    for comp in result["benchmark_comparison"]:
        print(f"{comp['name']}: {comp['value']}{comp['unit']} (업종평균: {comp['benchmark']}{comp['unit']}) - {comp['status']}")
    
    print(f"\n=== 개선 권고사항 ===")
    for rec in result["recommendations"]:
        print(f"[{rec['priority']}] {rec['issue']}")
        print(f"  → {rec['recommendation']}")
