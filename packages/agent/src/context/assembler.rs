use super::diagnostics::{ContextDiagnostics, ContextSection, ContextSectionUsage};
use super::policy::ContextPolicy;
use super::token_count::{TokenCounter, TokenCounterChoice};
use super::{transcript_messages_from_events, validate_context_limits, ContextLimitFailure};
use crate::event::{AgentEvent, ContextSummary};
use crate::store::Store;
use llms::{ProviderContextLimits, ProviderMessage, ProviderMessageRole};
use std::error::Error;
use std::fmt::{self, Display};

mod boundaries;
mod formatting;

use self::boundaries::{
    latest_user_prompt_start, protected_event_start, same_turn_summary_source_event_end,
    summary_source_event_end,
};
use self::formatting::{build_messages, format_messages_for_summary, format_summary_message};

/// Builds provider-visible working context from the durable transcript.
#[derive(Clone, Debug)]
pub struct ContextAssembler<C = TokenCounterChoice> {
    token_counter: C,
    policy: ContextPolicy,
}

/// Inputs needed to assemble one provider request context.
pub struct ContextAssemblyInput<'a> {
    pub system_prompt: String,
    pub store: &'a Store,
    pub provider_limits: ProviderContextLimits,
    pub continuation_prompt: Option<&'a str>,
}

/// Result of context assembly when the real provider request can proceed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssembledProviderContext {
    pub messages: Vec<ProviderMessage>,
    pub diagnostics: ContextDiagnostics,
}

/// Assembly outcome, including a request for summary compaction when needed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextAssembly {
    Ready(AssembledProviderContext),
    NeedsSummary(ContextSummaryRequest),
}

/// Compactable transcript prefix that should be summarized before the real request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextSummaryRequest {
    pub previous_summary: Option<ContextSummary>,
    pub source_event_start: usize,
    pub source_event_end: usize,
    pub transcript: String,
    pub estimated_tokens: usize,
    pub diagnostics: ContextDiagnostics,
}

/// Error returned when assembled context cannot be sent safely.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextAssemblyError {
    ProviderLimit(ContextLimitFailure),
    TokenBudgetExceeded {
        estimated_input_tokens: usize,
        usable_input_tokens: usize,
    },
}

impl Default for ContextAssembler<TokenCounterChoice> {
    /// Creates an assembler using default tiktoken-backed token counts and policy.
    fn default() -> Self {
        Self::new(TokenCounterChoice::default(), ContextPolicy::default())
    }
}

