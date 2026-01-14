"""
국세청 전자문서 PDF 파서 (Mac/Linux/Windows 호환)
- PyMuPDF(fitz)를 사용하여 PDF 내부의 XML 데이터 추출
- DLL 없이 순수 Python으로 동작
"""

import fitz  # PyMuPDF
import xml.etree.ElementTree as ET
from typing import Dict, Any, Optional, List
import re
from pathlib import Path


class NTSDocumentParser:
    """국세청 전자문서 PDF 파서"""
    
    def __init__(self, pdf_path: str = None, pdf_bytes: bytes = None, password: str = ""):
        """
        PDF 파일 초기화
        
        Args:
            pdf_path: PDF 파일 경로
            pdf_bytes: PDF 바이트 데이터
            password: PDF 비밀번호 (있는 경우)
        """
        self.password = password
        self.doc = None
        self.xml_data = None
        self.metadata = {}
        
        if pdf_bytes:
            self.doc = fitz.open(stream=pdf_bytes, filetype="pdf")
        elif pdf_path:
            self.doc = fitz.open(pdf_path)
        
        if self.doc and password:
            self.doc.authenticate(password)
    
    def verify_document(self) -> Dict[str, Any]:
        """
        전자문서 진본 검증 (기본 체크)
        실제 타임스탬프 검증은 DLL 필요
        """
        if not self.doc:
            return {"is_valid": False, "error": "문서가 로드되지 않았습니다."}
        
        # PDF 메타데이터 확인
        self.metadata = self.doc.metadata or {}
        
        # 국세청 문서 특성 확인
        is_nts_document = False
        producer = self.metadata.get("producer", "").lower()
        creator = self.metadata.get("creator", "").lower()
        
        if "국세청" in producer or "nts" in producer or "hometax" in creator:
            is_nts_document = True
        
        # 첨부파일 확인 (국세청 PDF는 XML 첨부 포함)
        embedded_files = self._get_embedded_files()
        has_xml = any(f.endswith('.xml') for f in embedded_files)
        
        return {
            "is_valid": True,
            "is_nts_document": is_nts_document or has_xml,
            "metadata": self.metadata,
            "page_count": self.doc.page_count,
            "embedded_files": embedded_files,
            "has_xml_data": has_xml
        }
    
    def _get_embedded_files(self) -> List[str]:
        """PDF 내 첨부 파일 목록"""
        if not self.doc:
            return []
        
        files = []
        try:
            # EmbeddedFiles 이름 트리에서 첨부파일 추출
            if hasattr(self.doc, 'embfile_names'):
                files = self.doc.embfile_names()
            
            # 또는 PDF 오브젝트에서 직접 탐색
            if not files:
                for i in range(len(self.doc)):
                    page = self.doc[i]
                    annots = page.annots()
                    if annots:
                        for annot in annots:
                            if annot.type[0] == 17:  # FileAttachment
                                files.append(annot.info.get("name", f"attachment_{i}"))
        except Exception as e:
            print(f"첨부파일 목록 추출 오류: {e}")
        
        return files
    
    def extract_xml_data(self) -> Optional[str]:
        """PDF에서 XML 데이터 추출"""
        if not self.doc:
            return None
        
        try:
            # 방법 1: 첨부파일에서 XML 추출
            if hasattr(self.doc, 'embfile_names'):
                for name in self.doc.embfile_names():
                    if name.lower().endswith('.xml'):
                        file_info = self.doc.embfile_info(name)
                        content = self.doc.embfile_get(name)
                        self.xml_data = content.decode('utf-8')
                        return self.xml_data
            
            # 방법 2: 페이지 텍스트에서 XML 패턴 추출
            for page_num in range(self.doc.page_count):
                page = self.doc[page_num]
                text = page.get_text()
                
                # XML 태그 패턴 검색
                xml_match = re.search(r'<\?xml.*?\?>', text, re.DOTALL)
                if xml_match:
                    # XML 시작부터 끝까지 추출 시도
                    start_idx = text.find('<?xml')
                    if start_idx >= 0:
                        self.xml_data = text[start_idx:]
                        return self.xml_data
            
            # 방법 3: 스트림 오브젝트에서 XML 탐색
            for xref in range(1, self.doc.xref_length()):
                try:
                    stream = self.doc.xref_stream(xref)
                    if stream and b'<?xml' in stream:
                        self.xml_data = stream.decode('utf-8', errors='ignore')
                        return self.xml_data
                except:
                    continue
            
        except Exception as e:
            print(f"XML 추출 오류: {e}")
        
        return None
    
    def parse_tax_data(self) -> Dict[str, Any]:
        """추출된 XML을 세무 데이터로 파싱"""
        if not self.xml_data:
            self.extract_xml_data()
        
        if not self.xml_data:
            # XML 추출 실패 시 텍스트 기반 파싱
            return self._parse_from_text()
        
        try:
            root = ET.fromstring(self.xml_data)
            
            # 문서 유형 판별
            doc_type = self._detect_document_type(root)
            
            if doc_type == "year_end_settlement":
                return self._parse_year_end_settlement(root)
            elif doc_type == "vat_return":
                return self._parse_vat_return(root)
            elif doc_type == "withholding_tax":
                return self._parse_withholding_tax(root)
            else:
                return self._parse_generic(root)
                
        except ET.ParseError as e:
            return {"error": f"XML 파싱 오류: {e}", "raw_xml": self.xml_data[:500]}
    
    def _parse_from_text(self) -> Dict[str, Any]:
        """PDF 텍스트에서 세무 정보 추출"""
        if not self.doc:
            return {"error": "문서 없음"}
        
        full_text = ""
        for page in self.doc:
            full_text += page.get_text()
        
        # 금액 패턴 추출
        amounts = re.findall(r'([가-힣]+)\s*[:：]?\s*([\d,]+)\s*원', full_text)
        
        result = {
            "document_type": "텍스트 기반 추출",
            "source": "pdf_text",
            "extracted_amounts": []
        }
        
        for label, amount in amounts[:20]:  # 상위 20개만
            try:
                result["extracted_amounts"].append({
                    "label": label,
                    "amount": int(amount.replace(',', ''))
                })
            except:
                pass
        
        return result
    
    def _detect_document_type(self, root: ET.Element) -> str:
        """XML 루트 요소로 문서 유형 판별"""
        tag = root.tag.lower()
        
        if '연말정산' in tag or 'yearend' in tag:
            return "year_end_settlement"
        elif '부가' in tag or 'vat' in tag:
            return "vat_return"
        elif '원천' in tag or 'withholding' in tag:
            return "withholding_tax"
        
        return "unknown"
    
    def _parse_year_end_settlement(self, root: ET.Element) -> Dict[str, Any]:
        """연말정산간소화 자료 파싱"""
        items = []
        total = 0
        
        for elem in root.iter():
            if '금액' in elem.tag or 'amount' in elem.tag.lower():
                try:
                    amount = int(elem.text.replace(',', ''))
                    items.append({
                        "category": elem.tag,
                        "amount": amount
                    })
                    total += amount
                except:
                    pass
        
        return {
            "document_type": "연말정산간소화 자료",
            "items": items,
            "total_amount": total
        }
    
    def _parse_vat_return(self, root: ET.Element) -> Dict[str, Any]:
        """부가가치세 신고서 파싱"""
        return {
            "document_type": "부가가치세 신고서",
            "elements": len(list(root.iter())),
            "parsed": True
        }
    
    def _parse_withholding_tax(self, root: ET.Element) -> Dict[str, Any]:
        """원천징수영수증 파싱"""
        return {
            "document_type": "원천징수영수증",
            "elements": len(list(root.iter())),
            "parsed": True
        }
    
    def _parse_generic(self, root: ET.Element) -> Dict[str, Any]:
        """일반 XML 파싱"""
        return {
            "document_type": "기타 세무 문서",
            "root_tag": root.tag,
            "element_count": len(list(root.iter())),
            "raw_xml_preview": self.xml_data[:1000] if self.xml_data else None
        }
    
    def close(self):
        """문서 닫기"""
        if self.doc:
            self.doc.close()


# 테스트용 함수
def parse_nts_pdf(pdf_path: str = None, pdf_bytes: bytes = None, password: str = "") -> Dict[str, Any]:
    """
    국세청 전자문서 PDF 파싱 메인 함수
    
    Args:
        pdf_path: PDF 파일 경로
        pdf_bytes: PDF 바이트 데이터
        password: 비밀번호
    
    Returns:
        파싱된 세무 데이터
    """
    parser = NTSDocumentParser(pdf_path=pdf_path, pdf_bytes=pdf_bytes, password=password)
    
    try:
        # 1. 문서 검증
        verification = parser.verify_document()
        
        if not verification.get("is_valid"):
            return {"success": False, "error": verification.get("error")}
        
        # 2. 데이터 추출
        tax_data = parser.parse_tax_data()
        
        return {
            "success": True,
            "verification": verification,
            "data": tax_data
        }
    
    finally:
        parser.close()


if __name__ == "__main__":
    # 테스트
    import sys
    if len(sys.argv) > 1:
        result = parse_nts_pdf(pdf_path=sys.argv[1])
        import json
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print("사용법: python nts_parser.py <pdf_file_path>")
