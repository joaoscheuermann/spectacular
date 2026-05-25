use super::*;

#[test]
fn scroll_state_when_streaming_deltas_update_correct_active_item_in_large_transcript() {
    let mut state = state();
    populate_large_transcript(&mut state);
    state.scroll.offset = 10;
    state.scroll.follow_tail = false;
    let active_id = TranscriptItemId::new("assistant-active");
    state.session.transcript.push(TranscriptItem::new(
        active_id.clone(),
        spectacular_tui::Timestamp::new(5_001),
        TranscriptItemContent::AssistantMessage(AssistantMessageItem::new("")),
    ));

    reduce(
        &mut state,
        ChatTuiAction::MessageDelta {
            id: active_id,
            text: "only active assistant changed".to_string(),
        },
    );

    assert!(matches!(
        state.session.transcript.last().map(|item| &item.content),
        Some(TranscriptItemContent::AssistantMessage(item))
            if item.text == "only active assistant changed"
    ));
    assert!(matches!(
        &state.session.transcript[0].content,
        TranscriptItemContent::UserPrompt(item) if item.text == "large transcript item 0"
    ));
    assert_eq!(state.scroll.offset, 10);
    assert!(!state.scroll.follow_tail);
}

/// Verifies spinner ticks during large transcript streaming stay within the documented budget.
#[test]
fn scroll_state_when_spinner_ticks_during_large_streaming_keep_rendering_responsive() {
    let mut state = state();
    populate_large_transcript(&mut state);
    reduce(
        &mut state,
        ChatTuiAction::MessageStarted {
            id: TranscriptItemId::new("assistant-active"),
        },
    );

    let started = Instant::now();
    for tick in 0..60 {
        reduce(&mut state, ChatTuiAction::SpinnerTick);
        reduce(
            &mut state,
            ChatTuiAction::MessageDelta {
                id: TranscriptItemId::new("assistant-active"),
                text: format!(" chunk-{tick}"),
            },
        );
        let _ = render_state_to_string(&state, Some(120));
    }

    let elapsed = started.elapsed();
    assert!(elapsed < RENDER_BUDGET, "spinner redraws took {elapsed:?}");
}

/// Verifies streaming tail updates and rendered-selection drag stay responsive at 20k items.
#[test]
fn shell_apply_terminal_event_when_streaming_drag_interleaves_with_large_transcript_keeps_rendering_responsive(
) {
    let mut state = state();
    populate_large_transcript(&mut state);
    reduce(
        &mut state,
        ChatTuiAction::Resize {
            width: 120,
            height: 30,
        },
    );
    let active_id = TranscriptItemId::new("assistant-active");

    let selectable_row = SelectableProjection::for_state(&state)
        .rows()
        .iter()
        .find(|row| row.surface == SelectableSurface::Transcript && !row.text.is_empty())
        .expect("visible transcript row")
        .screen_row as u16;

    let (mut shell, _intents) = spectacular_tui::Shell::new(state);
    shell.apply_action(ChatTuiAction::MessageStarted {
        id: active_id.clone(),
    });
    shell.apply_terminal_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        0,
        selectable_row,
    ));

    let started = Instant::now();
    for tick in 0..120 {
        shell.apply_action(ChatTuiAction::MessageDelta {
            id: active_id.clone(),
            text: format!(" chunk-{tick}"),
        });
        shell.apply_terminal_event(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            u16::try_from(4 + (tick % 12)).unwrap(),
            selectable_row,
        ));
        let _ = render_state_to_string(shell.state(), Some(120));
    }
    shell.apply_terminal_event(mouse(
        MouseEventKind::Up(MouseButton::Left),
        16,
        selectable_row,
    ));

    let elapsed = started.elapsed();
    assert!(
        elapsed < STREAMING_DRAG_BUDGET,
        "streaming drag took {elapsed:?}"
    );
    assert!(spectacular_tui::selected_text(shell.state()).is_some());
}