impl<C> ContextAssembler<C>
where
    C: TokenCounter,
{
    /// Creates an assembler with explicit token counter and context policy.
    pub fn new(token_counter: C, policy: ContextPolicy) -> Self {
        Self {
            token_counter,
            policy,
        }
    }

    /// Assembles provider context or identifies the transcript prefix to summarize first.
    pub fn assemble(
        &self,
        input: ContextAssemblyInput<'_>,
    ) -> Result<ContextAssembly, ContextAssemblyError> {
        let replay = replay_context(&input);
        let diagnostics = self.diagnostics(&replay.messages, replay.latest_summary.is_some());

        if diagnostics.compaction_would_trigger {
            if let Some(summary_request) =
                self.summary_request(input.store, replay.latest_summary, diagnostics.clone())
            {
                return Ok(ContextAssembly::NeedsSummary(summary_request));
            }
        }

        validate_context_limits(&replay.messages, input.provider_limits)
            .map_err(ContextAssemblyError::ProviderLimit)?;
        self.validate_token_budget(&diagnostics)?;

        Ok(ContextAssembly::Ready(AssembledProviderContext {
            messages: replay.messages,
            diagnostics,
        }))
    }

    /// Builds diagnostics by counting each provider-context section independently.
    fn diagnostics(&self, messages: &[ProviderMessage], has_summary: bool) -> ContextDiagnostics {
        let counts = SectionCounts::from_messages(messages, has_summary);
        let tokens = self.section_tokens(messages, &counts);
        let budget = self.policy.budget();
        let active_compaction_threshold = self.policy.active_compaction_threshold();
        let soft_compaction_threshold = self.policy.soft_compaction_threshold();

        ContextDiagnostics {
            total_input_tokens: tokens.total_input_tokens,
            usable_input_tokens: budget.map(|budget| budget.usable_input_tokens),
            active_compaction_threshold,
            soft_compaction_threshold,
            max_output_tokens: self.policy.max_output_tokens,
            reasoning_reserve_tokens: self.policy.reasoning_reserve_tokens,
            safety_margin_tokens: self.policy.safety_margin_tokens,
            message_count: messages.len(),
            section_usage: tokens.section_usage(&counts),
            soft_compaction_would_trigger: threshold_exceeded(
                tokens.total_input_tokens,
                soft_compaction_threshold,
            ),
            compaction_would_trigger: threshold_exceeded(
                tokens.total_input_tokens,
                active_compaction_threshold,
            ),
        }
    }

    /// Creates a summary request for compactable old transcript events, if any exist.
    fn summary_request(
        &self,
        store: &Store,
        latest_summary: Option<ContextSummary>,
        diagnostics: ContextDiagnostics,
    ) -> Option<ContextSummaryRequest> {
        let events = store.events();
        let replay_start = latest_summary
            .as_ref()
            .map(|summary| summary.source_event_end)
            .unwrap_or_default();
        let context = SummaryContext {
            events,
            replay_start,
            latest_summary,
            diagnostics,
        };
        if let Some(summary_request) = self.old_turn_summary_request(&context) {
            return Some(summary_request);
        }

        self.same_turn_summary_request(context)
    }

    /// Creates a summary request for compactable transcript before protected turns.
    fn old_turn_summary_request(
        &self,
        context: &SummaryContext<'_>,
    ) -> Option<ContextSummaryRequest> {
        let protect_start = protected_event_start(
            context.events,
            context.replay_start,
            self.policy.latest_turns_to_protect.max(1),
        );
        if protect_start <= context.replay_start {
            return None;
        }

        let summary_end = summary_source_event_end(
            context.events,
            context.replay_start,
            protect_start,
            self.policy.summary_source_token_limit(),
            &self.token_counter,
        );
        self.summary_request_for_range(SummaryRange {
            events: context.events,
            source_event_start: context.replay_start,
            source_event_end: summary_end,
            latest_summary: context.latest_summary.clone(),
            diagnostics: context.diagnostics.clone(),
            allow_empty_with_previous_summary: true,
        })
    }

    /// Creates a summary request for completed work inside the active protected turn.
    fn same_turn_summary_request(
        &self,
        context: SummaryContext<'_>,
    ) -> Option<ContextSummaryRequest> {
        let turn_start = latest_user_prompt_start(context.events, context.replay_start)
            .unwrap_or(context.replay_start);
        let summary_end = same_turn_summary_source_event_end(
            context.events,
            turn_start,
            self.policy.summary_source_token_limit(),
            &self.token_counter,
        )?;
        let compactable_messages =
            transcript_messages_from_events(&context.events[turn_start..summary_end]);
        if is_prompt_only_same_turn_range(&compactable_messages)
            && !context.diagnostics.exceeds_usable_input_budget()
        {
            return None;
        }

        self.summary_request_for_range(SummaryRange {
            events: context.events,
            source_event_start: turn_start,
            source_event_end: summary_end,
            latest_summary: context.latest_summary,
            diagnostics: context.diagnostics,
            allow_empty_with_previous_summary: false,
        })
    }

    /// Builds a summary request for an already-selected source event range.
    fn summary_request_for_range(&self, range: SummaryRange<'_>) -> Option<ContextSummaryRequest> {
        if range.source_event_end <= range.source_event_start {
            return None;
        }

        let compactable_events = &range.events[range.source_event_start..range.source_event_end];
        let compactable_messages = transcript_messages_from_events(compactable_events);
        if compactable_messages.is_empty()
            && (!range.allow_empty_with_previous_summary || range.latest_summary.is_none())
        {
            return None;
        }

        let transcript = format_messages_for_summary(&compactable_messages);
        let estimated_tokens = compactable_messages
            .iter()
            .map(|message| self.token_counter.count_message_tokens(message))
            .sum::<usize>();
        let summary_source_event_start = range
            .latest_summary
            .as_ref()
            .map(|summary| summary.source_event_start)
            .unwrap_or(range.source_event_start);

        Some(ContextSummaryRequest {
            previous_summary: range.latest_summary,
            source_event_start: summary_source_event_start,
            source_event_end: range.source_event_end,
            transcript,
            estimated_tokens,
            diagnostics: range.diagnostics,
        })
    }

    /// Fails when the final assembled context exceeds the usable model input budget.
    fn validate_token_budget(
        &self,
        diagnostics: &ContextDiagnostics,
    ) -> Result<(), ContextAssemblyError> {
        let Some(usable_input_tokens) = diagnostics.usable_input_tokens else {
            return Ok(());
        };
        if diagnostics.total_input_tokens <= usable_input_tokens {
            return Ok(());
        }

        Err(ContextAssemblyError::TokenBudgetExceeded {
            estimated_input_tokens: diagnostics.total_input_tokens,
            usable_input_tokens,
        })
    }

    /// Counts token estimates for a message slice.
    fn count_messages(&self, messages: &[ProviderMessage]) -> usize {
        messages
            .iter()
            .map(|message| self.token_counter.count_message_tokens(message))
            .sum()
    }

    fn section_tokens(
        &self,
        messages: &[ProviderMessage],
        counts: &SectionCounts,
    ) -> SectionTokens {
        let summary_end = counts.system + counts.summary;
        let transcript_end = summary_end + counts.transcript;
        let system = self.count_messages(&messages[..counts.system]);
        let summary = self.count_messages(&messages[counts.system..summary_end]);
        let transcript = self.count_messages(&messages[summary_end..transcript_end]);
        let continuation = self.count_messages(&messages[transcript_end..]);

        SectionTokens {
            system,
            summary,
            transcript,
            continuation,
            total_input_tokens: system + summary + transcript + continuation,
        }
    }
}

