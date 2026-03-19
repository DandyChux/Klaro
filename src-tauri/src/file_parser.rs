use calamine::{Reader, Xls, Xlsx, open_workbook};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Cursor;
use std::path::Path;

// Simple error type
#[derive(Debug)]
pub struct ParseError(pub String);

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError(e.to_string())
    }
}

impl From<csv::Error> for ParseError {
    fn from(e: csv::Error) -> Self {
        ParseError(e.to_string())
    }
}

impl From<calamine::Error> for ParseError {
    fn from(e: calamine::Error) -> Self {
        ParseError(e.to_string())
    }
}

impl From<calamine::XlsxError> for ParseError {
    fn from(e: calamine::XlsxError) -> Self {
        ParseError(e.to_string())
    }
}

impl From<std::string::FromUtf8Error> for ParseError {
    fn from(e: std::string::FromUtf8Error) -> Self {
        ParseError(e.to_string())
    }
}

impl<W: std::io::Write> From<csv::IntoInnerError<csv::Writer<W>>> for ParseError {
    fn from(e: csv::IntoInnerError<csv::Writer<W>>) -> Self {
        ParseError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ParseError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFile {
    pub file_type: FileType,
    pub content: FileContent,
    pub original_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FileType {
    Csv,
    Xlsx,
    Xls,
    Txt,
    Json,
    Unknown,
}

impl FileType {
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "csv" => FileType::Csv,
            "xlsx" => FileType::Xlsx,
            "xls" => FileType::Xls,
            "txt" => FileType::Txt,
            "json" => FileType::Json,
            _ => FileType::Unknown,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            FileType::Csv => "csv",
            FileType::Xlsx => "xlsx",
            FileType::Xls => "xls",
            FileType::Txt => "txt",
            FileType::Json => "json",
            FileType::Unknown => "txt",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileContent {
    Tabular(TabularData),
    Text(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabularData {
    pub headers: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
    pub sheet_name: Option<String>,
}

pub struct FileParser;

impl FileParser {
    pub fn parse_file(path: &str) -> Result<ParsedFile> {
        let path_obj = Path::new(path);
        let extension = path_obj.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file_type = FileType::from_extension(extension);
        let original_name = path_obj
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let content = match file_type {
            FileType::Csv => Self::parse_csv(path)?,
            FileType::Xlsx => Self::parse_xlsx(path)?,
            FileType::Xls => Self::parse_xls(path)?,
            FileType::Txt | FileType::Json => Self::parse_text(path)?,
            FileType::Unknown => Self::parse_text(path)?,
        };

        Ok(ParsedFile {
            file_type,
            content,
            original_name,
        })
    }

    pub fn parse_from_bytes(data: &[u8], filename: &str) -> Result<ParsedFile> {
        let path_obj = Path::new(filename);
        let extension = path_obj.extension().and_then(|e| e.to_str()).unwrap_or("");
        let file_type = FileType::from_extension(extension);

        let content = match file_type {
            FileType::Csv => Self::parse_csv_bytes(data)?,
            FileType::Xlsx => Self::parse_xlsx_bytes(data)?,
            FileType::Xls => Self::parse_xls_bytes(data)?,
            FileType::Txt | FileType::Json => Self::parse_text_bytes(data)?,
            FileType::Unknown => Self::parse_text_bytes(data)?,
        };

        Ok(ParsedFile {
            file_type,
            content,
            original_name: filename.to_string(),
        })
    }

    fn parse_csv(path: &str) -> Result<FileContent> {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .has_headers(true)
            .from_path(path)?;

        let headers: Option<Vec<String>> = reader
            .headers()
            .ok()
            .map(|h| h.iter().map(|s| s.to_string()).collect());

        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result?;
            rows.push(record.iter().map(|s| s.to_string()).collect());
        }

        Ok(FileContent::Tabular(TabularData {
            headers,
            rows,
            sheet_name: None,
        }))
    }

    fn parse_csv_bytes(data: &[u8]) -> Result<FileContent> {
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .has_headers(true)
            .from_reader(Cursor::new(data));

        let headers: Option<Vec<String>> = reader
            .headers()
            .ok()
            .map(|h| h.iter().map(|s| s.to_string()).collect());

        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result?;
            rows.push(record.iter().map(|s| s.to_string()).collect());
        }

        Ok(FileContent::Tabular(TabularData {
            headers,
            rows,
            sheet_name: None,
        }))
    }

    fn parse_xlsx(path: &str) -> Result<FileContent> {
        let mut workbook: Xlsx<_> = open_workbook(path)?;
        Self::parse_workbook(&mut workbook)
    }

    fn parse_xlsx_bytes(data: &[u8]) -> Result<FileContent> {
        let cursor = Cursor::new(data);
        let mut workbook: Xlsx<_> = Xlsx::new(cursor)?;
        Self::parse_workbook(&mut workbook)
    }

    fn parse_xls(path: &str) -> Result<FileContent> {
        let mut workbook: Xls<_> =
            open_workbook(path).map_err(|e| ParseError(format!("Failed to open XLS: {}", e)))?;
        Self::parse_workbook(&mut workbook)
    }

    fn parse_xls_bytes(data: &[u8]) -> Result<FileContent> {
        let cursor = Cursor::new(data);
        let mut workbook: Xls<_> =
            Xls::new(cursor).map_err(|e| ParseError(format!("Failed to parse XLS: {}", e)))?;
        Self::parse_workbook(&mut workbook)
    }

    fn parse_workbook<RS: std::io::Read + std::io::Seek, R: Reader<RS>>(
        workbook: &mut R,
    ) -> Result<FileContent>
    where
        <R as calamine::Reader<RS>>::Error: std::fmt::Display,
    {
        let sheet_names = workbook.sheet_names().to_owned();

        if sheet_names.is_empty() {
            return Err(ParseError("No sheets found in workbook".to_string()));
        }

        let sheet_name = sheet_names[0].clone();
        let range = workbook
            .worksheet_range(&sheet_name)
            .map_err(|e| ParseError(format!("Failed to read worksheet: {}", e)))?;

        let mut rows: Vec<Vec<String>> = Vec::new();

        for row in range.rows() {
            let row_data: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
            rows.push(row_data);
        }

        let headers = if !rows.is_empty() {
            Some(rows.remove(0))
        } else {
            None
        };

        Ok(FileContent::Tabular(TabularData {
            headers,
            rows,
            sheet_name: Some(sheet_name),
        }))
    }

    fn parse_text(path: &str) -> Result<FileContent> {
        let content = std::fs::read_to_string(path)?;
        Ok(FileContent::Text(content))
    }

    fn parse_text_bytes(data: &[u8]) -> Result<FileContent> {
        let content = String::from_utf8_lossy(data).to_string();
        Ok(FileContent::Text(content))
    }
}

pub fn content_to_csv(content: &FileContent) -> Result<String> {
    match content {
        FileContent::Tabular(data) => {
            let mut wtr = csv::Writer::from_writer(vec![]);

            if let Some(headers) = &data.headers {
                wtr.write_record(headers)?;
            }

            for row in &data.rows {
                wtr.write_record(row)?;
            }

            let bytes = wtr.into_inner()?;
            Ok(String::from_utf8(bytes)?)
        }
        FileContent::Text(text) => Ok(text.clone()),
    }
}
