use crate::context::{transcript_messages_from_events, TokenCounter};
use crate::event::AgentEvent;

/// Finds the event index where the protected recent user turns begin.
pub(super) fn protected_event_start(
    events: &[AgentEvent],
    replay_start: usize,
    protected_user_turns: usize,
) -> usize {
    let mut seen_user_turns = 0usize;
    for index in (replay_start..events.len()).rev() {
        if !matches!(events[index], AgentEvent::UserPrompt { .. }) {
            continue;
        }

        seen_user_turns += 1;
        if seen_user_turns == protected_user_turns {
            return index;
        }
    }

    replay_start
}

/// Finds the most recent user prompt at or after the replay boundary.
pub(super) fn latest_user_prompt_start(
    events: &[AgentEvent],
    replay_start: usize,
) -> Option<usize> {
    (replay_start..events.len())
        .rev()
        .find(|index| matches!(events[*index], AgentEvent::UserPrompt { .. }))
}

/// Chooses a same-turn compactable prefix without splitting tool-call/result pairs.
pub(super) fn same_turn_summary_source_event_end<C>(
    events: &[AgentEvent],
    turn_start: usize,
    token_limit: Option<usize>,
    token_counter: &C,
) -> Option<usize>
where
    C: TokenCounter,
{
    let boundaries = same_turn_compaction_boundaries(events, turn_start);
    let max_end = same_turn_max_source_end(&boundaries)?;
    let candidates = same_turn_candidate_ends(&boundaries, max_end);
    budgeted_same_turn_source_end(events, turn_start, &candidates, token_limit, token_counter)
}

/// Finds safe same-turn summary boundaries and completed tool-result boundaries.
fn same_turn_compaction_boundaries(
    events: &[AgentEvent],
    turn_start: usize,
) -> SameTurnCompactionBoundaries {
    let mut safe_ends = Vec::new();
    let mut tool_result_ends = Vec::new();
    let mut pending_tool_calls: Vec<String> = Vec::new();
    let mut has_orphan_tool_result = false;

    for index in turn_start..events.len() {
        let mut completed_tool_result = false;
        match &events[index] {
            AgentEvent::ToolCallStart { tool_call_id, .. } => {
                pending_tool_calls.push(tool_call_id.clone());
            }
            AgentEvent::ToolCallFinish { tool_call_id, .. } => {
                if remove_pending_tool_call(&mut pending_tool_calls, tool_call_id) {
                    completed_tool_result = true;
                } else {
                    has_orphan_tool_result = true;
                }
            }
            _ => {}
        }

        if has_orphan_tool_result || !pending_tool_calls.is_empty() {
            continue;
        }

        let event_end = index + 1;
        safe_ends.push(event_end);
        if completed_tool_result {
            tool_result_ends.push(event_end);
        }
    }

    SameTurnCompactionBoundaries {
        safe_ends,
        tool_result_ends,
    }
}

#[derive(Debug, Eq, PartialEq)]
struct SameTurnCompactionBoundaries {
    safe_ends: Vec<usize>,
    tool_result_ends: Vec<usize>,
}

/// Removes one pending tool call id when its tool result arrives.
fn remove_pending_tool_call(pending_tool_calls: &mut Vec<String>, tool_call_id: &str) -> bool {
    let Some(position) = pending_tool_calls
        .iter()
        .position(|pending_tool_call_id| pending_tool_call_id == tool_call_id)
    else {
        return false;
    };

    pending_tool_calls.remove(position);
    true
}

/// Keeps the latest completed tool interaction raw when there is older same-turn history.
fn same_turn_max_source_end(boundaries: &SameTurnCompactionBoundaries) -> Option<usize> {
    if boundaries.tool_result_ends.len() >= 2 {
        return boundaries
            .tool_result_ends
            .get(boundaries.tool_result_ends.len() - 2)
            .copied();
    }

    boundaries.safe_ends.last().copied()
}

/// Uses completed tool-result boundaries when possible to avoid prompt-only summaries.
fn same_turn_candidate_ends(
    boundaries: &SameTurnCompactionBoundaries,
    max_end: usize,
) -> Vec<usize> {
    let tool_result_candidates = boundaries
        .tool_result_ends
        .iter()
        .copied()
        .filter(|event_end| *event_end <= max_end)
        .collect::<Vec<_>>();
    if !tool_result_candidates.is_empty() {
        return tool_result_candidates;
    }

    boundaries
        .safe_ends
        .iter()
        .copied()
        .filter(|event_end| *event_end <= max_end)
        .collect()
}

/// Applies the summary input token limit to same-turn boundary candidates.
fn budgeted_same_turn_source_end<C>(
    events: &[AgentEvent],
    turn_start: usize,
    candidates: &[usize],
    token_limit: Option<usize>,
    token_counter: &C,
) -> Option<usize>
where
    C: TokenCounter,
{
    let Some(token_limit) = token_limit else {
        return candidates.last().copied();
    };

    let mut selected = None;
    for candidate in candidates {
        let tokens = transcript_messages_from_events(&events[turn_start..*candidate])
            .iter()
            .map(|message| token_counter.count_message_tokens(message))
            .sum::<usize>();
        if selected.is_some() && tokens > token_limit {
            return selected;
        }

        selected = Some(*candidate);
        if tokens >= token_limit {
            return selected;
        }
    }

    selected
}

/// Chooses a compactable event prefix that should fit in one summary request.
pub(super) fn summary_source_event_end<C>(
    events: &[AgentEvent],
    replay_start: usize,
    protect_start: usize,
    token_limit: Option<usize>,
    token_counter: &C,
) -> usize
where
    C: TokenCounter,
{
    let Some(token_limit) = token_limit else {
        return protect_start;
    };
    if token_limit == 0 {
        return next_turn_end(events, replay_start, protect_start);
    }

    let mut event_start = replay_start;
    let mut event_end = replay_start;
    let mut used_tokens = 0usize;
    while event_start < protect_start {
        let turn_end = next_turn_end(events, event_start, protect_start);
        let turn_tokens = transcript_messages_from_events(&events[event_start..turn_end])
            .iter()
            .map(|message| token_counter.count_message_tokens(message))
            .sum::<usize>();
        if event_end > replay_start && used_tokens + turn_tokens > token_limit {
            return event_end;
        }

        used_tokens += turn_tokens;
        event_end = turn_end;
        event_start = turn_end;
        if used_tokens >= token_limit {
            return event_end;
        }
    }

    event_end
}

/// Returns the end event index for the current compactable user turn group.
fn next_turn_end(events: &[AgentEvent], start: usize, protect_start: usize) -> usize {
    for index in start.saturating_add(1)..protect_start {
        if matches!(events[index], AgentEvent::UserPrompt { .. }) {
            return index;
        }
    }

    protect_start
}
