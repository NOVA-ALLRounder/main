"""
커머스/쇼핑몰 전자문서 테스트용 샘플 PDF 생성기 - 회사C
- 오픈마켓 정산 내역서
- 광고비 지출 내역서
- 매출/재고 현황 보고서
- 부가세 매입/매출 집계표
"""

from reportlab.lib.pagesizes import A4
from reportlab.pdfgen import canvas
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
from pathlib import Path

try:
    pdfmetrics.registerFont(TTFont('AppleGothic', '/System/Library/Fonts/Supplemental/AppleGothic.ttf'))
    FONT_NAME = 'AppleGothic'
except:
    FONT_NAME = 'Helvetica'

OUTPUT_DIR = Path(__file__).parent / "sample_pdfs" / "commerce_companyC"
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

COMPANY_NAME = "(주)회사C커머스"
BIZ_NUM = "789-01-23456"


def create_marketplace_settlement():
    """오픈마켓 정산 내역서"""
    filename = OUTPUT_DIR / "오픈마켓정산_2025_12.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "오픈마켓 정산 내역서")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"판매자: {COMPANY_NAME}")
    c.drawString(50, height - 110, f"사업자번호: {BIZ_NUM}")
    c.drawString(50, height - 130, "정산기간: 2025년 12월")
    
    c.line(50, height - 150, width - 50, height - 150)
    
    y = height - 180
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 마켓별 정산 현황 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 30
    c.drawString(60, y, "마켓")
    c.drawString(150, y, "판매건수")
    c.drawString(240, y, "매출액")
    c.drawString(350, y, "수수료")
    c.drawString(450, y, "정산액")
    y -= 5
    c.line(50, y, width - 50, y)
    y -= 18
    
    markets = [
        ("네이버스마트스토어", "1,245건", "89,500,000원", "4,475,000원", "85,025,000원"),
        ("쿠팡", "2,456건", "156,000,000원", "17,160,000원", "138,840,000원"),
        ("11번가", "567건", "34,200,000원", "4,446,000원", "29,754,000원"),
        ("G마켓/옥션", "234건", "18,700,000원", "2,244,000원", "16,456,000원"),
        ("자사몰", "456건", "45,600,000원", "0원", "45,600,000원"),
    ]
    
    for market, count, sales, fee, settle in markets:
        c.drawString(60, y, market)
        c.drawString(150, y, count)
        c.drawString(240, y, sales)
        c.drawString(350, y, fee)
        c.drawString(450, y, settle)
        y -= 20
    
    c.line(50, y + 5, width - 50, y + 5)
    c.setFont(FONT_NAME, 11)
    y -= 5
    c.drawString(60, y, "합계")
    c.drawString(150, y, "4,958건")
    c.drawString(240, y, "344,000,000원")
    c.drawString(350, y, "28,325,000원")
    c.drawString(450, y, "315,675,000원")
    
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<오픈마켓정산>
  <판매자>{}</판매자>
  <정산기간>2025-12</정산기간>
  <총매출>344000000</총매출>
  <총수수료>28325000</총수수료>
  <정산액>315675000</정산액>
</오픈마켓정산>""".format(COMPANY_NAME)
    
    c.setFont(FONT_NAME, 8)
    y -= 50
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_ad_expense():
    """광고비 지출 내역서"""
    filename = OUTPUT_DIR / "광고비내역_2025_12.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "온라인 광고비 지출 내역서")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"광고주: {COMPANY_NAME}")
    c.drawString(50, height - 110, "광고기간: 2025년 12월")
    
    c.line(50, height - 130, width - 50, height - 130)
    
    y = height - 160
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 광고 채널별 지출 현황 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 30
    c.drawString(60, y, "채널")
    c.drawString(180, y, "지출액")
    c.drawString(290, y, "클릭수")
    c.drawString(380, y, "전환수")
    c.drawString(460, y, "ROAS")
    y -= 5
    c.line(50, y, width - 50, y)
    y -= 18
    
    channels = [
        ("네이버 검색광고", "12,500,000원", "45,000", "1,350", "320%"),
        ("카카오 모먼트", "8,200,000원", "28,000", "840", "285%"),
        ("구글 Ads", "6,800,000원", "22,000", "660", "310%"),
        ("메타(페이스북/인스타)", "5,500,000원", "18,000", "540", "275%"),
        ("쿠팡 광고", "4,000,000원", "15,000", "600", "450%"),
    ]
    
    for channel, cost, clicks, conv, roas in channels:
        c.drawString(60, y, channel)
        c.drawString(180, y, cost)
        c.drawString(290, y, clicks)
        c.drawString(380, y, conv)
        c.drawString(460, y, roas)
        y -= 20
    
    c.line(50, y + 5, width - 50, y + 5)
    c.setFont(FONT_NAME, 11)
    y -= 5
    c.drawString(60, y, "합계")
    c.drawString(180, y, "37,000,000원")
    c.drawString(290, y, "128,000")
    c.drawString(380, y, "3,990")
    c.drawString(460, y, "평균 320%")
    
    y -= 30
    c.drawString(50, y, f"※ 광고비 대비 매출: {344000000:,}원 (ROAS 930%)")
    
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<광고비내역>
  <광고주>{}</광고주>
  <기간>2025-12</기간>
  <총광고비>37000000</총광고비>
  <총클릭>128000</총클릭>
  <총전환>3990</총전환>
  <평균ROAS>320</평균ROAS>
</광고비내역>""".format(COMPANY_NAME)
    
    c.setFont(FONT_NAME, 8)
    y -= 50
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_sales_inventory():
    """매출/재고 현황 보고서"""
    filename = OUTPUT_DIR / "매출재고현황_2025_12.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "매출 및 재고 현황 보고서")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"사업자: {COMPANY_NAME}")
    c.drawString(50, height - 110, "기준일: 2025년 12월 31일")
    
    c.line(50, height - 130, width - 50, height - 130)
    
    y = height - 160
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 카테고리별 매출/재고 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 30
    c.drawString(60, y, "카테고리")
    c.drawString(160, y, "매출액")
    c.drawString(270, y, "원가")
    c.drawString(360, y, "마진율")
    c.drawString(430, y, "재고금액")
    y -= 5
    c.line(50, y, width - 50, y)
    y -= 18
    
    categories = [
        ("의류/패션", "145,000,000원", "72,500,000원", "50%", "38,000,000원"),
        ("화장품/뷰티", "89,000,000원", "35,600,000원", "60%", "22,000,000원"),
        ("생활/주방", "56,000,000원", "33,600,000원", "40%", "15,000,000원"),
        ("가전/디지털", "34,000,000원", "27,200,000원", "20%", "28,000,000원"),
        ("식품", "20,000,000원", "14,000,000원", "30%", "8,000,000원"),
    ]
    
    for cat, sales, cost, margin, inventory in categories:
        c.drawString(60, y, cat)
        c.drawString(160, y, sales)
        c.drawString(270, y, cost)
        c.drawString(360, y, margin)
        c.drawString(430, y, inventory)
        y -= 20
    
    c.line(50, y + 5, width - 50, y + 5)
    c.setFont(FONT_NAME, 11)
    y -= 5
    c.drawString(60, y, "합계")
    c.drawString(160, y, "344,000,000원")
    c.drawString(270, y, "182,900,000원")
    c.drawString(360, y, "평균 47%")
    c.drawString(430, y, "111,000,000원")
    
    y -= 30
    c.drawString(50, y, "※ 재고회전일: 14일 | 평균 객단가: 69,400원")
    
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<매출재고현황>
  <사업자>{}</사업자>
  <총매출>344000000</총매출>
  <총원가>182900000</총원가>
  <매출총이익>161100000</매출총이익>
  <재고금액>111000000</재고금액>
  <재고회전일>14</재고회전일>
