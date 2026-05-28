#[test]
fn openai_auth_record_parses_id_token_claims() {
    let id_token = jwt_with_payload(
        r#"{
        "email": "user@example.com",
        "chatgpt_plan_type": "plus",
        "chatgpt_user_id": "user-1",
        "chatgpt_account_id": "account-1",
        "chatgpt_account_is_fedramp": true
    }"#,
    );
    let response = OpenAiTokenResponse {
        id_token,
        access_token: "access-token".to_owned(),
        refresh_token: "refresh-token".to_owned(),
    };

    let auth = OpenAiAuthRecord::from_token_response(response, 42).unwrap();

    assert_eq!(auth.email.as_deref(), Some("user@example.com"));
    assert_eq!(auth.plan_type.as_deref(), Some("plus"));
    assert_eq!(auth.user_id.as_deref(), Some("user-1"));
    assert_eq!(auth.account_id.as_deref(), Some("account-1"));
    assert!(auth.fedramp);
    assert_eq!(auth.last_refresh_epoch_seconds, 42);
}

#[test]
fn openai_auth_refreshes_after_interval() {
    let auth = OpenAiAuthRecord {
        id_token: jwt_with_payload("{}"),
        access_token: jwt_with_payload(r#"{"exp": 1000000}"#),
        refresh_token: "refresh-token".to_owned(),
        account_id: None,
        email: None,
        plan_type: None,
        user_id: None,
        fedramp: false,
        last_refresh_epoch_seconds: 0,
    };

    assert!(auth.should_refresh(8 * 24 * 60 * 60));
}

#[test]
fn openai_auth_refreshes_expired_jwt_access_token() {
    let auth = OpenAiAuthRecord {
        id_token: jwt_with_payload("{}"),
        access_token: jwt_with_payload(r#"{"exp": 100}"#),
        refresh_token: "refresh-token".to_owned(),
        account_id: None,
        email: None,
        plan_type: None,
        user_id: None,
        fedramp: false,
        last_refresh_epoch_seconds: 100,
    };

    assert!(auth.should_refresh(101));
}

#[test]
fn callback_request_requires_matching_state_and_code() {
    let request = "GET /auth/callback?code=abc123&state=expected HTTP/1.1\r\n\r\n";

    let code = parse_callback_request(request, "expected").unwrap();

    assert_eq!(code, "abc123");
}

#[test]
fn callback_request_rejects_state_mismatch() {
    let request = "GET /auth/callback?code=abc123&state=wrong HTTP/1.1\r\n\r\n";

    let error = parse_callback_request(request, "expected").unwrap_err();

    assert!(matches!(error, ProviderError::AuthenticationFailed { .. }));
    assert!(error.to_string().contains("state"));
}

fn jwt_with_payload(payload: &str) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode("{}"),
        URL_SAFE_NO_PAD.encode(payload),
        "signature"
    )
}
