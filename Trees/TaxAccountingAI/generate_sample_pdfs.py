"""
국세청 전자문서 테스트용 샘플 PDF 생성기
- 연말정산간소화 자료
- 부가가치세 신고서
- 원천징수영수증
- 소득금액증명
"""

from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas
from reportlab.lib.units import mm
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
import os
from pathlib import Path
import io

# 한글 폰트 등록 (Mac 기본 폰트)
try:
    pdfmetrics.registerFont(TTFont('AppleGothic', '/System/Library/Fonts/Supplemental/AppleGothic.ttf'))
    FONT_NAME = 'AppleGothic'
except:
    FONT_NAME = 'Helvetica'

OUTPUT_DIR = Path(__file__).parent / "sample_pdfs"
OUTPUT_DIR.mkdir(exist_ok=True)


def create_year_end_settlement():
    """연말정산간소화 자료 샘플 PDF"""
    filename = OUTPUT_DIR / "연말정산간소화_2025.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    # 헤더
    c.setFont(FONT_NAME, 20)
    c.drawString(50, height - 50, "연말정산간소화 자료")
    
    c.setFont(FONT_NAME, 12)
    c.drawString(50, height - 80, "발급기관: 국세청 홈택스")
    c.drawString(50, height - 100, "귀속년도: 2025년")
    c.drawString(50, height - 120, "발급일자: 2026-01-13")
    
    # 구분선
    c.line(50, height - 140, width - 50, height - 140)
    
    # 소득공제 항목
    y = height - 170
    c.setFont(FONT_NAME, 14)
    c.drawString(50, y, "[ 소득공제 내역 ]")
    
    items = [
        ("보험료", "2,400,000원", "공제액: 240,000원"),
        ("의료비", "3,500,000원", "공제액: 350,000원"),
        ("교육비", "4,800,000원", "공제액: 720,000원"),
        ("신용카드", "15,000,000원", "공제액: 2,250,000원"),
        ("체크카드/현금영수증", "8,000,000원", "공제액: 1,200,000원"),
        ("기부금", "1,000,000원", "공제액: 150,000원"),
    ]
    
    c.setFont(FONT_NAME, 11)
    y -= 30
    for item, amount, deduction in items:
        c.drawString(60, y, f"• {item}")
        c.drawString(200, y, amount)
        c.drawString(350, y, deduction)
        y -= 25
    
    # 합계
    c.line(50, y + 10, width - 50, y + 10)
    c.setFont(FONT_NAME, 12)
    y -= 20
    c.drawString(60, y, "총 공제 가능액: 4,910,000원")
    c.drawString(60, y - 25, "예상 환급액: 736,500원")
    
    # XML 데이터 (텍스트로 삽입)
    c.setFont(FONT_NAME, 8)
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<연말정산간소화>
  <귀속년도>2025</귀속년도>
  <보험료금액>2400000</보험료금액>
  <의료비금액>3500000</의료비금액>
  <교육비금액>4800000</교육비금액>
  <신용카드금액>15000000</신용카드금액>
  <기부금금액>1000000</기부금금액>
  <총공제액>4910000</총공제액>
