#[test]
fn handle_lifecycle_answer_success_renders_acceptance_and_continuation() {
    let secret = "sk-test_secret_1234567890";
    let mut client = RecordingLifecycleClient::new().with_answer(Ok(LifecycleAnswerResponse {
        worker_id: "worker-123".to_owned(),
        request_id: "request-1".to_owned(),
        accepted: true,
    }));

    let output = lifecycle_output(
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: secret.to_owned(),
            addr: None,
        }),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[ClientCall::Answer {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: secret.to_owned(),
            addr: None,
        }]
    );
    assert_contains(&output, "worker-123");
    assert_contains(&output, "request-1");
    assert_contains(&output, "answer accepted");
    assert_not_contains(&output, secret);
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_worker_unknown_worker_returns_clear_error() {
    let mut client = RecordingLifecycleClient::new().with_stream(Err(
        LifecycleError::UnknownWorker("unknown-worker".to_owned()),
    ));

    let error = lifecycle_error(Command::Worker(worker_args("unknown-worker")), &mut client);

    assert_eq!(
        client.calls(),
        &[ClientCall::Stream {
            worker_id: "unknown-worker".to_owned(),
            addr: None,
        }]
    );
    assert_contains(&error, "unknown worker");
    assert_contains(&error, "unknown-worker");
}

#[test]
fn handle_lifecycle_answer_unknown_request_returns_clear_error() {
    let mut client =
        RecordingLifecycleClient::new().with_answer(Err(LifecycleError::UnknownRequest {
            worker_id: "worker-123".to_owned(),
            request_id: "missing-request".to_owned(),
        }));

    let error = lifecycle_error(
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "missing-request".to_owned(),
            text: "continue".to_owned(),
            addr: None,
        }),
        &mut client,
    );

    assert_contains(&error, "unknown request");
    assert_contains(&error, "worker-123");
    assert_contains(&error, "missing-request");
}

#[test]
fn handle_lifecycle_commands_daemon_unavailable_return_nonzero_without_direct_worker_fallback() {
    for command in [
        Command::Feature(dispatch_args(
            "write requirements",
            "https://github.com/org/repo.git",
        )),
        Command::Debug(dispatch_args(
            "investigate failure",
            "https://github.com/org/repo.git",
        )),
        Command::List(LifecycleAddressArgs { addr: None }),
        Command::Worker(worker_args("worker-123")),
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: "continue".to_owned(),
            addr: None,
        }),
    ] {
        let mut client = RecordingLifecycleClient::unavailable();

        let error = lifecycle_error(command, &mut client);

        assert_contains(&error, "daemon unavailable");
        assert_not_contains(&error, "direct worker fallback");
        assert_eq!(
            client.calls().len(),
            1,
            "client calls: {:?}",
            client.calls()
        );
    }
}

#[test]
fn handle_lifecycle_output_redacts_repo_credentials_everywhere() {
    let raw_repo = "https://user:pass@github.com/org/repo.git";
    let token_repo = "https://ghp_secret@github.com/org/repo.git";
    let unsafe_status = "queued\ninjected";
    let mut dispatch_client = RecordingLifecycleClient::new().with_dispatch(Ok(dispatch_response(
        "feature-123",
        LifecycleDispatchMode::Feature,
        "https://github.com/org/repo.git",
        "accepted",
    )));
    let mut list_client = RecordingLifecycleClient::new().with_list(Ok(LifecycleListResponse {
        workers: vec![worker_summary(
            "feature-123",
            LifecycleDispatchMode::Feature,
            token_repo,
            unsafe_status,
            0,
            "queued",
        )],
    }));
    let mut stream_client = RecordingLifecycleClient::new().with_stream(Ok(vec![stream_event(
        2,
        "repo_preparation",
        "running",
        token_repo,
    )]));

    let dispatch = lifecycle_output(
        Command::Feature(dispatch_args("write requirements", raw_repo)),
        &mut dispatch_client,
    );
    let list = lifecycle_output(
        Command::List(LifecycleAddressArgs { addr: None }),
        &mut list_client,
    );
    let stream = lifecycle_output(
        Command::Worker(worker_args("worker-123")),
        &mut stream_client,
    );
    let combined = format!("{dispatch}\n{list}\n{stream}");

    assert_not_contains(&combined, "user:pass");
    assert_not_contains(&combined, "ghp_secret");
    assert_not_contains(&combined, raw_repo);
    assert_not_contains(&combined, token_repo);
    assert_not_contains(&combined, unsafe_status);
    assert_not_contains(&combined, "\ninjected");
    assert_contains(&combined, "queued injected");
    assert_contains(&combined, "github.com/org/repo.git");
}

#[test]
fn handle_lifecycle_output_redacts_provider_and_env_secret_text() {
    let secret = "sk-test_secret_1234567890abcdef";
    let mut client = RecordingLifecycleClient::new().with_answer(Err(LifecycleError::Daemon(
        format!("OPENAI_API_KEY={secret}"),
    )));

    let error = lifecycle_error(
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: format!("OPENAI_API_KEY={secret}"),
            addr: None,
        }),
        &mut client,
    );

    assert_not_contains(&error, secret);
    assert_not_contains(&error, &format!("OPENAI_API_KEY={secret}"));
    assert_contains(&error, "[REDACTED]");
}
