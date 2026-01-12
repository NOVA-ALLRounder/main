import requests
import xml.etree.ElementTree as ET
from typing import List, Dict, Optional
import os

class LawApiClient:
    BASE_URL = "http://www.law.go.kr/DRF/lawSearch.do"
    
    def __init__(self, api_key: Optional[str] = None):
        self.api_key = api_key or os.getenv("LAW_API_KEY")
        self.user_id = os.getenv("LAW_USER_ID", "test") # Default ID often required
        
    def search_law(self, query: str) -> List[Dict]:
        """
        Search for laws by query string.
        Returns a list of law metadata (ID, Name, Link).
        """
        if not self.api_key:
            print("Warning: LAW_API_KEY is not set. Returning mock data.")
            return self._mock_search_results(query)

        params = {
            "OC": self.user_id,
            "target": "law",
            "type": "XML",
            "query": query,
            "key": self.api_key
        }
        
        try:
            response = requests.get(self.BASE_URL, params=params)
            response.raise_for_status()
            return self._parse_search_xml(response.text)
        except Exception as e:
            print(f"Error fetching law data: {e}")
            return []

    def get_law_detail(self, law_id: str) -> str:
        """
        Fetch the full text/structure of a specific law.
        """
        if not self.api_key:
            return self._mock_law_detail(law_id)
            
        url = "http://www.law.go.kr/DRF/lawService.do"
        params = {
            "OC": self.user_id,
            "target": "law",
            "MST": law_id,
            "type": "XML",
            "key": self.api_key
        }
        
        try:
            response = requests.get(url, params=params)
            response.raise_for_status()
            return response.text
        except Exception as e:
            print(f"Error fetching law detail: {e}")
            return ""

    def _parse_search_xml(self, xml_string: str) -> List[Dict]:
        root = ET.fromstring(xml_string)
        laws = []
        for law in root.findall(".//law"):
            laws.append({
                "id": law.findtext("lawId"),
                "name": law.findtext("lawNm"),
                "link": law.findtext("lawLink"),
                "date": law.findtext("promDt")
            })
        return laws

    def _mock_search_results(self, query: str) -> List[Dict]:
        # Mock data for testing without API Key
        mocks = [
            {"id": "001", "name": "법인세법", "link": "/law/법인세법", "date": "20240101"},
            {"id": "002", "name": "소득세법", "link": "/law/소득세법", "date": "20240101"},
            {"id": "003", "name": "부가가치세법", "link": "/law/부가가치세법", "date": "20240101"},
        ]
        return [m for m in mocks if query in m['name']]

    def _mock_law_detail(self, law_id: str) -> str:
        # Simple mock detail plain text
        if law_id == "001":
            return "제1조(목적) 이 법은 법인세의 과세전요건과 절차를 규정함으로써 국세수입을 원활하게 확보하고 공평과세에 이바지함을 목적으로 한다.\n제55조(세율) ① 법인세의 과세표준이 2억원 이하인 경우 세율은 100분의 9로 한다."
        return "법령 상세 내용을 찾을 수 없습니다."

if __name__ == "__main__":
    client = LawApiClient()
    results = client.search_law("법인세")
    print(f"Search Results: {results}")
