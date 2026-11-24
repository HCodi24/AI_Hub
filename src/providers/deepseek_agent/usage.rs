use serde_json::Value;

/// Token counters of one agent run, summed over every API reply.
#[derive(Default)]
pub(super) struct Usage {
    input: u64,
    output: u64,
    cached: u64,
    reasoning: u64,
}

impl Usage {
    pub(super) fn add(&mut self, usage: &Value) {
        self.input = self.input.saturating_add(usage["prompt_tokens"].as_u64().unwrap_or(0));
        self.output = self.output.saturating_add(usage["completion_tokens"].as_u64().unwrap_or(0));
        // DeepSeek reports `prompt_cache_hit_tokens`; OpenAI-style hosts use `prompt_tokens_details.cached_tokens`.
        self.cached = self.cached.saturating_add(
            usage["prompt_cache_hit_tokens"].as_u64()
                .or(usage["prompt_tokens_details"]["cached_tokens"].as_u64())
                .unwrap_or(0),
        );
        self.reasoning = self.reasoning.saturating_add(usage["completion_tokens_details"]["reasoning_tokens"].as_u64().unwrap_or(0));
    }

    pub(super) fn line(&self) -> Option<String> {
        (self.input + self.output > 0).then(|| format!("Token: vào {} (cache {}), ra {} (suy luận {})", self.input, self.cached, self.output, self.reasoning))
    }
}
