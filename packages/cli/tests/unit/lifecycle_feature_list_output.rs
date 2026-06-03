#[test]
fn handle_lifecycle_dispatch_feature_success_renders_lifecycle_sequence_only() {
    let mut client = RecordingLifecycleClient::new().with_dispatch(Ok(dispatch_response(
        "feature-123",
        LifecycleDispatchMode::Feature,
        "https://github.com/org/repo.git",
        "accepted",
    )));

    let output = lifecycle_output(
        Command::Feature(dispatch_args(
            "write requirements",
            "https://github.com/org/repo.git",
        )),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[
            ClientCall::Connect { addr: None },
            ClientCall::Dispatch {
                mode: LifecycleDispatchMode::Feature,
                prompt: "write requirements".to_owned(),
                repo: "https://github.com/org/repo.git".to_owned(),
                addr: None,
            },
        ]
    );
    assert_order(
        &output,
        &[
            "connecting to daemon",
            "connected to daemon",
            "creating worker",
            "created worker: feature-123",
        ],
    );
    for unexpected in [
        "[accepted] Lifecycle worker",
        "Mode:",
        "Repo:",
        "Status:",
        "Scope:",
        "prompt/requirements",
    ] {
        assert_not_contains(&output, unexpected);
    }
    assert_not_contains(&output, "daemon client not wired");
    assert_not_contains(&output, "PRD");
    assert_not_contains(&output, "technical design");
    assert_not_contains(&output, "implementation");
    assert_not_contains(&output, "handover");
}

#[test]
fn handle_lifecycle_dispatch_debug_success_renders_mode_label_and_shared_v1_wording() {
    let mut client = RecordingLifecycleClient::new().with_dispatch(Ok(dispatch_response(
        "debug-456",
        LifecycleDispatchMode::Debug,
        "https://github.com/org/repo.git",
        "accepted",
    )));

    let output = lifecycle_output(
        Command::Debug(dispatch_args(
            "investigate failure",
            "https://github.com/org/repo.git",
        )),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[ClientCall::Dispatch {
            mode: LifecycleDispatchMode::Debug,
            prompt: "investigate failure".to_owned(),
            repo: "https://github.com/org/repo.git".to_owned(),
            addr: None,
        }]
    );
    assert_contains(&output, "debug-456");
    assert_contains(&output, "Mode: debug");
    assert_contains(&output, "prompt/requirements");
    assert_not_contains(&output, "distinct debug workflow");
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_dispatch_blank_prompt_rejects_without_client_call() {
    let mut client = RecordingLifecycleClient::new();

    let error = lifecycle_error(
        Command::Feature(dispatch_args("   ", "https://github.com/org/repo.git")),
        &mut client,
    );

    assert!(
        client.calls().is_empty(),
        "client calls: {:?}",
        client.calls()
    );
    assert_contains(&error, "prompt");
    assert_contains(&error, "must not be empty");
}

#[test]
fn handle_lifecycle_dispatch_blank_repo_rejects_without_client_call() {
    let mut client = RecordingLifecycleClient::new();

    let error = lifecycle_error(
        Command::Feature(dispatch_args("write requirements", "   ")),
        &mut client,
    );

    assert!(
        client.calls().is_empty(),
        "client calls: {:?}",
        client.calls()
    );
    assert_contains(&error, "repo");
    assert_contains(&error, "must not be empty");
}

#[test]
fn handle_lifecycle_dispatch_feature_invalid_repo_rejects_before_connection() {
    for raw_repo in [
        r"C:\Users\jvito\repo",
        "/tmp/repo",
        "../repo",
        "//server/share/repo",
        "file:///tmp/repo",
    ] {
        let mut client = RecordingLifecycleClient::new();

        let error = lifecycle_error(
            Command::Feature(dispatch_args("write requirements", raw_repo)),
            &mut client,
        );

        assert!(
            client.calls().is_empty(),
            "client calls: {:?}",
            client.calls()
        );
        assert_contains(&error, "repo URL");
        assert_contains(&error, "remote URL");
        assert_not_contains(&error, raw_repo);
    }
}

#[test]
fn handle_lifecycle_list_empty_renders_connection_lines_and_headers() {
    let mut client = RecordingLifecycleClient::new().with_list(Ok(LifecycleListResponse {
        workers: Vec::new(),
    }));

    let output = lifecycle_output(
        Command::List(LifecycleAddressArgs { addr: None }),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[
            ClientCall::Connect { addr: None },
            ClientCall::List { addr: None }
        ]
    );
    assert_order(
        &output,
        &[
            "connecting to daemon",
            "connected to daemon",
            "status",
            "worker id",
            "git repo",
        ],
    );
    assert_not_contains(&output, "daemon client not wired");
}

#[test]
fn handle_lifecycle_list_rows_renders_table_in_daemon_order() {
    let mut client = RecordingLifecycleClient::new().with_list(Ok(LifecycleListResponse {
        workers: vec![
            worker_summary(
                "feature-123",
                LifecycleDispatchMode::Feature,
                "https://github.com/org/first.git",
                "accepted",
                0,
                "queued",
            ),
            {
                let mut summary = worker_summary(
                    "debug-456",
                    LifecycleDispatchMode::Debug,
                    "https://github.com/org/second.git",
                    "waiting",
                    7,
                    "waiting for input",
                );
                summary.pending_request_id = Some("request-1".to_owned());
                summary
            },
        ],
    }));

    let output = lifecycle_output(
        Command::List(LifecycleAddressArgs {
            addr: Some("127.0.0.1:47822".to_owned()),
        }),
        &mut client,
    );

    assert_eq!(
        client.calls(),
        &[
            ClientCall::Connect {
                addr: Some("127.0.0.1:47822".to_owned()),
            },
            ClientCall::List {
                addr: Some("127.0.0.1:47822".to_owned()),
            },
        ]
    );
    assert_order(
        &output,
        &[
            "connecting to daemon",
            "connected to daemon",
            "status",
            "worker id",
            "git repo",
            "accepted",
            "feature-123",
            "https://github.com/org/first.git",
            "waiting",
            "debug-456",
            "https://github.com/org/second.git",
        ],
    );
    for unexpected in ["Mode:", "Sequence:", "Activity:", "Request:"] {
        assert_not_contains(&output, unexpected);
    }
    assert_not_contains(&output, "daemon client not wired");
}
