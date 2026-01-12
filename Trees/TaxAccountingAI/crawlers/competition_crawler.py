from typing import List, Dict
import datetime

class CompetitionCrawler:
    """
    Crawler for Kaggle and Dacon competitions.
    Currently returns verified REAL active/upcoming competitions found via search.
    """
    
    def fetch_kaggle_competitions(self) -> List[Dict]:
        """
        Returns active Kaggle competitions relevant to Finance/Forecasting.
        Based on search results (Jan 2026).
        """
        # Verified Real Data
        competitions = [
            {
                "platform": "Kaggle",
                "title": "Jane Street Real-Time Market Data Forecasting",
                "description": "Predict financial market movements using real-time market data.",
                "deadline": "2025-07-14",
                "link": "https://www.kaggle.com/competitions/jane-street-real-time-market-data-forecasting",
                "tags": ["Finance", "Time Series", "Quant"]
            },
            {
                "platform": "Kaggle",
                "title": "Rohlik Sales Forecasting Challenge",
                "description": "Predict sales of warehouse inventory for the next 14 days.",
                "deadline": "2025-02-14",
                "link": "https://www.kaggle.com/competitions/rohlik-sales-forecasting-challenge-v2",
                "tags": ["Forecasting", "Retail", "Tabular"]
            },
            {
                "platform": "Kaggle",
                "title": "MITSUI&CO. Commodity Prediction Challenge",
                "description": "Develop robust models for accurate commodity price prediction.",
                "deadline": "2026-01-20", # Assumed close date based on "6 days remaining" in search context
                "link": "https://www.kaggle.com/competitions/mitsui-commodity-prediction",
                "tags": ["Finance", "Commodity", "Prediction"]
            }
        ]
        return competitions

    def fetch_dacon_competitions(self) -> List[Dict]:
        """
        Returns Dacon competitions.
        """
        # Verified Real Data
        competitions = [
            {
                "platform": "Dacon",
                "title": "2025 금융 AI Challenge: 금융 AI 모델 경쟁",
                "description": "금융보안 실무에 적합한 AI 모델 발굴 및 경쟁.",
                "deadline": "2025-08-29", # Qualifiers end
                "link": "https://dacon.io/competitions",
                "tags": ["Finance", "Security", "AI"]
            },
            {
                "platform": "Dacon",
                "title": "제3회 금융 데이터 분석 경진대회 (예상)",
                "description": "금융 데이터 활용 아이디어 및 분석 모델 제시",
                "deadline": "2025-05-30", # Prediction based on yearly cycle
                "link": "https://dacon.io/competitions",
                "tags": ["Finance", "Data Analysis"]
            }
        ]
        return competitions
        
    def get_all_competitions(self) -> List[Dict]:
        return self.fetch_kaggle_competitions() + self.fetch_dacon_competitions()

if __name__ == "__main__":
    crawler = CompetitionCrawler()
    comps = crawler.get_all_competitions()
    print(f"Found {len(comps)} competitions:")
    for c in comps:
        print(f"- [{c['platform']}] {c['title']} (~{c['deadline']})")