</매출재고현황>""".format(COMPANY_NAME)
    
    c.setFont(FONT_NAME, 8)
    y -= 50
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def create_vat_summary():
    """부가세 매입/매출 집계표"""
    filename = OUTPUT_DIR / "부가세집계_2025_2기.pdf"
    c = canvas.Canvas(str(filename), pagesize=A4)
    width, height = A4
    
    c.setFont(FONT_NAME, 18)
    c.drawString(50, height - 50, "부가가치세 매입/매출 집계표")
    
    c.setFont(FONT_NAME, 11)
    c.drawString(50, height - 90, f"사업자: {COMPANY_NAME}")
    c.drawString(50, height - 110, f"사업자번호: {BIZ_NUM}")
    c.drawString(50, height - 130, "과세기간: 2025년 2기 (7월~12월)")
    
    c.line(50, height - 150, width - 50, height - 150)
    
    y = height - 180
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 매출세액 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 25
    c.drawString(60, y, "• 과세 매출 (카드/현금영수증): 312,800,000원")
    y -= 20
    c.drawString(60, y, "• 세금계산서 발행 매출: 31,200,000원")
    y -= 20
    c.drawString(60, y, "• 매출세액 (10%): 34,400,000원")
    
    y -= 35
    c.setFont(FONT_NAME, 13)
    c.drawString(50, y, "[ 매입세액 ]")
    
    c.setFont(FONT_NAME, 10)
    y -= 25
    c.drawString(60, y, "• 상품 매입: 182,900,000원 (세금계산서)")
    y -= 20
    c.drawString(60, y, "• 광고비: 37,000,000원 (세금계산서)")
    y -= 20
    c.drawString(60, y, "• 물류비: 15,000,000원 (세금계산서)")
    y -= 20
    c.drawString(60, y, "• 기타 운영비: 8,000,000원")
    y -= 20
    c.drawString(60, y, "• 매입세액: 24,290,000원")
    
    y -= 35
    c.line(50, y + 5, width - 50, y + 5)
    c.setFont(FONT_NAME, 12)
    c.drawString(50, y, "납부세액: 10,110,000원")
    
    xml_data = """<?xml version="1.0" encoding="UTF-8"?>
<부가세집계>
  <사업자>{}</사업자>
  <과세기간>2025-2기</과세기간>
  <과세매출>344000000</과세매출>
  <매출세액>34400000</매출세액>
  <과세매입>242900000</과세매입>
  <매입세액>24290000</매입세액>
  <납부세액>10110000</납부세액>
</부가세집계>""".format(COMPANY_NAME)
    
    c.setFont(FONT_NAME, 8)
    y -= 50
    for line in xml_data.split('\n'):
        c.drawString(50, y, line)
        y -= 10
    
    c.save()
    print(f"생성 완료: {filename}")
    return filename


def generate_commerce_samples():
    """커머스(회사C) 샘플 PDF 생성"""
    print(f"🛒 {COMPANY_NAME} 전자문서 샘플 PDF 생성 시작...\n")
    
    files = [
        create_marketplace_settlement(),
        create_ad_expense(),
        create_sales_inventory(),
        create_vat_summary(),
    ]
    
    print(f"\n✅ 총 {len(files)}개 커머스 샘플 PDF 생성 완료!")
    print(f"📁 저장 위치: {OUTPUT_DIR}")
    
    return files


if __name__ == "__main__":
    generate_commerce_samples()
