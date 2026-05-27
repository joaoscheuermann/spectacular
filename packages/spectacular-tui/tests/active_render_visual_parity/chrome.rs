use super::*;

#[test]
fn render_app_when_active_app_renders_terminal_flow_without_prototype_chrome() {
    let output = render(&state());

    assert!(output.contains("> "));
    assert!(output.contains("session-123 · /workspace/spectacular · GPT 5.1 (high)"));
    assert!(!output.contains("Transcript"));
    assert!(!output.contains("No transcript items yet"));
    assert!(!output.contains("Prompt:"));
    assert!(!output.contains("Completions:"));
    assert!(!output.contains("Guidance:"));
    assert!(!output.contains("Status: idle"));
    assert!(!output.contains("cwd:"));
    assert!(!output.contains("provider/model:"));
}

#[test]
fn render_app_when_opening_banner_matches_original_shape_width_and_styles() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::OpeningBanner(OpeningBannerItem::new(
            "0.1.0",
            "GPT 5.1",
            "high",
            "/workspace/spectacular",
            "session-123",
        )),
    ));

    let lines = app_render_lines(&state);
    let text = visible_text(&lines);
    let banner = &text[..7];

    assert_eq!(
        banner[0],
        "╭──────────────────────────────────────────────────────╮"
    );
    assert_eq!(
        banner[1],
        "│ Spectacular (v0.1.0)                                 │"
    );
    assert_eq!(
        banner[2],
        "│                                                      │"
    );
    assert_eq!(
        banner[3],
        "│ model:     GPT 5.1 high                              │"
    );
    assert_eq!(
        banner[4],
        "│ directory: /workspace/spectacular                    │"
    );
    assert_eq!(
        banner[5],
        "│ session:   session-123                               │"
    );
    assert_eq!(
        banner[6],
        "╰──────────────────────────────────────────────────────╯"
    );
    assert_eq!(lines[0].spans[0].style, RenderStyle::Title);
    assert_eq!(lines[1].spans[1].style, RenderStyle::Title);
    assert_eq!(lines[3].spans[1].style, RenderStyle::Text);
    assert_eq!(lines[4].spans[1].style, RenderStyle::Text);
    assert_eq!(lines[5].spans[1].style, RenderStyle::Text);
    assert_eq!(lines[6].spans[0].style, RenderStyle::Title);
}

#[test]
fn render_app_when_opening_banner_does_not_emit_mojibake() {
    let mut state = state();
    state.session.transcript.push(item(
        1,
        TranscriptItemContent::OpeningBanner(OpeningBannerItem::new(
            "0.1.0",
            "GPT 5.1",
            "high",
            "/workspace/spectacular",
            "session-123",
        )),
    ));

    let output = render(&state);

    assert!(output.contains('╭'));
    assert!(output.contains('─'));
    assert!(output.contains('│'));
    assert!(output.contains('╯'));
    assert!(!output.contains('�'));
}
