/// Token usage diagnostics for one assembled provider context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextDiagnostics {
    pub total_input_tokens: usize,
    pub usable_input_tokens: Option<usize>,
    pub active_compaction_threshold: Option<usize>,
    pub soft_compaction_threshold: Option<usize>,
    pub max_output_tokens: usize,
    pub reasoning_reserve_tokens: usize,
    pub safety_margin_tokens: usize,
    pub message_count: usize,
    pub section_usage: Vec<ContextSectionUsage>,
    pub soft_compaction_would_trigger: bool,
    pub compaction_would_trigger: bool,
}

/// Token usage for one provider-context section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextSectionUsage {
    pub section: ContextSection,
    pub message_count: usize,
    pub estimated_tokens: usize,
}

/// Named sections that make up provider-visible context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextSection {
    System,
    Summary,
    Transcript,
    Continuation,
}
