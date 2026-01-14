"""
병원(의료기관) 전자문서 테스트용 샘플 PDF 생성기 - 회사B
- 요양급여비용 심사결과 통보서
- 진료비 청구 내역서
- 비급여 매출 현황표
- 의료기관 월별 실적 보고서
"""

from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas
from reportlab.lib.units import mm
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
import os
from pathlib import Path

# 한글 폰트 등록 (Mac 기본 폰트)
try:
    pdfmetrics.registerFont(TTFont('AppleGothic', '/System/Library/Fonts/Supplemental/AppleGothic.ttf'))
    FONT_NAME = 'AppleGothic'
except:
    FONT_NAME = 'Helvetica'

OUTPUT_DIR = Path(__file__).parent / "sample_pdfs" / "hospital_companyB"
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

COMPANY_NAME = "(의)회사B의원"
BIZ_NUM = "456-78-90123"


def create_insurance_claim_result():
    """요양급여비용 심사결과 통보서"""
    filename = OUTPUT_DIR / "요양급여_심사결과_2025_12.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "요양급여비용 심사결과 통보서")
    
    c.setFont(FONT_NAME, 10)
    c.drawString(400, height - 50, "건강보험심사평가원")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"요양기관명: {COMPANY_NAME}")
    c.drawString(50, height - 110, f"사업자번호: {BIZ_NUM}")
    c.drawString(50, height - 130, "심사년월: 2025년 12월분")
    c.drawString(50, height - 150, "통보일자: 2026년 01월 10일")
    
    c.line(50, height - 170, width - 50, height - 170)
    
    y = height - 200
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 심사결과 요약 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 30
    
    items = [
        ("청구건수", "1,245건"),
        ("청구금액", "156,780,000원"),
        ("심사조정금액", "-4,230,000원"),
        ("결정금액", "152,550,000원"),
        ("본인부담금", "22,882,500원"),
        ("공단부담금", "129,667,500원"),
    ]
    
    for label, value in items:
        c.drawString(60, y, f"• {label}:")
        c.drawString(200, y, value)
        y -= 22
    
    y -= 20
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 조정내역 상세 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 25
    
    adjustments = [
        ("의료장비 사용료 과다청구", "-1,500,000원", "12건"),
        ("약제비 산정기준 초과", "-980,000원", "45건"),
        ("검사료 중복청구", "-750,000원", "8건"),
        ("진찰료 산정오류", "-500,000원", "23건"),
        ("주사료 기준초과", "-500,000원", "15건"),
    ]
    
    c.drawString(60, y, "사유")
    c.drawString(280, y, "조정금액")
    c.drawString(400, y, "건수")
    y -= 5
    c.line(50, y, width - 50, y)
    y -= 18
    
    for reason, amount, count in adjustments:
        c.drawString(60, y, reason)
        c.drawString(280, y, amount)
        c.drawString(400, y, count)
        y -= 20
    
    # XML
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<요양급여심사결과>
  <요양기관>{}</요양기관>
  <심사년월>2025-12</심사년월>
  <청구금액>156780000</청구금액>
  <조정금액>4230000</조정금액>
  <결정금액>152550000</결정금액>
  <본인부담금>22882500</본인부담금>
  <공단부담금>129667500</공단부담금>
