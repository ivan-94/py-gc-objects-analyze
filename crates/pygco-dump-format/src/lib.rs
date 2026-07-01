use std::io::{BufRead, Lines};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const FORMAT_NAME: &str = "pygco-dump-jsonl";
pub const FORMAT_MAJOR_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum DumpFormatError {
    #[error("line {line}: malformed json: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("line {line}: missing field `{field}`")]
    MissingField { line: usize, field: &'static str },
    #[error("line {line}: invalid record type `{record_type}`")]
    InvalidRecordType { line: usize, record_type: String },
    #[error("line {line}: invalid metadata phase `{phase}`")]
    InvalidPhase { line: usize, phase: String },
    #[error("line {line}: unsupported dump format `{format}`")]
    UnsupportedFormat { line: usize, format: String },
    #[error("line {line}: unsupported dump format version `{version}`")]
    UnsupportedVersion { line: usize, version: u32 },
    #[error("line {line}: io error: {source}")]
    Io {
        line: usize,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "record_type")]
pub enum DumpRecord {
    #[serde(rename = "metadata")]
    Metadata(MetadataRecord),
    #[serde(rename = "object")]
    Object(ObjectRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "phase")]
pub enum MetadataRecord {
    #[serde(rename = "start")]
    Start(Box<MetadataStart>),
    #[serde(rename = "end")]
    End(MetadataEnd),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataStart {
    pub format: String,
    pub format_version: u32,
    pub producer: String,
    pub producer_version: String,
    pub producer_run_id: String,
    pub dump_sequence: u64,
    pub created_at: String,
    pub process_started_at: Option<String>,
    pub host_id: Option<String>,
    pub container_id: Option<String>,
    pub pid: u32,
    pub python_version: String,
    pub platform: String,
    pub collect_before_dump: bool,
    pub include_referents: bool,
    pub include_referent_stubs: bool,
    pub include_repr: bool,
    pub repr_limit: u64,
    pub object_count: u64,
    #[serde(default)]
    pub gc_count: Option<Value>,
    #[serde(default)]
    pub gc_stats: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MetadataEnd {
    pub dumped_count: u64,
    pub stub_count: u64,
    pub total_object_records: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectRecord {
    pub id: i64,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub module: Option<String>,
    #[serde(default)]
    pub qualname: Option<String>,
    pub size: Option<i64>,
    #[serde(default)]
    pub gc_tracked: Option<bool>,
    #[serde(default)]
    pub stub: bool,
    #[serde(default)]
    pub referents: Vec<i64>,
    #[serde(default)]
    pub repr: Option<String>,
}

pub fn parse_line(line: &str, line_number: usize) -> Result<DumpRecord, DumpFormatError> {
    let value: Value = serde_json::from_str(line).map_err(|source| DumpFormatError::Json {
        line: line_number,
        source,
    })?;
    let record_type =
        value
            .get("record_type")
            .and_then(Value::as_str)
            .ok_or(DumpFormatError::MissingField {
                line: line_number,
                field: "record_type",
            })?;
    match record_type {
        "metadata" => parse_metadata(value, line_number),
        "object" => serde_json::from_value(value).map_err(|source| DumpFormatError::Json {
            line: line_number,
            source,
        }),
        other => Err(DumpFormatError::InvalidRecordType {
            line: line_number,
            record_type: other.to_owned(),
        }),
    }
}

fn parse_metadata(value: Value, line_number: usize) -> Result<DumpRecord, DumpFormatError> {
    let phase =
        value
            .get("phase")
            .and_then(Value::as_str)
            .ok_or(DumpFormatError::MissingField {
                line: line_number,
                field: "phase",
            })?;
    match phase {
        "start" => {
            let start: MetadataStart =
                serde_json::from_value(value).map_err(|source| DumpFormatError::Json {
                    line: line_number,
                    source,
                })?;
            validate_start(&start, line_number)?;
            Ok(DumpRecord::Metadata(MetadataRecord::Start(Box::new(start))))
        }
        "end" => serde_json::from_value(value).map_err(|source| DumpFormatError::Json {
            line: line_number,
            source,
        }),
        other => Err(DumpFormatError::InvalidPhase {
            line: line_number,
            phase: other.to_owned(),
        }),
    }
}

pub fn validate_start(start: &MetadataStart, line_number: usize) -> Result<(), DumpFormatError> {
    if start.format != FORMAT_NAME {
        return Err(DumpFormatError::UnsupportedFormat {
            line: line_number,
            format: start.format.clone(),
        });
    }
    if start.format_version != FORMAT_MAJOR_VERSION {
        return Err(DumpFormatError::UnsupportedVersion {
            line: line_number,
            version: start.format_version,
        });
    }
    Ok(())
}

pub struct DumpRecordLines<R: BufRead> {
    lines: Lines<R>,
    line_number: usize,
}

impl<R: BufRead> DumpRecordLines<R> {
    pub fn new(reader: R) -> Self {
        Self {
            lines: reader.lines(),
            line_number: 0,
        }
    }
}

impl<R: BufRead> Iterator for DumpRecordLines<R> {
    type Item = Result<(usize, DumpRecord), DumpFormatError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next_line = self.lines.next()?;
            self.line_number += 1;
            match next_line {
                Ok(line) if line.trim().is_empty() => continue,
                Ok(line) => {
                    return Some(parse_line(&line, self.line_number).map(|r| (self.line_number, r)))
                }
                Err(source) => {
                    return Some(Err(DumpFormatError::Io {
                        line: self.line_number,
                        source,
                    }))
                }
            }
        }
    }
}

pub fn object_id_json(id: i64) -> String {
    id.to_string()
}

pub fn split_type(
    type_name: &str,
    module: Option<&str>,
    qualname: Option<&str>,
) -> (String, String) {
    if let (Some(module), Some(qualname)) = (module, qualname) {
        return (empty_as_builtins(module), empty_as_unknown(qualname));
    }
    if let Some(module) = module {
        let q = type_name
            .strip_prefix(module)
            .and_then(|value| value.strip_prefix('.'))
            .unwrap_or(type_name);
        return (empty_as_builtins(module), empty_as_unknown(q));
    }
    if let Some((module, qualname)) = type_name.rsplit_once('.') {
        return (empty_as_builtins(module), empty_as_unknown(qualname));
    }
    ("builtins".to_owned(), empty_as_unknown(type_name))
}

fn empty_as_builtins(value: &str) -> String {
    if value.is_empty() {
        "builtins".to_owned()
    } else {
        value.to_owned()
    }
}

fn empty_as_unknown(value: &str) -> String {
    if value.is_empty() {
        "<unknown>".to_owned()
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_start_object_end_records() {
        let start = r#"{"record_type":"metadata","phase":"start","format":"pygco-dump-jsonl","format_version":1,"producer":"pygco_dump","producer_version":"0.1.0","producer_run_id":"run","dump_sequence":1,"created_at":"2026-07-01T00:00:00Z","pid":1,"python_version":"3.12","platform":"test","collect_before_dump":false,"include_referents":true,"include_referent_stubs":true,"include_repr":false,"repr_limit":0,"object_count":1}"#;
        let object = r#"{"record_type":"object","id":1,"type":"dict","module":"builtins","qualname":"dict","size":64,"gc_tracked":true,"stub":false,"referents":[2]}"#;
        let end = r#"{"record_type":"metadata","phase":"end","dumped_count":1,"stub_count":0,"total_object_records":1,"elapsed_ms":1}"#;

        assert!(matches!(
            parse_line(start, 1).unwrap(),
            DumpRecord::Metadata(MetadataRecord::Start(_))
        ));
        assert!(matches!(
            parse_line(object, 2).unwrap(),
            DumpRecord::Object(_)
        ));
        assert!(matches!(
            parse_line(end, 3).unwrap(),
            DumpRecord::Metadata(MetadataRecord::End(_))
        ));
    }

    #[test]
    fn rejects_unknown_major_version() {
        let start = r#"{"record_type":"metadata","phase":"start","format":"pygco-dump-jsonl","format_version":2,"producer":"pygco_dump","producer_version":"0.1.0","producer_run_id":"run","dump_sequence":1,"created_at":"2026-07-01T00:00:00Z","pid":1,"python_version":"3.12","platform":"test","collect_before_dump":false,"include_referents":true,"include_referent_stubs":true,"include_repr":false,"repr_limit":0,"object_count":1}"#;
        assert!(matches!(
            parse_line(start, 1).unwrap_err(),
            DumpFormatError::UnsupportedVersion { .. }
        ));
    }

    #[test]
    fn accepts_optional_forward_fields() {
        let start = r#"{"record_type":"metadata","phase":"start","format":"pygco-dump-jsonl","format_version":1,"producer":"pygco_dump","producer_version":"0.1.0","producer_run_id":"run","dump_sequence":1,"created_at":"2026-07-01T00:00:00Z","pid":1,"python_version":"3.12","platform":"test","collect_before_dump":false,"include_referents":true,"include_referent_stubs":true,"include_repr":false,"repr_limit":0,"object_count":1,"future_optional":"ok"}"#;
        assert!(parse_line(start, 1).is_ok());
    }

    #[test]
    fn malformed_json_reports_line_number() {
        let error = parse_line("{", 42).unwrap_err();
        assert!(matches!(error, DumpFormatError::Json { line: 42, .. }));
        assert!(error.to_string().contains("line 42"));
    }
}
