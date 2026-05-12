/// Runtime policy controlling token budgets and automatic compaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextPolicy {
    pub auto_compaction_enabled: bool,
    pub model_context_window_tokens: Option<usize>,
    pub max_output_tokens: usize,
    pub reasoning_reserve_tokens: usize,
    pub safety_margin_tokens: usize,
    pub soft_compact_ratio_percent: u8,
    pub hard_compact_ratio_percent: u8,
    pub soft_compact_at_tokens: Option<usize>,
    pub auto_compact_at_tokens: Option<usize>,
    pub latest_turns_to_protect: usize,
    pub summary_max_tokens: usize,
    pub summary_input_safety_margin_tokens: usize,
    pub max_summary_passes_per_request: usize,
}

/// Derived token budget for one provider request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextBudget {
    pub model_context_window_tokens: usize,
    pub max_output_tokens: usize,
    pub reasoning_reserve_tokens: usize,
    pub safety_margin_tokens: usize,
    pub usable_input_tokens: usize,
    pub soft_compact_at_tokens: usize,
    pub hard_compact_at_tokens: usize,
}

impl Default for ContextPolicy {
    /// Returns a conservative compaction policy with automatic summaries enabled.
    fn default() -> Self {
        Self {
            auto_compaction_enabled: true,
            model_context_window_tokens: None,
            max_output_tokens: 4096,
            reasoning_reserve_tokens: 0,
            safety_margin_tokens: 1024,
            soft_compact_ratio_percent: 50,
            hard_compact_ratio_percent: 75,
            soft_compact_at_tokens: None,
            auto_compact_at_tokens: None,
            latest_turns_to_protect: 6,
            summary_max_tokens: 2500,
            summary_input_safety_margin_tokens: 1024,
            max_summary_passes_per_request: 1,
        }
    }
}

impl ContextPolicy {
    /// Derives a concrete context budget when the model window is known.
    pub fn budget(&self) -> Option<ContextBudget> {
        let model_context_window_tokens = self.model_context_window_tokens?;
        let usable_input_tokens = model_context_window_tokens
            .saturating_sub(self.max_output_tokens)
            .saturating_sub(self.reasoning_reserve_tokens)
            .saturating_sub(self.safety_margin_tokens);

        Some(ContextBudget {
            model_context_window_tokens,
            max_output_tokens: self.max_output_tokens,
            reasoning_reserve_tokens: self.reasoning_reserve_tokens,
            safety_margin_tokens: self.safety_margin_tokens,
            usable_input_tokens,
            soft_compact_at_tokens: ratio_tokens(
                usable_input_tokens,
                self.soft_compact_ratio_percent,
            ),
            hard_compact_at_tokens: ratio_tokens(
                usable_input_tokens,
                self.hard_compact_ratio_percent,
            ),
        })
    }

    /// Returns the threshold that starts automatic compaction, if configured.
    pub fn active_compaction_threshold(&self) -> Option<usize> {
        if !self.auto_compaction_enabled {
            return None;
        }

        if let Some(threshold) = self.auto_compact_at_tokens {
            return Some(threshold);
        }

        self.budget().map(|budget| budget.hard_compact_at_tokens)
    }

    /// Returns the soft diagnostic threshold without triggering compaction.
    pub fn soft_compaction_threshold(&self) -> Option<usize> {
        if let Some(threshold) = self.soft_compact_at_tokens {
            return Some(threshold);
        }

        self.budget().map(|budget| budget.soft_compact_at_tokens)
    }

    /// Returns the maximum old transcript tokens to send to a summary request.
    pub fn summary_source_token_limit(&self) -> Option<usize> {
        self.budget().map(|budget| {
            budget
                .usable_input_tokens
                .saturating_sub(self.summary_max_tokens)
                .saturating_sub(self.summary_input_safety_margin_tokens)
        })
    }
}

/// Applies an integer percentage to a token count without floating point drift.
fn ratio_tokens(tokens: usize, percent: u8) -> usize {
    tokens.saturating_mul(percent as usize) / 100
}
