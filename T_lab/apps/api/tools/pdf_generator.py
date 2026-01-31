from reportlab.lib.pagesizes import letter
from reportlab.platypus import SimpleDocTemplate, Paragraph, Spacer, Image, Table, TableStyle
from reportlab.lib.styles import getSampleStyleSheet, ParagraphStyle
from reportlab.lib import colors
from reportlab.pdfbase import pdfmetrics
from reportlab.pdfbase.ttfonts import TTFont
import re
import os


def create_pdf(markdown_text: str, output_path: str, title: str = "Research Report", image_path: str = None):
    """Convert markdown research report to PDF with proper table and image handling."""
    # Register Korean Font
    font_path = os.path.join(os.path.dirname(__file__), "fonts", "NanumGothic.ttf")
    pdfmetrics.registerFont(TTFont('NanumGothic', font_path))
    
    doc = SimpleDocTemplate(output_path, pagesize=letter)
    styles = getSampleStyleSheet()
    
    # Update default styles to use Korean font
    styles['Normal'].fontName = 'NanumGothic'
    styles['Heading1'].fontName = 'NanumGothic'
    styles['Heading2'].fontName = 'NanumGothic'
    styles['Heading3'].fontName = 'NanumGothic'
    styles['Title'].fontName = 'NanumGothic'
    
    # Custom styles
    title_style = ParagraphStyle(
        'TitleStyle',
        parent=styles['Heading1'],
        fontName='NanumGothic',
        fontSize=18,
        spaceAfter=12
    )
    
    table_header_style = ParagraphStyle(
        'TableHeader',
        parent=styles['Normal'],
        fontName='NanumGothic',
        fontSize=8,
        alignment=1  # Center
    )
    
    table_cell_style = ParagraphStyle(
        'TableCell',
        parent=styles['Normal'],
        fontName='NanumGothic',
        fontSize=8
    )
    
    content_list = []
    
    # Add title
    content_list.append(Paragraph(title, title_style))
    content_list.append(Spacer(1, 12))
    
    # Remove existing image links to avoid duplication
    markdown_text = re.sub(r'!\[.*?\]\(.*?\)', '', markdown_text)
    
    # Parse markdown and convert to PDF elements
    lines = markdown_text.split('\n')
    i = 0
    while i < len(lines):
        line = lines[i]
        
        # Handle Markdown Tables
        if '|' in line and i + 1 < len(lines) and '---' in lines[i + 1]:
            table_data = []
            
            # Parse header
            header_cells = [cell.strip() for cell in line.split('|') if cell.strip()]
            table_data.append([Paragraph(cell, table_header_style) for cell in header_cells])
            
            # Skip separator line
            i += 2
            
            # Parse body rows
            while i < len(lines) and '|' in lines[i]:
                row_cells = [cell.strip() for cell in lines[i].split('|') if cell.strip()]
                table_data.append([Paragraph(cell, table_cell_style) for cell in row_cells])
                i += 1
            
            # Create PDF Table
            if table_data:
                # Calculate column widths (proportional)
                num_cols = len(table_data[0]) if table_data else 1
                col_width = 480 / num_cols
                
                pdf_table = Table(table_data, colWidths=[col_width] * num_cols)
                pdf_table.setStyle(TableStyle([
                    # Header row - light gray background, dark text
                    ('BACKGROUND', (0, 0), (-1, 0), colors.Color(0.85, 0.85, 0.9)),
                    ('TEXTCOLOR', (0, 0), (-1, 0), colors.Color(0.1, 0.1, 0.2)),
                    ('ALIGN', (0, 0), (-1, -1), 'CENTER'),
                    ('FONTNAME', (0, 0), (-1, -1), 'NanumGothic'),
                    ('FONTSIZE', (0, 0), (-1, 0), 9),
                    ('FONTSIZE', (0, 1), (-1, -1), 8),
                    ('BOTTOMPADDING', (0, 0), (-1, 0), 8),
                    ('TOPPADDING', (0, 0), (-1, -1), 6),
                    # Data rows - white background, dark text
                    ('BACKGROUND', (0, 1), (-1, -1), colors.white),
                    ('TEXTCOLOR', (0, 1), (-1, -1), colors.black),
                    ('GRID', (0, 0), (-1, -1), 0.5, colors.Color(0.7, 0.7, 0.75)),
                    ('VALIGN', (0, 0), (-1, -1), 'MIDDLE'),
                ]))
                content_list.append(pdf_table)
                content_list.append(Spacer(1, 12))
            continue
        
        # Handle headings
        if line.startswith('# '):
            content_list.append(Paragraph(line[2:], styles['Heading1']))
        elif line.startswith('## '):
            content_list.append(Paragraph(line[3:], styles['Heading2']))
        elif line.startswith('### '):
            content_list.append(Paragraph(line[4:], styles['Heading3']))
        elif line.startswith('#### '):
            content_list.append(Paragraph(line[5:], styles['Normal']))
        elif line.strip() == '' or line.strip() == '---':
            content_list.append(Spacer(1, 6))
        elif line.startswith('- '):
            # Bullet point
            bullet_text = f"• {line[2:]}"
            content_list.append(Paragraph(bullet_text, styles['Normal']))
        elif line.startswith('**') and line.endswith('**'):
            # Bold line (like keywords)
            content_list.append(Paragraph(f"<b>{line[2:-2]}</b>", styles['Normal']))
        else:
            if line.strip():
                # Clean up any remaining markdown syntax
                clean_line = re.sub(r'\*\*(.*?)\*\*', r'<b>\1</b>', line)
                clean_line = re.sub(r'\*(.*?)\*', r'<i>\1</i>', clean_line)
                clean_line = re.sub(r'\$(.*?)\$', r'\1', clean_line)  # Remove LaTeX delimiters
                content_list.append(Paragraph(clean_line, styles['Normal']))
        
        i += 1
    
    # Add Image naturally (no separate heading)
    if image_path and os.path.exists(image_path):
        try:
            content_list.append(Spacer(1, 16))
            
            # Resize image to fit page width (preserving aspect ratio)
            from reportlab.lib.utils import ImageReader
            img = ImageReader(image_path)
            iw, ih = img.getSize()
            aspect = ih / float(iw)
            
            display_width = 460  # Approx 16cm
            display_height = display_width * aspect
            
            # Limit height if too tall (e.g. > 600)
            if display_height > 600:
                display_height = 600
                display_width = display_height / aspect

            pdf_img = Image(image_path, width=display_width, height=display_height)
            content_list.append(pdf_img)
        except Exception as e:
            print(f"Failed to add image: {e}")
    
    doc.build(content_list)
    return output_path
