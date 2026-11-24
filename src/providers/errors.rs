use serde_json::Value;

pub(crate) fn error_value_text(value: &Value) -> Option<String> {
    let error = value.get("error")?;
    if let Some(message) = error.as_str() {
        return Some(message.to_owned());
    }
    if let Some(message) = error.get("message").and_then(Value::as_str) {
        return Some(message.to_owned());
    }
    Some(error.to_string())
}

pub(crate) fn safe_error_body(body: &str, secret: &str) -> String {
    let mut body = if let Ok(mut value) = serde_json::from_str::<Value>(body) {
        redact_sensitive_fields(&mut value);
        value.to_string()
    } else {
        body.to_owned()
    };
    if !secret.is_empty() {
        body = body.replace(secret, "[redacted]");
    }
    let truncated = body.chars().take(500).collect::<String>();
    if truncated.trim().is_empty() {
        "(empty response body)".to_owned()
    } else {
        truncated
    }
}

fn redact_sensitive_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                let key = key.to_ascii_lowercase();
                if key.contains("key")
                    || key.contains("token")
                    || key.contains("secret")
                    || key.contains("authorization")
                {
                    *value = Value::String("[redacted]".to_owned());
                } else {
                    redact_sensitive_fields(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_sensitive_fields(value);
            }
        }
        _ => {}
    }
}
