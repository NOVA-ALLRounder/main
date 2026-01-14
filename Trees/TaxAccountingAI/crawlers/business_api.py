"""
공공데이터 사업자등록정보 조회 API 연동 모듈

국세청 사업자등록정보 진위확인 및 상태조회 서비스 활용
- API: data.go.kr (공공데이터포털)
- 서비스명: 국세청_사업자등록정보 진위확인 및 상태조회 서비스

무료 API 키 발급: https://www.data.go.kr/data/15081808/openapi.do
"""

import requests
from typing import Dict, Optional
import os

class BusinessInfoAPI:
    """
    공공데이터 사업자정보 조회 API 클라이언트
    """
    
    # 공공데이터 API 엔드포인트
    STATUS_API_URL = "https://api.odcloud.kr/api/nts-businessman/v1/status"
    VALIDATE_API_URL = "https://api.odcloud.kr/api/nts-businessman/v1/validate"
    
    def __init__(self, api_key: str = None):
        """
        API 키 설정
        - 환경변수: DATA_GO_KR_API_KEY
        - 또는 생성자에 직접 전달
        """
        self.api_key = api_key or os.getenv("DATA_GO_KR_API_KEY", "")
        
    def lookup_business(self, biz_num: str) -> Dict:
        """
        사업자등록번호로 사업자 정보 조회
        
        Args:
            biz_num: 사업자등록번호 (숫자만 또는 XXX-XX-XXXXX 형식)
            
        Returns:
            {
                "success": bool,
                "data": {
                    "b_no": "사업자등록번호",
                    "b_stt": "계속사업자/휴업자/폐업자",
                    "b_stt_cd": "01/02/03",
                    "tax_type": "일반/간이/면세사업자",
                    "tax_type_cd": "01/02/03",
                    "end_dt": "폐업일자",
                    "utcc_yn": "단위과세자여부",
                    "tax_type_change_dt": "과세유형전환일자",
                    "invoice_apply_dt": "세금계산서적용일자"
                },
                "message": "조회 결과 메시지"
            }
        """
        # 하이픈 제거
        biz_num_clean = biz_num.replace("-", "").strip()
        
        if len(biz_num_clean) != 10:
            return {
                "success": False,
                "data": None,
                "message": "사업자등록번호는 10자리여야 합니다."
            }
        
        # API 키 확인
        if not self.api_key:
            # API 키 없으면 Mock 데이터 반환
            return self._mock_lookup(biz_num_clean)
        
        try:
            # 실제 API 호출
            headers = {
                "Content-Type": "application/json",
                "Accept": "application/json"
            }
            
            params = {
                "serviceKey": self.api_key
            }
            
            body = {
                "b_no": [biz_num_clean]
            }
            
            response = requests.post(
                self.STATUS_API_URL,
                headers=headers,
                params=params,
                json=body,
                timeout=10
            )
            
            if response.status_code == 200:
                result = response.json()
                
                if result.get("status_code") == "OK" and result.get("data"):
                    biz_data = result["data"][0]
                    
                    # 상태 코드 해석
                    status_map = {
                        "01": "계속사업자",
                        "02": "휴업자",
                        "03": "폐업자"
                    }
                    
                    tax_type_map = {
                        "01": "일반과세자",
                        "02": "간이과세자",
                        "03": "면세사업자",
                        "04": "비영리/공익단체",
                        "05": "국가/지자체",
                        "06": "간이과세자(세금계산서 발급사업자)"
                    }
                    
                    return {
                        "success": True,
                        "data": {
                            "b_no": biz_data.get("b_no", biz_num_clean),
                            "b_stt": status_map.get(biz_data.get("b_stt_cd", ""), "알수없음"),
                            "b_stt_cd": biz_data.get("b_stt_cd", ""),
                            "tax_type": tax_type_map.get(biz_data.get("tax_type_cd", ""), "알수없음"),
                            "tax_type_cd": biz_data.get("tax_type_cd", ""),
                            "end_dt": biz_data.get("end_dt", ""),
                            "utcc_yn": biz_data.get("utcc_yn", "N"),
                            "tax_type_change_dt": biz_data.get("tax_type_change_dt", ""),
                            "invoice_apply_dt": biz_data.get("invoice_apply_dt", "")
                        },
                        "message": "조회 성공"
                    }
                else:
                    return {
                        "success": False,
                        "data": None,
                        "message": result.get("msg", "조회 실패")
                    }
            else:
                return {
                    "success": False,
                    "data": None,
                    "message": f"API 오류: {response.status_code}"
                }
                
        except requests.exceptions.Timeout:
            return {
                "success": False,
                "data": None,
                "message": "API 응답 시간 초과"
            }
        except Exception as e:
            print(f"Business lookup error: {e}")
            return self._mock_lookup(biz_num_clean)
    
    def _mock_lookup(self, biz_num: str) -> Dict:
        """
        API 키 없을 때 Mock 데이터 반환 (데모용)
        """
        # 테스트용 사업자번호별 Mock 데이터
        mock_data = {
            "1234567890": {
                "b_no": "123-45-67890",
                "b_stt": "계속사업자",
                "b_stt_cd": "01",
                "tax_type": "일반과세자",
                "tax_type_cd": "01",
                "company_name": "(주)테스트스타트업",
                "industry": "소프트웨어 개발업"
            },
            "4567890123": {
                "b_no": "456-78-90123",
                "b_stt": "계속사업자",
                "b_stt_cd": "01",
                "tax_type": "일반과세자",
                "tax_type_cd": "01",
                "company_name": "테스트병원",
                "industry": "의원"
            },
            "7890123456": {
                "b_no": "789-01-23456",
                "b_stt": "계속사업자",
                "b_stt_cd": "01",
                "tax_type": "일반과세자",
                "tax_type_cd": "01",
                "company_name": "(주)테스트커머스",
                "industry": "전자상거래업"
            }
        }
        
        if biz_num in mock_data:
            return {
                "success": True,
                "data": mock_data[biz_num],
                "message": "[MOCK] 테스트 데이터입니다. 실제 API 연동 시 DATA_GO_KR_API_KEY 환경변수를 설정하세요."
            }
        else:
            # 입력된 번호로 랜덤 데이터 생성
            return {
                "success": True,
                "data": {
                    "b_no": f"{biz_num[:3]}-{biz_num[3:5]}-{biz_num[5:]}",
                    "b_stt": "계속사업자",
                    "b_stt_cd": "01",
                    "tax_type": "일반과세자",
                    "tax_type_cd": "01",
                    "company_name": f"사업자 {biz_num[-4:]}",
                    "industry": "기타 서비스업"
                },
                "message": "[MOCK] 테스트 데이터입니다."
            }
    
    def validate_business(self, biz_num: str, start_dt: str, p_nm: str, p_nm2: str = "", b_nm: str = "") -> Dict:
        """
        사업자등록정보 진위확인
        
        Args:
            biz_num: 사업자등록번호
            start_dt: 개업일자 (YYYYMMDD)
            p_nm: 대표자 성명
            p_nm2: 대표자 성명2 (공동대표)
            b_nm: 상호
            
        Returns:
            진위확인 결과
        """
        if not self.api_key:
            return {
                "success": False,
                "data": None,
                "message": "진위확인은 API 키가 필요합니다."
            }
        
        try:
            headers = {
                "Content-Type": "application/json",
                "Accept": "application/json"
            }
            
            params = {
                "serviceKey": self.api_key
            }
            
            body = {
                "businesses": [{
                    "b_no": biz_num.replace("-", ""),
                    "start_dt": start_dt,
                    "p_nm": p_nm,
                    "p_nm2": p_nm2,
                    "b_nm": b_nm
                }]
            }
            
            response = requests.post(
                self.VALIDATE_API_URL,
                headers=headers,
                params=params,
                json=body,
                timeout=10
            )
            
            if response.status_code == 200:
                result = response.json()
                if result.get("status_code") == "OK" and result.get("data"):
                    return {
                        "success": True,
                        "data": result["data"][0],
                        "message": "진위확인 완료"
                    }
            
            return {
                "success": False,
                "data": None,
                "message": "진위확인 실패"
            }
            
        except Exception as e:
            return {
                "success": False,
                "data": None,
                "message": f"오류: {str(e)}"
            }


# 테스트 코드
if __name__ == "__main__":
    api = BusinessInfoAPI()
    
    # 테스트 사업자번호 조회
    test_nums = ["123-45-67890", "456-78-90123", "999-99-99999"]
    
    for num in test_nums:
        print(f"\n조회: {num}")
        result = api.lookup_business(num)
        if result["success"]:
            print(f"  상태: {result['data']['b_stt']}")
            print(f"  과세유형: {result['data']['tax_type']}")
            if result['data'].get('company_name'):
                print(f"  상호: {result['data']['company_name']}")
        else:
            print(f"  실패: {result['message']}")
