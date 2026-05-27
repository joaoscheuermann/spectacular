use super::*;

#[test]
fn render_app_when_footer_includes_session_first_without_usage() {
    let state = state();
    let footer = footer_render_line(&state);

    assert_eq!(
        footer.plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high)"
    );
    assert!(footer
        .spans
        .iter()
        .all(|span| span.style == RenderStyle::Dim));
}

#[test]
fn render_app_when_footer_metadata_is_unavailable_omits_worktree_metadata() {
    let state = State::new(SessionId::new("session-123"), runtime(), display(None));
    let footer = footer_render_line(&state);

    assert_eq!(
        footer.plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · ~0/200k ctx"
    );
    assert!(footer
        .spans
        .iter()
        .all(|span| span.style == RenderStyle::Dim));
}

#[test]
fn render_app_when_footer_includes_clean_branch_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("main"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · main · ~0/200k ctx"
    );
}

#[test]
fn render_app_when_footer_includes_dirty_branch_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("main*"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · main* · ~0/200k ctx"
    );
}

#[test]
fn render_app_when_footer_includes_detached_head_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("abc1234"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · abc1234 · ~0/200k ctx"
    );
}

#[test]
fn render_app_when_footer_includes_dirty_detached_head_worktree_metadata() {
    let mut display = display(None);
    display.worktree = Some(WorktreeMetadata::new("abc1234*"));
    let state = State::new(SessionId::new("session-123"), runtime(), display);

    assert_eq!(
        footer_render_line(&state).plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · abc1234* · ~0/200k ctx"
    );
}

#[test]
fn render_app_when_footer_matches_original_shape_with_usage() {
    let context_usage = ContextTokenUsage::new(80_000, Some(200_000));
    let mut state = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(context_usage)),
    );
    state.session.context_usage = Some(context_usage);
    state.session.turn_usage = Some(TurnTokenUsage {
        input_tokens: 3_000,
        output_tokens: 9_000,
        total_tokens: 12_000,
        has_provider_metadata: true,
    });
    state.session.total_usage = Some(TurnTokenUsage {
        input_tokens: 30_000,
        output_tokens: 12_000,
        total_tokens: 42_000,
        has_provider_metadata: true,
    });

    let footer = footer_render_line(&state);

    assert_eq!(
        footer.plain_text(),
        "session-123 · /workspace/spectacular · GPT 5.1 (high) · 42k/200k tks"
    );
    assert_eq!(footer.spans.last().unwrap().style, RenderStyle::Dim);
}

#[test]
fn render_app_when_footer_usage_uses_warning_and_critical_styles() {
    let warning = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(ContextTokenUsage::new(160, Some(200)))),
    );
    let critical = State::new(
        SessionId::new("session-123"),
        runtime(),
        display(Some(ContextTokenUsage::new(180, Some(200)))),
    );

    assert_eq!(
        footer_right_render_line(&warning).unwrap().spans[0].style,
        RenderStyle::Warning
    );
    assert_eq!(
        footer_right_render_line(&critical).unwrap().spans[0].style,
        RenderStyle::Error
    );
}