</연말정산간소화>"""
    
    y -= 80
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 15
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_vat_return():
    """부가가치세 신고서 샘플 PDF"""
    filename = OUTPUT_DIR / "부가가치세신고서_2025_2기.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 20)
    c.drawString(50, height - 50, "부가가치세 확정신고서")
    
    c.setFont(FONT_NAME, 12)
    c.drawString(50, height - 80, "신고구분: 일반과세자")
    c.drawString(50, height - 100, "과세기간: 2025년 2기 (7월~12월)")
    c.drawString(50, height - 120, "사업자번호: 123-45-67890")
    
    c.line(50, height - 140, width - 50, height - 140)
    
    y = height - 170
    c.setFont(FONT_NAME, 14)
    c.drawString(50, y, "[ 매출세액 ]")
    
    c.setFont(FONT_NAME, 11)
    y -= 30
    c.drawString(60, y, "과세 매출액: 250,000,000원")
    y -= 25
    c.drawString(60, y, "면세 매출액: 25,000,000원")
    y -= 25
    c.drawString(60, y, "매출세액 (10%): 25,000,000원")
    
    y -= 40
    c.setFont(FONT_NAME, 14)
    c.drawString(50, y, "[ 매입세액 ]")
    
    c.setFont(FONT_NAME, 11)
    y -= 30
    c.drawString(60, y, "과세 매입액: 180,000,000원")
    y -= 25
    c.drawString(60, y, "매입세액: 18,000,000원")
    
    y -= 40
    c.line(50, y + 10, width - 50, y + 10)
    c.setFont(FONT_NAME, 12)
    c.drawString(60, y, "납부세액: 7,000,000원")
    
    # XML
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<부가가치세신고>
  <과세기간>2025년2기</과세기간>
  <과세매출>250000000</과세매출>
  <면세매출>25000000</면세매출>
  <매출세액>25000000</매출세액>
  <과세매입>180000000</과세매입>
  <매입세액>18000000</매입세액>
  <납부세액>7000000</납부세액>
</부가가치세신고>"""
    
    c.setFont(FONT_NAME, 8)
    y -= 60
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 15
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_withholding_tax():
    """원천징수영수증 샘플 PDF"""
    filename = OUTPUT_DIR / "원천징수영수증_2025.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 20)
    c.drawString(50, height - 50, "근로소득 원천징수영수증")
    
    c.setFont(FONT_NAME, 12)
    c.drawString(50, height - 80, "귀속연도: 2025년")
    c.drawString(50, height - 100, "징수의무자: (주)테스트기업")
    c.drawString(50, height - 120, "사업자번호: 987-65-43210")
    
    c.line(50, height - 140, width - 50, height - 140)
    
    y = height - 170
    c.setFont(FONT_NAME, 14)
    c.drawString(50, y, "[ 근로소득 내역 ]")
    
    c.setFont(FONT_NAME, 11)
    y -= 30
    items = [
        ("급여", "48,000,000원"),
        ("상여", "6,000,000원"),
        ("인정상여", "0원"),
        ("주식매수선택권 행사이익", "0원"),
        ("우리사주조합 인출금", "0원"),
        ("임원퇴직소득금액 한도초과액", "0원"),
        ("직무발명보상금", "1,000,000원"),
        ("기타근로소득", "0원"),
    ]
    
    for item, amount in items:
        c.drawString(60, y, f"• {item}: {amount}")
        y -= 22
    
    y -= 20
    c.line(50, y + 10, width - 50, y + 10)
    c.drawString(60, y, "총 급여: 55,000,000원")
    y -= 25
    c.drawString(60, y, "결정세액: 2,750,000원")
    y -= 25
    c.drawString(60, y, "기납부세액: 2,900,000원")
    y -= 25
    c.setFont(FONT_NAME, 12)
    c.drawString(60, y, "차감징수세액 (환급): -150,000원")
    
    # XML
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<원천징수영수증>
  <귀속연도>2025</귀속연도>
  <총급여>55000000</총급여>
  <결정세액>2750000</결정세액>
  <기납부세액>2900000</기납부세액>
  <차감징수세액>-150000</차감징수세액>
</원천징수영수증>"""
    
    c.setFont(FONT_NAME, 8)
    y -= 50
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 15
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_income_statement():
    """소득금액증명 샘플 PDF"""
    filename = OUTPUT_DIR / "소득금액증명_2025.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 20)
    c.drawString(50, height - 50, "소득금액증명원")
    
    c.setFont(FONT_NAME, 12)
    c.drawString(50, height - 80, "발급번호: NTS-2026-01-12345678")
    c.drawString(50, height - 100, "발급일자: 2026년 01월 13일")
    c.drawString(50, height - 120, "발급기관: 국세청")
    
    c.line(50, height - 140, width - 50, height - 140)
    
    y = height - 170
    c.setFont(FONT_NAME, 14)
    c.drawString(50, y, "[ 신청인 인적사항 ]")
    
    c.setFont(FONT_NAME, 11)
    y -= 30
    c.drawString(60, y, "성명: 홍길동")
    y -= 25
    c.drawString(60, y, "주민등록번호: 850101-1******")
    y -= 25
    c.drawString(60, y, "주소: 서울특별시 강남구 테헤란로 123")
    
    y -= 40
    c.setFont(FONT_NAME, 14)
    c.drawString(50, y, "[ 소득금액 증명 내용 ]")
    
    c.setFont(FONT_NAME, 11)
    y -= 30
    years = [
        ("2025년", "근로소득", "55,000,000원"),
        ("2024년", "근로소득", "52,000,000원"),
        ("2023년", "근로소득", "48,000,000원"),
    ]
    
    for year, income_type, amount in years:
        c.drawString(60, y, f"• {year}: {income_type} {amount}")
        y -= 25
    
    y -= 20
    c.line(50, y + 10, width - 50, y + 10)
    c.drawString(60, y, "상기 내용이 사실임을 증명합니다.")
    y -= 40
    c.drawString(300, y, "국  세  청  장")
    
    # XML
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<소득금액증명>
  <발급번호>NTS-2026-01-12345678</발급번호>
  <성명>홍길동</성명>
  <소득내역>
    <연도2025>55000000</연도2025>
    <연도2024>52000000</연도2024>
    <연도2023>48000000</연도2023>
  </소득내역>
</소득금액증명>"""
    
    c.setFont(FONT_NAME, 8)
    y -= 80
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 15
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def generate_all_samples():
    """모든 샘플 PDF 생성"""
    print("국세청 전자문서 샘플 PDF 생성 시작...\n")
    
    files = [
        create_year_end_settlement(),
        create_vat_return(),
        create_withholding_tax(),
        create_income_statement(),
    ]
    
    print(f"\n✅ 총 {len(files)}개 샘플 PDF 생성 완료!")
    print(f"📁 저장 위치: {OUTPUT_DIR}")
    
    return files


if __name__ == "__main__":
    generate_all_samples()
