use super::*;
use spectacular_tui::runtime::merge_controller_state_and_view_update;

#[test]
fn merge_controller_state_and_view_update_when_tail_item_wraps_preserves_visible_anchor() {
    let mut local_state = state();
    reduce(
        &mut local_state,
        ChatTuiAction::Resize {
            width: 80,
            height: 8,
        },
    );
    spectacular_tui::apply_view_action_to_state(
        &mut local_state,
        spectacular_tui::ViewAction::Resize {
            width: 80,
            height: 8,
        },
    );
    for index in 0..8 {
        reduce(
            &mut local_state,
            ChatTuiAction::NoticeReported {
                message: format!("anchor line {index}"),
            },
        );
    }
    reduce(
        &mut local_state,
        ChatTuiAction::MessageStarted {
            id: TranscriptItemId::new("assistant-active"),
        },
    );
    reduce(
        &mut local_state,
        ChatTuiAction::MessageDelta {
            id: TranscriptItemId::new("assistant-active"),
            text: "tail".to_string(),
        },
    );
    spectacular_tui::apply_view_action_to_state(
        &mut local_state,
        spectacular_tui::ViewAction::ScrollTranscript(3),
    );
    let local_view = spectacular_tui::ViewState::from_state(&local_state);
    let before = render_state_to_string(&local_state, Some(80));

    let mut controller_state = local_state.clone();
    reduce(
        &mut controller_state,
        ChatTuiAction::MessageDelta {
            id: TranscriptItemId::new("assistant-active"),
            text: "\nstreamed row 1\nstreamed row 2\nstreamed row 3\nstreamed row 4".to_string(),
        },
    );

    let (merged_state, merged_view, prompt_reset) = merge_controller_state_and_view_update(
        &local_state,
        &local_view,
        controller_state,
        &local_state.session.prompt,
    );
    let after = render_state_to_string(
        &spectacular_tui::materialize_state(&merged_state, &merged_view),
        Some(80),
    );

    assert_eq!(prompt_reset, None);
    assert_eq!(
        visible_transcript_text(&before),
        visible_transcript_text(&after)
    );
    assert!(merged_view.scroll.offset > local_view.scroll.offset);
    assert!(!merged_view.scroll.follow_tail);
}

fn visible_transcript_text(rendered: &str) -> Vec<&str> {
    rendered
        .lines()
        .filter(|line| line.contains("anchor line"))
        .collect()
}

#[test]
fn merge_controller_state_and_view_update_when_controller_snapshot_has_stale_layout_metrics_preserves_local_viewport(
) {
    let mut local_state = state();
    reduce(
        &mut local_state,
        ChatTuiAction::Resize {
            width: 32,
            height: 12,
        },
    );
    for index in 0..10 {
        reduce(
            &mut local_state,
            ChatTuiAction::NoticeReported {
                message: format!(
                    "wrapped anchor line {index} with enough words to use several narrow rows"
                ),
            },
        );
    }
    let active_id = TranscriptItemId::new("assistant-active");
    reduce(
        &mut local_state,
        ChatTuiAction::MessageStarted {
            id: active_id.clone(),
        },
    );
    reduce(
        &mut local_state,
        ChatTuiAction::MessageDelta {
            id: active_id.clone(),
            text: "tail".to_string(),
        },
    );

    let local_width = 31;
    let mut local_view = spectacular_tui::ViewState::from_state(&local_state);
    local_view.scroll.visible_rows = 5;
    let row_starts =
        spectacular_tui::components::transcript_layout_row_starts(&local_state, local_width);
    let before_top = row_starts[3];
    local_view.scroll.offset = bottom_relative_offset(&local_state, local_width, 5, before_top);
    local_view.scroll.follow_tail = false;

    let mut controller_state = local_state.clone();
    controller_state.prompt_layout = spectacular_tui::PromptLayoutMetrics::default();
    reduce(
        &mut controller_state,
        ChatTuiAction::MessageDelta {
            id: active_id,
            text: "\nstreamed tail content that wraps differently at the local width".to_string(),
        },
    );

    let (merged_state, merged_view, prompt_reset) = merge_controller_state_and_view_update(
        &local_state,
        &local_view,
        controller_state,
        &local_state.session.prompt,
    );
    let after_top = scroll_top_row(&merged_state, &merged_view, local_width, 5);

    assert_eq!(prompt_reset, None);
    assert_eq!(merged_state.prompt_layout, local_state.prompt_layout);
    assert_eq!(after_top, before_top);
}

#[test]
fn merge_controller_state_and_view_update_when_streaming_tail_item_grows_preserves_top_row_inside_tail_item(
) {
    let mut local_state = state();
    reduce(
        &mut local_state,
        ChatTuiAction::Resize {
            width: 80,
            height: 12,
        },
    );
    let active_id = TranscriptItemId::new("assistant-active");
    reduce(
        &mut local_state,
        ChatTuiAction::MessageStarted {
            id: active_id.clone(),
        },
    );
    reduce(
        &mut local_state,
        ChatTuiAction::MessageDelta {
            id: active_id.clone(),
            text: (0..24)
                .map(|index| format!("streamed line {index}"))
                .collect::<Vec<_>>()
                .join("\n"),
        },
    );

    let local_width = 79;
    let mut local_view = spectacular_tui::ViewState::from_state(&local_state);
    local_view.scroll.visible_rows = 5;
    let before_top = 10;
    local_view.scroll.offset = bottom_relative_offset(&local_state, local_width, 5, before_top);
    local_view.scroll.follow_tail = false;

    let mut controller_state = local_state.clone();
    reduce(
        &mut controller_state,
        ChatTuiAction::MessageDelta {
            id: active_id,
            text: "\nnew streamed row 1\nnew streamed row 2".to_string(),
        },
    );

    let (merged_state, merged_view, prompt_reset) = merge_controller_state_and_view_update(
        &local_state,
        &local_view,
        controller_state,
        &local_state.session.prompt,
    );
    let after_top = scroll_top_row(&merged_state, &merged_view, local_width, 5);

    assert_eq!(prompt_reset, None);
    assert_eq!(after_top, before_top);
}

fn bottom_relative_offset(
    state: &spectacular_tui::State,
    width: usize,
    visible_rows: u32,
    top_row: usize,
) -> u32 {
    let total_rows = spectacular_tui::components::transcript_layout_total_rows(state, width);
    let max_offset = total_rows.saturating_sub(visible_rows as usize);
    u32::try_from(max_offset.saturating_sub(top_row)).unwrap_or(u32::MAX)
}

fn scroll_top_row(
    state: &spectacular_tui::State,
    view: &spectacular_tui::ViewState,
    width: usize,
    visible_rows: u32,
) -> usize {
    let total_rows = spectacular_tui::components::transcript_layout_total_rows(state, width);
    let max_offset = total_rows.saturating_sub(visible_rows as usize);
    max_offset.saturating_sub(view.scroll.offset as usize)
}

#[test]
fn apply_view_action_when_user_scrolls_during_edge_selection_stops_auto_scroll() {
    let mut state = state();
    populate_large_transcript(&mut state);
    state.scroll.offset = 10;
    state.scroll.follow_tail = false;
    state.app_selection.dragging = true;
    state.app_selection.viewport_edge = Some(spectacular_tui::ViewportEdge::AboveTranscript);

    spectacular_tui::apply_view_action_to_state(
        &mut state,
        spectacular_tui::ViewAction::ScrollTranscript(-3),
    );

    assert_eq!(state.scroll.offset, 7);
    assert!(!state.app_selection.dragging);
    assert_eq!(state.app_selection.viewport_edge, None);
}
