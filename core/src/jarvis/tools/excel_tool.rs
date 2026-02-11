// Excel Tool - Automated Excel file processing

use super::{Tool, ToolParams, ToolResult};
use anyhow::Result;
use calamine::{open_workbook, Reader, Xlsx};
use serde_json::json;
use std::path::Path;

pub struct ExcelTool {}

impl ExcelTool {
    pub fn new() -> Self {
        Self {}
    }

    fn read_excel(&self, path: &str) -> Result<ToolResult> {
        if !Path::new(path).exists() {
            return Ok(ToolResult {
                success: false,
                message: format!("File not found: {}", path),
                data: None,
            });
        }

        let mut workbook: Xlsx<_> = open_workbook(path)?;
        let sheet_names = workbook.sheet_names().to_vec();

        if sheet_names.is_empty() {
            return Ok(ToolResult {
                success: false,
                message: "No sheets found in workbook".to_string(),
                data: None,
            });
        }

        // Read first sheet
        let first_sheet = &sheet_names[0];
        let range = workbook
            .worksheet_range(first_sheet)?;

        let mut rows = Vec::new();
        let mut row_count = 0;

        for row in range.rows().take(100) {
            // Limit to 100 rows
            let cells: Vec<String> = row
                .iter()
                .map(|cell| cell.to_string())
                .collect();
            rows.push(cells);
            row_count += 1;
        }

        log::info!("?諭?Read {} rows from {}", row_count, path);

        Ok(ToolResult {
            success: true,
            message: format!("Successfully read {} from sheet '{}'", path, first_sheet),
            data: Some(json!({
                "path": path,
                "sheet": first_sheet,
                "row_count": row_count,
                "preview": rows.iter().take(5).collect::<Vec<_>>(),
            })),
        })
    }

    fn analyze_excel(&self, path: &str) -> Result<ToolResult> {
        // Simplified analysis - count rows, columns, detect data types
        let mut workbook: Xlsx<_> = open_workbook(path)?;
        let sheet_names = workbook.sheet_names().to_vec();

        if sheet_names.is_empty() {
            return Ok(ToolResult {
                success: false,
                message: "No sheets found".to_string(),
                data: None,
            });
        }

        let first_sheet = &sheet_names[0];
        let range = workbook.worksheet_range(first_sheet)?;

        let (rows, cols) = range.get_size();

        log::info!("?諭?Analyzed {}: {}x{} cells", path, rows, cols);

        Ok(ToolResult {
            success: true,
            message: format!("Analyzed {} - {}x{} cells", path, rows, cols),
            data: Some(json!({
                "path": path,
                "rows": rows,
                "columns": cols,
                "sheets": sheet_names,
            })),
        })
    }
}

#[async_trait::async_trait]
impl Tool for ExcelTool {
    fn name(&self) -> &str {
        "excel"
    }

    fn description(&self) -> &str {
        "Excel file processing - read, analyze, and manipulate Excel files"
    }

    async fn execute(&self, params: ToolParams) -> Result<ToolResult> {
        let action = params.action.as_str();

        match action {
            "read" => {
                let path = params.get_string("path")?;
                self.read_excel(&path)
            }
            "analyze" => {
                let path = params.get_string("path")?;
                self.analyze_excel(&path)
            }
            _ => Ok(ToolResult {
                success: false,
                message: format!("Unknown action: {}", action),
                data: None,
            }),
        }
    }
}