/// Returns true when same-turn compaction would only summarize the active prompt.
fn is_prompt_only_same_turn_range(messages: &[ProviderMessage]) -> bool {
    matches!(
        messages,
        [ProviderMessage {
            role: ProviderMessageRole::User,
            ..
        }]
    )
}

impl ContextDiagnostics {
    /// Returns true when estimated input already exceeds the usable provider input budget.
    fn exceeds_usable_input_budget(&self) -> bool {
        self.usable_input_tokens
            .map(|usable_input_tokens| self.total_input_tokens > usable_input_tokens)
            .unwrap_or(false)
    }
}

struct ReplayContext {
    latest_summary: Option<ContextSummary>,
    messages: Vec<ProviderMessage>,
}

struct SummaryContext<'a> {
    events: &'a [AgentEvent],
    replay_start: usize,
    latest_summary: Option<ContextSummary>,
    diagnostics: ContextDiagnostics,
}

struct SummaryRange<'a> {
    events: &'a [AgentEvent],
    source_event_start: usize,
    source_event_end: usize,
    latest_summary: Option<ContextSummary>,
    diagnostics: ContextDiagnostics,
    allow_empty_with_previous_summary: bool,
}

struct SectionCounts {
    system: usize,
    summary: usize,
    transcript: usize,
    continuation: usize,
}

impl SectionCounts {
    fn from_messages(messages: &[ProviderMessage], has_summary: bool) -> Self {
        let system = 1;
        let summary = usize::from(has_summary);
        let continuation = usize::from(
            messages
                .last()
                .is_some_and(|message| message.role == ProviderMessageRole::User),
        );
        let transcript = messages
            .len()
            .saturating_sub(system + summary + continuation);

        Self {
            system,
            summary,
            transcript,
            continuation,
        }
    }
}

struct SectionTokens {
    system: usize,
    summary: usize,
    transcript: usize,
    continuation: usize,
    total_input_tokens: usize,
}

impl SectionTokens {
    fn section_usage(&self, counts: &SectionCounts) -> Vec<ContextSectionUsage> {
        section_usage(vec![
            (ContextSection::System, counts.system, self.system),
            (ContextSection::Summary, counts.summary, self.summary),
            (
                ContextSection::Transcript,
                counts.transcript,
                self.transcript,
            ),
            (
                ContextSection::Continuation,
                counts.continuation,
                self.continuation,
            ),
        ])
    }
}

fn replay_context(input: &ContextAssemblyInput<'_>) -> ReplayContext {
    let latest_summary = latest_context_summary(input.store.events()).cloned();
    let replay_start = latest_summary
        .as_ref()
        .map(|summary| summary.source_event_end)
        .unwrap_or_default();
    let replay_events = input.store.events().get(replay_start..).unwrap_or_default();
    let summary_message = latest_summary
        .as_ref()
        .map(|summary| ProviderMessage::system(format_summary_message(&summary.content)));
    let transcript_messages = transcript_messages_from_events(replay_events);
    let continuation_message = input.continuation_prompt.map(ProviderMessage::user);
    let messages = build_messages(
        input.system_prompt.clone(),
        summary_message,
        transcript_messages,
        continuation_message,
    );

    ReplayContext {
        latest_summary,
        messages,
    }
}

impl Display for ContextAssemblyError {
    /// Formats this value for user-facing display.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContextAssemblyError::ProviderLimit(error) => Display::fmt(error, formatter),
            ContextAssemblyError::TokenBudgetExceeded {
                estimated_input_tokens,
                usable_input_tokens,
            } => write!(
                formatter,
                "{estimated_input_tokens} estimated input tokens exceeds usable budget {usable_input_tokens}"
            ),
        }
    }
}

impl Error for ContextAssemblyError {
    /// Returns the underlying source error when one is available.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ContextAssemblyError::ProviderLimit(error) => Some(error),
            ContextAssemblyError::TokenBudgetExceeded { .. } => None,
        }
    }
}

/// Drops zero-sized sections from diagnostics output.
fn section_usage(sections: Vec<(ContextSection, usize, usize)>) -> Vec<ContextSectionUsage> {
    sections
        .into_iter()
        .filter(|(_, message_count, estimated_tokens)| *message_count > 0 || *estimated_tokens > 0)
        .map(
            |(section, message_count, estimated_tokens)| ContextSectionUsage {
                section,
                message_count,
                estimated_tokens,
            },
        )
        .collect()
}

/// Returns true when a configured threshold is exceeded.
fn threshold_exceeded(total_input_tokens: usize, threshold: Option<usize>) -> bool {
    threshold
        .map(|threshold| total_input_tokens > threshold)
        .unwrap_or(false)
}

/// Finds the latest stored context summary in the durable event stream.
fn latest_context_summary(events: &[AgentEvent]) -> Option<&ContextSummary> {
    events.iter().rev().find_map(|event| match event {
        AgentEvent::ContextSummaryCreated(summary) => Some(summary),
        _ => None,
    })
}
