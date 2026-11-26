//! CSV (Excel-friendly, formula-injection safe) and JSON export of the displayed rows.

use crate::model::PortEntry;
use serde_json::{Value, json};

const HEADERS: [&str; 7] = [
    "Port",
    "Protocole",
    "Adresse",
    "Application",
    "PID",
    "RAM (Mo)",
    "Chemin",
];

/// Quotes a cell, doubles quotes and neutralises spreadsheet formulas (= + - @ tab CR).
fn cell(s: &str) -> String {
    let guarded = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_string()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

/// UTF-8 with BOM so Excel detects the encoding.
pub fn csv(rows: &[PortEntry]) -> Vec<u8> {
    let mut out = String::from("\u{feff}");
    let line =
        |cells: Vec<String>| cells.iter().map(|c| cell(c)).collect::<Vec<_>>().join(",") + "\r\n";
    out += &line(HEADERS.iter().map(|h| h.to_string()).collect());
    for r in rows {
        out += &line(vec![
            r.port.to_string(),
            r.proto.as_str().into(),
            r.address.clone(),
            r.name.clone(),
            r.pid.to_string(),
            r.memory_mb.map(|m| m.to_string()).unwrap_or_default(),
            r.path.clone().unwrap_or_default(),
        ]);
    }
    out.into_bytes()
}

/// Same keys as the v1 export.
pub fn json(rows: &[PortEntry]) -> String {
    let arr: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "port": r.port,
                "protocol": r.proto.as_str(),
                "address": r.address,
                "processName": r.name,
                "pid": r.pid,
                "memoryMB": r.memory_mb,
                "exePath": r.path,
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into())
}

/// openkiller-ports-AAAAMMJJ-HHMMSS.<ext>
pub fn file_name(t: [u16; 6], ext: &str) -> String {
    format!(
        "openkiller-ports-{:04}{:02}{:02}-{:02}{:02}{:02}.{ext}",
        t[0], t[1], t[2], t[3], t[4], t[5]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Proto;

    fn row(name: &str, path: Option<&str>) -> PortEntry {
        PortEntry {
            port: 8123,
            proto: Proto::Tcp,
            address: "0.0.0.0:8123".into(),
            pid: 42,
            name: name.into(),
            memory_mb: Some(12),
            path: path.map(str::to_string),
        }
    }

    #[test]
    fn csv_escaping() {
        assert_eq!(cell("a\"b"), "\"a\"\"b\"");
        assert_eq!(cell("a,b\nc"), "\"a,b\nc\"");
        assert_eq!(cell("=cmd|' /C calc'!A0"), "\"'=cmd|' /C calc'!A0\"");
        assert_eq!(cell("+1"), "\"'+1\"");
        assert_eq!(cell("-1"), "\"'-1\"");
        assert_eq!(cell("@SUM"), "\"'@SUM\"");
        assert_eq!(cell("\t=1"), "\"'\t=1\"");
        assert_eq!(cell("\r=1"), "\"'\r=1\"");
        assert_eq!(cell("C:\\x.exe"), "\"C:\\x.exe\"");
    }

    #[test]
    fn csv_layout() {
        let bytes = csv(&[row("=evil", None)]);
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let text = String::from_utf8(bytes).unwrap();
        let mut lines = text.trim_start_matches('\u{feff}').lines();
        assert_eq!(
            lines.next().unwrap(),
            "\"Port\",\"Protocole\",\"Adresse\",\"Application\",\"PID\",\"RAM (Mo)\",\"Chemin\""
        );
        assert_eq!(
            lines.next().unwrap(),
            "\"8123\",\"TCP\",\"0.0.0.0:8123\",\"'=evil\",\"42\",\"12\",\"\""
        );
    }

    #[test]
    fn json_keys_and_nulls() {
        let v: Value = serde_json::from_str(&json(&[row("python", None)])).unwrap();
        let o = &v[0];
        assert_eq!(o["port"], 8123);
        assert_eq!(o["protocol"], "TCP");
        assert_eq!(o["processName"], "python");
        assert_eq!(o["memoryMB"], 12);
        assert!(o["exePath"].is_null());
    }

    #[test]
    fn names() {
        assert_eq!(
            file_name([2026, 11, 6, 9, 5, 3], "csv"),
            "openkiller-ports-20261106-090503.csv"
        );
    }
}
