use super::diagnostics::{ContextDiagnostics, ContextSection, ContextSectionUsage};
use super::policy::ContextPolicy;
use super::token_count::{ApproximateTokenCounter, TokenCounter};
use super::{transcript_messages_from_events, validate_context_limits, ContextLimitFailure};
use crate::event::{AgentEvent, ContextSummary};
use crate::store::Store;
use spectacular_llms::{ProviderContextLimits, ProviderMessage, ProviderMessageRole};
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
pub struct ContextAssembler<C = ApproximateTokenCounter> {
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

impl Default for ContextAssembler<ApproximateTokenCounter> {
    /// Creates an assembler using approximate token counts and default context policy.
    fn default() -> Self {
        Self::new(ApproximateTokenCounter, ContextPolicy::default())
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
            input.system_prompt,
            summary_message,
            transcript_messages,
            continuation_message,
        );
        let diagnostics = self.diagnostics(&messages, latest_summary.is_some());

        if diagnostics.compaction_would_trigger {
            if let Some(summary_request) =
                self.summary_request(input.store, latest_summary, diagnostics.clone())
            {
                return Ok(ContextAssembly::NeedsSummary(summary_request));
            }
        }

        validate_context_limits(&messages, input.provider_limits)
            .map_err(ContextAssemblyError::ProviderLimit)?;
        self.validate_token_budget(&diagnostics)?;

        Ok(ContextAssembly::Ready(AssembledProviderContext {
            messages,
            diagnostics,
        }))
    }

    /// Builds diagnostics by counting each provider-context section independently.
    fn diagnostics(&self, messages: &[ProviderMessage], has_summary: bool) -> ContextDiagnostics {
        let system_count = 1;
        let summary_count = usize::from(has_summary);
        let continuation_count = usize::from(
            messages
                .last()
                .is_some_and(|message| message.role == ProviderMessageRole::User),
        );
        let transcript_count = messages
            .len()
            .saturating_sub(system_count + summary_count + continuation_count);
        let system_tokens = self.count_messages(&messages[..system_count]);
        let summary_tokens =
            self.count_messages(&messages[system_count..system_count + summary_count]);
        let transcript_start = system_count + summary_count;
        let transcript_end = transcript_start + transcript_count;
        let transcript_tokens = self.count_messages(&messages[transcript_start..transcript_end]);
        let continuation_tokens = self.count_messages(&messages[transcript_end..]);
        let total_input_tokens =
            system_tokens + summary_tokens + transcript_tokens + continuation_tokens;
        let budget = self.policy.budget();
        let active_compaction_threshold = self.policy.active_compaction_threshold();
        let soft_compaction_threshold = self.policy.soft_compaction_threshold();

        ContextDiagnostics {
            total_input_tokens,
            usable_input_tokens: budget.map(|budget| budget.usable_input_tokens),
            active_compaction_threshold,
            soft_compaction_threshold,
            max_output_tokens: self.policy.max_output_tokens,
            reasoning_reserve_tokens: self.policy.reasoning_reserve_tokens,
            safety_margin_tokens: self.policy.safety_margin_tokens,
            message_count: messages.len(),
            section_usage: section_usage(vec![
                (ContextSection::System, system_count, system_tokens),
                (ContextSection::Summary, summary_count, summary_tokens),
                (
                    ContextSection::Transcript,
                    transcript_count,
                    transcript_tokens,
                ),
                (
                    ContextSection::Continuation,
                    continuation_count,
                    continuation_tokens,
                ),
            ]),
            soft_compaction_would_trigger: threshold_exceeded(
                total_input_tokens,
                soft_compaction_threshold,
            ),
            compaction_would_trigger: threshold_exceeded(
                total_input_tokens,
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
        if let Some(summary_request) = self.old_turn_summary_request(
            events,
            replay_start,
            latest_summary.clone(),
            diagnostics.clone(),
        ) {
            return Some(summary_request);
        }

        self.same_turn_summary_request(events, replay_start, latest_summary, diagnostics)
    }

    /// Creates a summary request for compactable transcript before protected turns.
    fn old_turn_summary_request(
        &self,
        events: &[AgentEvent],
        replay_start: usize,
        latest_summary: Option<ContextSummary>,
        diagnostics: ContextDiagnostics,
    ) -> Option<ContextSummaryRequest> {
        let protect_start = protected_event_start(
            events,
            replay_start,
            self.policy.latest_turns_to_protect.max(1),
        );
        if protect_start <= replay_start {
            return None;
        }

        let summary_end = summary_source_event_end(
            events,
            replay_start,
            protect_start,
            self.policy.summary_source_token_limit(),
            &self.token_counter,
        );
        self.summary_request_for_range(
            events,
            replay_start,
            summary_end,
            latest_summary,
            diagnostics,
            true,
        )
    }

    /// Creates a summary request for completed work inside the active protected turn.
    fn same_turn_summary_request(
        &self,
        events: &[AgentEvent],
        replay_start: usize,
        latest_summary: Option<ContextSummary>,
        diagnostics: ContextDiagnostics,
    ) -> Option<ContextSummaryRequest> {
        let turn_start = latest_user_prompt_start(events, replay_start).unwrap_or(replay_start);
        let summary_end = same_turn_summary_source_event_end(
            events,
            turn_start,
            self.policy.summary_source_token_limit(),
            &self.token_counter,
        )?;
        let compactable_messages =
            transcript_messages_from_events(&events[turn_start..summary_end]);
        if is_prompt_only_same_turn_range(&compactable_messages)
            && !diagnostics.exceeds_usable_input_budget()
        {
            return None;
        }

        self.summary_request_for_range(
            events,
            turn_start,
            summary_end,
            latest_summary,
            diagnostics,
            false,
        )
    }

    /// Builds a summary request for an already-selected source event range.
    fn summary_request_for_range(
        &self,
        events: &[AgentEvent],
        source_event_start: usize,
        source_event_end: usize,
        latest_summary: Option<ContextSummary>,
        diagnostics: ContextDiagnostics,
        allow_empty_with_previous_summary: bool,
    ) -> Option<ContextSummaryRequest> {
        if source_event_end <= source_event_start {
            return None;
        }

        let compactable_events = &events[source_event_start..source_event_end];
        let compactable_messages = transcript_messages_from_events(compactable_events);
        if compactable_messages.is_empty()
            && (!allow_empty_with_previous_summary || latest_summary.is_none())
        {
            return None;
        }

        let transcript = format_messages_for_summary(&compactable_messages);
        let estimated_tokens = compactable_messages
            .iter()
            .map(|message| self.token_counter.count_message_tokens(message))
            .sum::<usize>();
        let summary_source_event_start = latest_summary
            .as_ref()
            .map(|summary| summary.source_event_start)
            .unwrap_or(source_event_start);

        Some(ContextSummaryRequest {
            previous_summary: latest_summary,
            source_event_start: summary_source_event_start,
            source_event_end,
            transcript,
            estimated_tokens,
            diagnostics,
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
