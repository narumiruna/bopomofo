use crate::{Error, Result, files, write_json};
use ime_core::{Engine, Event, Lexicon, Output};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;

const MAX_EVENTS: usize = 100_000;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub(crate) event: Event,
    #[serde(default, deserialize_with = "present_expectation")]
    expect: Option<Value>,
}

// An explicit JSON null is an expectation, not an absent field.
fn present_expectation<'de, D>(deserializer: D) -> std::result::Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

pub(crate) struct Case {
    pub(crate) records: Vec<Record>,
    pub(crate) sha256: String,
    pub(crate) bytes: usize,
    pub(crate) keys: usize,
}

impl Case {
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let bytes = files::read_input(path)?;
        let text =
            std::str::from_utf8(&bytes).map_err(|_| Error::new("invalid_corpus_encoding"))?;
        let mut records = Vec::new();
        let mut keys = 0;
        for (index, line) in text.lines().enumerate() {
            let number = index + 1;
            if records.len() == MAX_EVENTS {
                return Err(Error::at_line("event_limit", number));
            }
            // Keep serde_json's default recursion limit. Do not display parser
            // errors: they can contain fixture text or unknown field names.
            let record: Record =
                serde_json::from_str(line).map_err(|_| Error::at_line("invalid_record", number))?;
            keys += usize::from(matches!(&record.event, Event::Key(_)));
            records.push(record);
        }
        if records.is_empty() {
            return Err(Error::new("empty_corpus"));
        }
        Ok(Self {
            records,
            sha256: files::checksum(&bytes),
            bytes: bytes.len(),
            keys,
        })
    }
}

impl Record {
    pub(crate) fn check(&self, output: &Output, line: usize) -> Result<()> {
        if let Some(expected) = &self.expect {
            let actual = serde_json::to_value(output)
                .map_err(|_| Error::at_line("output_serialization_failed", line))?;
            if !subset_matches(expected, &actual) {
                return Err(Error::at_line("expectation_mismatch", line));
            }
        }
        Ok(())
    }
}

fn subset_matches(expected: &Value, actual: &Value) -> bool {
    match (expected, actual) {
        (Value::Object(expected), Value::Object(actual)) => expected.iter().all(|(key, value)| {
            actual
                .get(key)
                .is_some_and(|actual| subset_matches(value, actual))
        }),
        (Value::Array(expected), Value::Array(actual)) => {
            expected.len() == actual.len()
                && expected
                    .iter()
                    .zip(actual)
                    .all(|(expected, actual)| subset_matches(expected, actual))
        }
        _ => expected == actual,
    }
}

pub(crate) fn run(lexicon: Arc<Lexicon>, case: &Case, output: &mut impl Write) -> Result<()> {
    let mut engine = Engine::new(lexicon);
    for (index, record) in case.records.iter().enumerate() {
        let result = engine.handle(record.event.clone());
        record.check(&result, index + 1)?;
        write_json(output, &result)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::subset_matches;
    use serde_json::json;

    #[test]
    fn objects_are_recursive_subsets() {
        let actual = json!({"snapshot": {"candidates": [{"text": "自製", "start": 0, "end": 1}]}, "commit": null});
        assert!(subset_matches(
            &json!({"snapshot": {"candidates": [{"text": "自製"}]}}),
            &actual
        ));
        assert!(subset_matches(&json!({"commit": null}), &actual));
        assert!(!subset_matches(
            &json!({"snapshot": {"candidates": [{"text": "其他"}]}}),
            &actual
        ));
        assert!(!subset_matches(&json!({"missing": null}), &actual));
    }

    #[test]
    fn arrays_require_equal_lengths_and_order() {
        assert!(subset_matches(
            &json!([{"a": 1}]),
            &json!([{"a": 1, "b": 2}])
        ));
        assert!(!subset_matches(&json!([]), &json!([1])));
        assert!(!subset_matches(&json!([1]), &json!([1, 2])));
        assert!(!subset_matches(&json!([1, 2]), &json!([2, 1])));
    }

    #[test]
    fn primitives_and_types_match_exactly() {
        assert!(subset_matches(&json!(false), &json!(false)));
        assert!(!subset_matches(&json!(false), &json!(0)));
        assert!(!subset_matches(&json!({}), &json!([])));
        assert!(!subset_matches(&json!(1), &json!(1.0)));
    }
}
