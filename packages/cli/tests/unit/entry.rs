use super::*;
use crate::cli_types::{
    Cli, Command, LifecycleAddressArgs, LifecycleAnswerArgs, LifecycleDaemonArgs,
    LifecycleDispatchArgs, LifecycleWorkerArgs,
};
use clap::Parser;
use std::cell::RefCell;
use std::rc::Rc;

#[tokio::test]
async fn dispatch_config_command_does_not_create_debug_logger() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let dependencies = DispatchDependencies::new(
        {
            let events = Rc::clone(&events);
            move || -> Result<(), std::io::Error> {
                events.borrow_mut().push("debug_logger");
                panic!("config dispatch must not create a debug logger")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ()| {
                events.borrow_mut().push("chat");
                panic!("config dispatch must not run chat")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ConfigArgs| {
                events.borrow_mut().push("config");
                Ok("config output".to_owned())
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: LifecycleDaemonArgs| {
                events.borrow_mut().push("daemon");
                panic!("config dispatch must not run daemon")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: Command| {
                events.borrow_mut().push("lifecycle");
                panic!("config dispatch must not run lifecycle")
            }
        },
    );
    let cli = Cli::try_parse_from(["doric", "config"]).unwrap();

    let output = dispatch_with_dependencies(cli, dependencies).await.unwrap();

    assert_eq!(output, Some("config output".to_owned()));
    assert_eq!(events.borrow().as_slice(), ["config"]);
}

#[tokio::test]
async fn dispatch_lifecycle_command_does_not_create_debug_logger() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let dependencies = DispatchDependencies::new(
        {
            let events = Rc::clone(&events);
            move || -> Result<(), std::io::Error> {
                events.borrow_mut().push("debug_logger");
                panic!("lifecycle dispatch must not create a debug logger")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ()| {
                events.borrow_mut().push("chat");
                panic!("lifecycle dispatch must not run chat")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ConfigArgs| {
                events.borrow_mut().push("config");
                panic!("lifecycle dispatch must not run config")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: LifecycleDaemonArgs| {
                events.borrow_mut().push("daemon");
                panic!("lifecycle dispatch must not run daemon")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: Command| {
                events.borrow_mut().push("lifecycle");
                Ok(Some("worker accepted".to_owned()))
            }
        },
    );
    let cli = Cli {
        command: Some(Command::Feature(LifecycleDispatchArgs {
            prompt: "write a prompt".to_owned(),
            repo: "C:/repo".to_owned(),
            addr: None,
        })),
    };

    let output = dispatch_with_dependencies(cli, dependencies).await.unwrap();

    assert_eq!(output, Some("worker accepted".to_owned()));
    assert_eq!(events.borrow().as_slice(), ["lifecycle"]);
}

#[tokio::test]
async fn dispatch_daemon_command_runs_daemon_runner_without_chat_or_lifecycle_client() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let dependencies = DispatchDependencies::new(
        {
            let events = Rc::clone(&events);
            move || -> Result<(), std::io::Error> {
                events.borrow_mut().push("debug_logger");
                panic!("daemon dispatch must not create a debug logger")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ()| {
                events.borrow_mut().push("chat");
                panic!("daemon dispatch must not run chat")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ConfigArgs| {
                events.borrow_mut().push("config");
                panic!("daemon dispatch must not run config")
            }
        },
        {
            let events = Rc::clone(&events);
            move |args: LifecycleDaemonArgs| {
                events.borrow_mut().push("daemon");
                assert_eq!(args.addr.as_deref(), Some("127.0.0.1:47822"));
                assert_eq!(args.worker_root.as_deref(), Some("C:/workers"));
                Ok("daemon started".to_owned())
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: Command| {
                events.borrow_mut().push("lifecycle");
                panic!("daemon dispatch must use a daemon runner seam")
            }
        },
    );
    let cli = Cli {
        command: Some(Command::Daemon(LifecycleDaemonArgs {
            addr: Some("127.0.0.1:47822".to_owned()),
            worker_root: Some("C:/workers".to_owned()),
        })),
    };

    let output = dispatch_with_dependencies(cli, dependencies).await.unwrap();

    assert_eq!(output, Some("daemon started".to_owned()));
    assert_eq!(events.borrow().as_slice(), ["daemon"]);
}

#[tokio::test]
async fn dispatch_lifecycle_unavailable_returns_error_without_direct_worker_fallback() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let dependencies = DispatchDependencies::new(
        {
            let events = Rc::clone(&events);
            move || -> Result<(), std::io::Error> {
                events.borrow_mut().push("debug_logger");
                panic!("lifecycle dispatch must not create a debug logger")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ()| {
                events.borrow_mut().push("chat");
                panic!("lifecycle dispatch must not run chat")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: ConfigArgs| {
                events.borrow_mut().push("config");
                panic!("lifecycle dispatch must not run config")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: LifecycleDaemonArgs| {
                events.borrow_mut().push("daemon");
                panic!("lifecycle dispatch must not run daemon")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: Command| {
                events.borrow_mut().push("lifecycle_client");
                Err(AppError::InvalidLifecycleCommand(
                    "daemon unavailable: failed to connect to lifecycle daemon".to_owned(),
                ))
            }
        },
    );
    let cli = Cli {
        command: Some(Command::Feature(LifecycleDispatchArgs {
            prompt: "write requirements".to_owned(),
            repo: "C:/repo".to_owned(),
            addr: None,
        })),
    };

    let error = dispatch_with_dependencies(cli, dependencies)
        .await
        .expect_err("unavailable daemon should return a lifecycle error");

    assert_eq!(
        crate::output::user_facing_error(&error),
        "daemon unavailable: failed to connect to lifecycle daemon"
    );
    assert_eq!(events.borrow().as_slice(), ["lifecycle_client"]);
}

#[tokio::test]
async fn dispatch_lifecycle_commands_bypass_chat_debug_log_startup() {
    for command in lifecycle_commands() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let dependencies = DispatchDependencies::new(
            {
                let events = Rc::clone(&events);
                move || -> Result<(), std::io::Error> {
                    events.borrow_mut().push("debug_logger");
                    panic!("lifecycle dispatch must not create a debug logger")
                }
            },
            {
                let events = Rc::clone(&events);
                move |_: ()| {
                    events.borrow_mut().push("chat");
                    panic!("lifecycle dispatch must not run chat")
                }
            },
            {
                let events = Rc::clone(&events);
                move |_: ConfigArgs| {
                    events.borrow_mut().push("config");
                    panic!("lifecycle dispatch must not run config")
                }
            },
            {
                let events = Rc::clone(&events);
                move |_: LifecycleDaemonArgs| {
                    events.borrow_mut().push("daemon");
                    panic!("lifecycle command must not run daemon")
                }
            },
            {
                let events = Rc::clone(&events);
                move |_: Command| {
                    events.borrow_mut().push("lifecycle");
                    Ok(Some("lifecycle output".to_owned()))
                }
            },
        );
        let cli = Cli {
            command: Some(command),
        };

        let output = dispatch_with_dependencies(cli, dependencies).await.unwrap();

        assert_eq!(output, Some("lifecycle output".to_owned()));
        assert_eq!(events.borrow().as_slice(), ["lifecycle"]);
    }
}

#[tokio::test]
async fn dispatch_bare_invocation_creates_debug_logger_before_chat_run() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let dependencies = DispatchDependencies::new(
        {
            let events = Rc::clone(&events);
            move || {
                events.borrow_mut().push("debug_logger");
                Ok(TestDebugLogger)
            }
        },
        {
            let events = Rc::clone(&events);
            move |_| {
                events.borrow_mut().push("chat");
                Ok("session-123".to_owned())
            }
        },
        {
            let events = Rc::clone(&events);
            move |_| {
                events.borrow_mut().push("config");
                panic!("bare dispatch must not run config")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_: LifecycleDaemonArgs| {
                events.borrow_mut().push("daemon");
                panic!("bare dispatch must not run daemon")
            }
        },
        {
            let events = Rc::clone(&events);
            move |_| {
                events.borrow_mut().push("lifecycle");
                panic!("bare dispatch must not run lifecycle")
            }
        },
    );
    let cli = Cli::try_parse_from(["doric"]).unwrap();

    let output = dispatch_with_dependencies(cli, dependencies).await.unwrap();

    assert_eq!(output, Some("Closed session: session-123".to_owned()));
    assert_eq!(events.borrow().as_slice(), ["debug_logger", "chat"]);
}

struct TestDebugLogger;

fn lifecycle_commands() -> Vec<Command> {
    vec![
        Command::Feature(LifecycleDispatchArgs {
            prompt: "write requirements".to_owned(),
            repo: "C:/repo".to_owned(),
            addr: None,
        }),
        Command::Debug(LifecycleDispatchArgs {
            prompt: "investigate failure".to_owned(),
            repo: "C:/repo".to_owned(),
            addr: None,
        }),
        Command::List(LifecycleAddressArgs { addr: None }),
        Command::Worker(LifecycleWorkerArgs {
            id: "worker-123".to_owned(),
            addr: None,
        }),
        Command::Answer(LifecycleAnswerArgs {
            worker_id: "worker-123".to_owned(),
            request_id: "request-1".to_owned(),
            text: "continue".to_owned(),
            addr: None,
        }),
    ]
}
