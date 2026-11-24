use serde_json::Value;

pub fn token_usage_log(usage: &Value, total_cost_usd: Option<&Value>) -> Option<String> {
    let input_tokens = usage["input_tokens"].as_u64()?;
    let output_tokens = usage["output_tokens"].as_u64()?;
    let cached_tokens = usage["cached_input_tokens"].as_u64().unwrap_or_else(|| {
        usage["cache_read_input_tokens"]
            .as_u64()
            .unwrap_or_default()
            .saturating_add(
                usage["cache_creation_input_tokens"]
                    .as_u64()
                    .unwrap_or_default(),
            )
    });
    let reasoning_tokens = usage["reasoning_output_tokens"]
        .as_u64()
        .or_else(|| usage["thinking_tokens"].as_u64());
    let mut line = format!("Token: vào {input_tokens} (cache {cached_tokens}), ra {output_tokens}");
    if let Some(reasoning_tokens) = reasoning_tokens {
        line.push_str(&format!(" (suy luận {reasoning_tokens})"));
    }
    if let Some(cost) = total_cost_usd.filter(|value| !value.is_null()) {
        let cost = format_cost(cost);
        line.push_str(&format!(", chi phí ${cost}"));
    }
    Some(line)
}

fn format_cost(cost: &Value) -> String {
    let number = cost.as_f64().or_else(|| cost.as_str()?.parse::<f64>().ok());
    if let Some(number) = number.filter(|number| number.is_finite()) {
        let rounded = format!("{number:.4}");
        return rounded
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
    }
    cost.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| cost.to_string())
}