</요양급여심사결과>""".format(COMPANY_NAME)
    
    c.setFont(FONT_NAME, 8)
    y -= 40
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 12
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_medical_billing():
    """진료비 청구 내역서"""
    filename = OUTPUT_DIR / "진료비청구내역_2025_12.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "진료비 청구 내역서")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"의료기관: {COMPANY_NAME}")
    c.drawString(50, height - 110, f"사업자번호: {BIZ_NUM}")
    c.drawString(50, height - 130, "청구기간: 2025년 12월")
    
    c.line(50, height - 150, width - 50, height - 150)
    
    y = height - 180
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 진료과별 청구 현황 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 30
    
    # 테이블 헤더
    c.drawString(60, y, "진료과")
    c.drawString(150, y, "내원환자수")
    c.drawString(250, y, "청구건수")
    c.drawString(350, y, "청구금액")
    y -= 5
    c.line(50, y, width - 50, y)
    y -= 18
    
    departments = [
        ("내과", "412명", "523건", "45,230,000원"),
        ("외과", "189명", "245건", "38,450,000원"),
        ("정형외과", "234명", "312건", "42,100,000원"),
        ("피부과", "156명", "178건", "15,800,000원"),
        ("이비인후과", "98명", "112건", "8,200,000원"),
        ("소아청소년과", "187명", "225건", "12,000,000원"),
    ]
    
    total_patients = 0
    total_claims = 0
    total_amount = 0
    
    for dept, patients, claims, amount in departments:
        c.drawString(60, y, dept)
        c.drawString(150, y, patients)
        c.drawString(250, y, claims)
        c.drawString(350, y, amount)
        y -= 20
        
        total_patients += int(patients.replace('명', ''))
        total_claims += int(claims.replace('건', ''))
        total_amount += int(amount.replace(',', '').replace('원', ''))
    
    c.line(50, y + 5, width - 50, y + 5)
    y -= 5
    c.setFont(FONT_NAME, 11)
    c.drawString(60, y, "합계")
    c.drawString(150, y, f"{total_patients:,}명")
    c.drawString(250, y, f"{total_claims:,}건")
    c.drawString(350, y, f"{total_amount:,}원")
    
    # XML
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<진료비청구내역>
  <의료기관>{}</의료기관>
  <청구기간>2025-12</청구기간>
  <총환자수>{}</총환자수>
  <총청구건수>{}</총청구건수>
  <총청구금액>{}</총청구금액>
</진료비청구내역>""".format(COMPANY_NAME, total_patients, total_claims, total_amount)
    
    c.setFont(FONT_NAME, 8)
    y -= 60
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 12
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_non_covered_revenue():
    """비급여 매출 현황표"""
    filename = OUTPUT_DIR / "비급여매출현황_2025_12.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "비급여 매출 현황표")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"의료기관: {COMPANY_NAME}")
    c.drawString(50, height - 110, "기준월: 2025년 12월")
    
    c.line(50, height - 130, width - 50, height - 130)
    
    y = height - 160
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 비급여 항목별 매출 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 30
    
    c.drawString(60, y, "항목")
    c.drawString(220, y, "건수")
    c.drawString(320, y, "매출액")
    c.drawString(430, y, "비율")
    y -= 5
    c.line(50, y, width - 50, y)
    y -= 18
    
    items = [
        ("도수치료", "245건", "24,500,000원", "32.1%"),
        ("MRI 검사", "89건", "17,800,000원", "23.3%"),
        ("CT 검사", "123건", "12,300,000원", "16.1%"),
        ("초음파 검사", "312건", "9,360,000원", "12.3%"),
        ("주사료 (영양주사 등)", "456건", "6,840,000원", "9.0%"),
        ("제증명료", "234건", "2,340,000원", "3.1%"),
        ("기타", "187건", "3,140,000원", "4.1%"),
    ]
    
    for item, count, amount, ratio in items:
        c.drawString(60, y, item)
        c.drawString(220, y, count)
        c.drawString(320, y, amount)
        c.drawString(430, y, ratio)
        y -= 20
    
    c.line(50, y + 5, width - 50, y + 5)
    y -= 5
    c.setFont(FONT_NAME, 11)
    c.drawString(60, y, "합계")
    c.drawString(220, y, "1,646건")
    c.drawString(320, y, "76,280,000원")
    c.drawString(430, y, "100%")
    
    y -= 40
    c.setFont(FONT_NAME, 10)
    c.drawString(50, y, f"※ 급여 대비 비급여 비율: 32.8% (급여: 156,780,000원)")
    
    # XML
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<비급여매출>
  <의료기관>{}</의료기관>
  <기준월>2025-12</기준월>
  <총비급여매출>76280000</총비급여매출>
  <급여대비비율>32.8</급여대비비율>
  <도수치료>24500000</도수치료>
  <MRI>17800000</MRI>
  <CT>12300000</CT>
</비급여매출>""".format(COMPANY_NAME)
    
    c.setFont(FONT_NAME, 8)
    y -= 50
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 12
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_monthly_report():
    """의료기관 월별 실적 보고서"""
    filename = OUTPUT_DIR / "월별실적보고서_2025_12.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "의료기관 월별 실적 보고서")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"의료기관: {COMPANY_NAME}")
    c.drawString(50, height - 110, f"사업자번호: {BIZ_NUM}")
    c.drawString(50, height - 130, "보고기간: 2025년 12월")
    
    c.line(50, height - 150, width - 50, height - 150)
    
    y = height - 180
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 월간 실적 요약 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 30
    
    metrics = [
        ("총 매출", "233,060,000원", "전월대비 +5.2%"),
        ("  - 급여 매출", "156,780,000원", "(67.3%)"),
        ("  - 비급여 매출", "76,280,000원", "(32.7%)"),
        ("총 내원 환자수", "1,276명", "전월대비 +3.1%"),
        ("  - 초진 환자", "234명", "(18.3%)"),
        ("  - 재진 환자", "1,042명", "(81.7%)"),
        ("평균 진료비", "182,600원", "전월대비 +2.1%"),
        ("재료비", "28,450,000원", "(매출대비 12.2%)"),
        ("인건비", "85,000,000원", "(매출대비 36.5%)"),
        ("영업이익", "45,230,000원", "(영업이익률 19.4%)"),
    ]
    
    for metric, value, note in metrics:
        c.drawString(60, y, metric)
        c.drawString(220, y, value)
        c.drawString(380, y, note)
        y -= 20
    
    y -= 20
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 주요 지표 분석 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 25
    c.drawString(60, y, "• 환자 1인당 평균 진료횟수: 1.23회")
    y -= 20
    c.drawString(60, y, "• 의료장비 가동률: 78.5%")
    y -= 20
    c.drawString(60, y, "• 심사삭감률: 2.7% (업계평균 3.2%)")
    
    # XML
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<월별실적보고>
  <의료기관>{}</의료기관>
  <보고기간>2025-12</보고기간>
  <총매출>233060000</총매출>
  <급여매출>156780000</급여매출>
  <비급여매출>76280000</비급여매출>
  <총환자수>1276</총환자수>
  <영업이익>45230000</영업이익>
  <영업이익률>19.4</영업이익률>
</월별실적보고>""".format(COMPANY_NAME)
    
    c.setFont(FONT_NAME, 8)
    y -= 50
    c.drawString(50, y, "[ 전자문서 데이터 ]")
    y -= 12
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def generate_hospital_samples():
    """병원(회사B) 샘플 PDF 생성"""
    print(f"🏥 {COMPANY_NAME} 전자문서 샘플 PDF 생성 시작...\n")
    
    files = [
        create_insurance_claim_result(),
        create_medical_billing(),
        create_non_covered_revenue(),
        create_monthly_report(),
    ]
    
    print(f"\n✅ 총 {len(files)}개 병원 샘플 PDF 생성 완료!")
    print(f"📁 저장 위치: {OUTPUT_DIR}")
    
    return files


if __name__ == "__main__":
    generate_hospital_samples()
