//! The values of variables: their types, as a script knows them, how they
//! are shown and read, and turning one into another type.


/// The types a variable can be, by the names a script knows them by,
/// which are the same in every language.
pub(in crate::tab) const TYPES: [&str; 5] = ["string", "number", "boolean", "array", "table"];

pub(in crate::tab) const NUMBER: usize = 1;

pub(in crate::tab) const BOOLEAN: usize = 2;

pub(in crate::tab) const ARRAY: usize = 3;

pub(in crate::tab) const TABLE: usize = 4;

/// A variable's value as it is shown to be edited: text as it is, the
/// rest as JSON writes it.
pub(in crate::tab) fn shown_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_default()
        }
        other => other.to_string(),
    }
}

/// What was typed as a variable's value: a number, true or false, a table
/// if it reads as one, and text otherwise.
pub(in crate::tab) fn read_value(typed: &str) -> serde_json::Value {
    let trimmed = typed.trim();
    let table = trimmed.starts_with(['{', '[']);
    let plain =
        matches!(trimmed, "true" | "false") || trimmed.parse::<f64>().is_ok_and(f64::is_finite);
    let read = (table || plain)
        .then(|| serde_json::from_str(trimmed).ok())
        .flatten();
    read.unwrap_or_else(|| serde_json::Value::String(typed.to_owned()))
}

/// Which of [`TYPES`] a value is.
pub(in crate::tab) fn kind_of(value: &serde_json::Value) -> usize {
    use serde_json::Value;
    match value {
        Value::Number(_) => NUMBER,
        Value::Bool(_) => BOOLEAN,
        Value::Array(_) => ARRAY,
        Value::Object(_) => TABLE,
        Value::String(_) | Value::Null => 0,
    }
}

/// A value as another type, keeping of it what that type can hold.
pub(in crate::tab) fn converted(value: &serde_json::Value, kind: usize) -> serde_json::Value {
    use serde_json::Value;
    let truthy = |value: &Value| match value {
        Value::Bool(flag) => *flag,
        Value::Number(number) => number.as_f64() != Some(0.0),
        Value::String(text) => !matches!(
            text.trim().to_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        ),
        Value::Array(items) => !items.is_empty(),
        Value::Object(fields) => !fields.is_empty(),
        Value::Null => false,
    };
    let empty =
        |value: &Value| matches!(value, Value::String(text) if text.is_empty()) || value.is_null();
    match kind {
        NUMBER => {
            let number = match value {
                Value::Number(number) => number.as_f64(),
                Value::String(text) => text.trim().parse().ok(),
                Value::Bool(flag) => Some(f64::from(u8::from(*flag))),
                _ => None,
            };
            read_value(
                &number
                    .filter(|number| number.is_finite())
                    .unwrap_or(0.0)
                    .to_string(),
            )
        }
        BOOLEAN => Value::Bool(truthy(value)),
        ARRAY => match value {
            Value::Array(_) => value.clone(),
            Value::Object(fields) => Value::Array(fields.values().cloned().collect()),
            other if empty(other) => Value::Array(Vec::new()),
            other => Value::Array(vec![other.clone()]),
        },
        TABLE => match value {
            Value::Object(_) => value.clone(),
            Value::Array(items) => {
                let numbered = items
                    .iter()
                    .enumerate()
                    .map(|(place, item)| ((place + 1).to_string(), item.clone()));
                Value::Object(numbered.collect())
            }
            other if empty(other) => Value::Object(serde_json::Map::new()),
            other => Value::Object([("value".to_owned(), other.clone())].into_iter().collect()),
        },
        _ => Value::String(shown_value(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_read_as_what_they_look_like() {
        use serde_json::json;
        assert_eq!(read_value(" 42 "), json!(42));
        assert_eq!(read_value("3.5"), json!(3.5));
        assert_eq!(read_value("true"), json!(true));
        assert_eq!(read_value("{\"hp\": 10}"), json!({"hp": 10}));
        assert_eq!(read_value("[1, 2]"), json!([1, 2]));
        // What only looks like one of those is text, kept as typed.
        assert_eq!(read_value("{broken"), json!("{broken"));
        assert_eq!(read_value("orc chieftain "), json!("orc chieftain "));
        assert_eq!(read_value("NaN"), json!("NaN"));
        assert_eq!(read_value(""), json!(""));
        // And what is shown reads back as what it was.
        for value in [json!(42), json!(true), json!("orc"), json!({"a": [1, 2]})] {
            assert_eq!(read_value(&shown_value(&value)), value);
        }
    }

    #[test]
    fn a_variable_changes_type_keeping_what_the_new_one_can_hold() {
        use serde_json::json;
        let to = |value: serde_json::Value, kind: usize| converted(&value, kind);
        assert_eq!(to(json!("12"), NUMBER), json!(12));
        assert_eq!(to(json!("orc"), NUMBER), json!(0));
        assert_eq!(to(json!(true), NUMBER), json!(1));
        assert_eq!(to(json!(7.5), 0), json!("7.5"));
        assert_eq!(to(json!("no"), BOOLEAN), json!(false));
        assert_eq!(to(json!("yes"), BOOLEAN), json!(true));
        assert_eq!(to(json!(0), BOOLEAN), json!(false));
        assert_eq!(to(json!("orc"), ARRAY), json!(["orc"]));
        assert_eq!(to(json!(""), ARRAY), json!([]));
        assert_eq!(to(json!({"a": 1, "b": 2}), ARRAY), json!([1, 2]));
        assert_eq!(to(json!(["axe", 3]), TABLE), json!({"1": "axe", "2": 3}));
        assert_eq!(to(json!(5), TABLE), json!({"value": 5}));
        // Each is the type it was made into, and stays so made again.
        for kind in 0..TYPES.len() {
            let made = to(json!("12"), kind);
            assert_eq!(kind_of(&made), kind);
            assert_eq!(to(made.clone(), kind), made);
        }
    }
}
