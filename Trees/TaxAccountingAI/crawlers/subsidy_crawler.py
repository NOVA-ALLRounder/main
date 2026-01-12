import requests
from bs4 import BeautifulSoup
from typing import List, Dict
import random
from datetime import datetime, timedelta

class SubsidyCrawler:
    """
    Crawler for K-Startup and Bizinfo.
    Currently implements a mock strategy due to anti-bot protections on real sites,
    but structured to accept a real crawling logic.
    """
    
    def fetch_kstartup_notices(self, pages=1) -> List[Dict]:
        """
        Simulate fetching notices from K-Startup.
        In a real implementation, this would use `requests` or `playwright` to parse
        https://www.k-startup.go.kr/web/contents/bizpbanc-ongoing.do
        """
        print(f"Crawling K-Startup (Simulated)... Pages: {pages}")
        
        # Simulated data based on the technical report's examples
        mock_data = [
            {
                "title": "2025년 초기창업패키지 창업기업 모집공고",
                "org": "창업진흥원",
                "start_date": "2025-02-20",
                "end_date": "2025-03-15",
                "link": "https://www.k-startup.go.kr/example/1",
                "tags": ["창업", "지원금", "최대1억"]
            },
            {
                "title": "2025년 예비창업패키지 모집",
                "org": "중소벤처기업부",
                "start_date": "2025-02-22",
                "end_date": "2025-03-20",
                "link": "https://www.k-startup.go.kr/example/2",
                "tags": ["예비창업", "바우처", "평균5천"]
            },
            {
                "title": "R&D 역량강화 지원사업 공고",
                "org": "테크노파크",
                "start_date": "2025-01-15",
                "end_date": "2025-02-15",
                "link": "https://www.k-startup.go.kr/example/3",
                "tags": ["R&D", "기술개발"]
            }
        ]
        
        return mock_data

    def fetch_bizinfo_notices(self) -> List[Dict]:
        """
        Simulate fetching from Bizinfo (Enterprise Madang).
        """
        print("Crawling Bizinfo (Simulated)...")
        return [
            {
                "title": "2025년 정책자금 융자 공고",
                "org": "중소벤처기업진흥공단",
                "start_date": "2025-01-08",
                "end_date": "2025-12-31",
                "link": "https://www.bizinfo.go.kr/example/1",
                "tags": ["융자", "정책자금"]
            }
        ]

if __name__ == "__main__":
    crawler = SubsidyCrawler()
    notices = crawler.fetch_kstartup_notices()
    print("Found Notices:")
    for n in notices:
        print(f"- [{n['start_date']}] {n['title']} ({n['org']})")
