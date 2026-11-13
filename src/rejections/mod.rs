//! The rejection dataset shipped as data: rows, columns, licence and summary
//! keys unchanged from rejection-taxonomy v0.6.1 (data byte-identical to
//! v0.6.0).
//!
//! priorart uses this as the descriptive layer - the vocabulary the mechanism
//! estimates are keyed to - never as headline evidence.

use serde_json::Value;

pub const SUMMARY_JSON: &str = include_str!("../../data/rejections/summary.json");
pub const PROVENANCE_JSON: &str = include_str!("../../data/rejections/provenance.json");
pub const REJECTIONS_CSV: &str = include_str!("../../data/rejections/rejections.csv");
pub const SCHEMA_MD: &str = include_str!("../../data/rejections/SCHEMA.md");
pub const LICENSE_DATA: &str = include_str!("../../data/rejections/LICENSE-DATA");

pub fn summary() -> &'static Value {
    static DOC: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DOC.get_or_init(|| {
        serde_json::from_str(SUMMARY_JSON)
            .unwrap_or_else(|e| panic!("data/rejections/summary.json did not parse: {e}"))
    })
}

pub fn provenance() -> &'static Value {
    static DOC: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
    DOC.get_or_init(|| {
        serde_json::from_str(PROVENANCE_JSON)
            .unwrap_or_else(|e| panic!("data/rejections/provenance.json did not parse: {e}"))
    })
}

pub fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// The number of data rows in the CSV (header excluded).
pub fn row_count() -> usize {
    REJECTIONS_CSV
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count()
        - 1
}

/// The CSV header columns, in file order.
pub fn columns() -> Vec<&'static str> {
    REJECTIONS_CSV
        .lines()
        .next()
        .map(|h| h.split(',').collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dataset_keeps_its_row_and_column_contract() {
        assert_eq!(row_count(), 113, "113 logged rows");
        assert_eq!(
            columns(),
            vec![
                "id",
                "family_id",
                "bug_family",
                "mechanism_class",
                "triage_class",
                "earliest_gate",
                "root_cause_class",
                "repeat_of_own",
                "duplicate_source",
                "preventable_by",
            ]
        );
        for row in REJECTIONS_CSV
            .lines()
            .skip(1)
            .filter(|l| !l.trim().is_empty())
        {
            assert_eq!(row.split(',').count(), 10, "row: {row}");
        }
    }

    #[test]
    fn summary_keys_are_unchanged_and_total_matches_the_rows() {
        let s = summary();
        for key in [
            "total",
            "prehunt_catchable",
            "prehunt_catchable_pct",
            "withheld_rows",
            "effective_n",
            "triage",
            "gate",
            "bug_family",
            "mechanism_class",
            "duplicate_source",
            "preventable_by",
        ] {
            assert!(s.get(key).is_some(), "summary.json lost the key {key}");
        }
        assert_eq!(u64_field(s, "total"), 113);
        assert_eq!(u64_field(s, "published_rows"), 110);
        assert_eq!(u64_field(s, "withheld_rows"), 3);
        // The CSV carries every logged row; three of them have a withheld
        // payload, so published_rows + withheld_rows equals the row count.
        assert_eq!(row_count(), 113);
        assert_eq!(
            u64_field(s, "published_rows") + u64_field(s, "withheld_rows"),
            row_count() as u64
        );
    }

    #[test]
    fn provenance_records_v061_and_the_unchanged_source_hash() {
        let p = provenance();
        assert_eq!(
            p.get("source_sha256").and_then(Value::as_str),
            Some("8dc5ea43ee90382919a15a0a76cc8a063685320d75cc9f5d47a5798e08991825")
        );
        assert_eq!(
            p.get("dataset_version").and_then(Value::as_str),
            Some("0.6.1")
        );
        assert_eq!(
            p.get("data_identical_to").and_then(Value::as_str),
            Some("0.6.0")
        );
    }

    #[test]
    fn the_licence_and_schema_ship_with_the_data() {
        assert!(LICENSE_DATA.contains("CC BY 4.0"));
        assert!(SCHEMA_MD.contains("data/rejections.csv"));
    }
}
