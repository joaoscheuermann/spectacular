#[test]
fn openai_response_urls_separate_api_key_and_oauth_backends() {
    assert_eq!(API_RESPONSES_URL, "https://api.openai.com/v1/responses");
    assert_eq!(
        CHATGPT_RESPONSES_URL,
        "https://chatgpt.com/backend-api/codex/responses"
    );
}
