import time
import pandas as pd
from win32com.client import Dispatch
from pywinauto import Application

# Load the Excel application
excel = Dispatch('Excel.Application')
excel.Visible = True

# Open the specified workbook
workbook = excel.Workbooks.Open(r'C:\path\to\your\Report.xlsx')

# Wait for the workbook to open
time.sleep(2)

# Connect to the Excel application
app = Application().connect(title='Report.xlsx - Excel')

# Select the 'Grid' pane and input the value '1000'
app.Report_xlsx_Excel.set_focus()
app.Report_xlsx_Excel.child_window(title="Grid", control_type="Pane").click_input()
app.Report_xlsx_Excel.child_window(title="Grid", control_type="Pane").type_keys('1000', with_spaces=True)

# Save and close the workbook
workbook.Save()
workbook.Close()
excel.Quit()