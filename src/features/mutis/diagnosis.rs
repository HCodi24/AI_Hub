use serde_json::{json, Value};

pub fn normalize_error(value: &str) -> String {
    let mut out = String::new();
    let mut digits = false;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            if !digits {
                out.push('#');
            }
            digits = true;
        } else {
            digits = false;
            out.push(ch);
        }
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

#[allow(dead_code)]
pub fn parse_diagnosis(text: &str) -> anyhow::Result<serde_json::Value> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("Thiếu JSON"))?;
    let mut depth = 0;
    for (offset, ch) in text[start..].char_indices() {
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let value: serde_json::Value =
                    serde_json::from_str(&text[start..start + offset + 1])?;
                if !matches!(value["health"].as_str(), Some("tốt" | "trung bình" | "kém"))
                    || !value["findings"].is_array()
                {
                    anyhow::bail!("Sai cấu trúc chẩn đoán");
                }
                return validate_diagnosis(value);
            }
        }
    }
    anyhow::bail!("JSON chưa hoàn chỉnh")
}

pub fn validate_diagnosis(value: Value) -> anyhow::Result<Value> {
    if !matches!(value["health"].as_str(), Some("tốt" | "trung bình" | "kém"))
        || !value["findings"].is_array()
        || !value["summary"].is_string()
    {
        anyhow::bail!("Cấu trúc kết quả chẩn đoán không hợp lệ");
    }
    let findings = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let field = |name: &str| {
                finding[name]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Trường phát hiện không hợp lệ: {name}"))
            };
            let severity = field("severity")?;
            if !matches!(severity, "high" | "medium" | "low") {
                anyhow::bail!("Mức độ nghiêm trọng của phát hiện không hợp lệ");
            }
            let setting_change_value = finding
                .get("setting_change")
                .ok_or_else(|| anyhow::anyhow!("Thiếu thay đổi thiết lập"))?;
            let setting_change = match setting_change_value {
                Value::Null => Value::Null,
                change if change["path"].is_string() && change.get("value").is_some() => {
                    json!({"path": change["path"], "value": change["value"]})
                }
                _ => anyhow::bail!("Cấu trúc thay đổi thiết lập không hợp lệ"),
            };
            Ok(json!({
                "problem": field("problem")?,
                "evidence": field("evidence")?,
                "cause": field("cause")?,
                "severity": severity,
                "suggestion": field("suggestion")?,
                "setting_change": setting_change
            }))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(json!({"health": value["health"], "findings": findings, "summary": value["summary"]}))
}
