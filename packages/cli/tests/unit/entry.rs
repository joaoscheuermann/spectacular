use super::*;
use crate::cli_types::{Cli, Command, LifecycleDispatchArgs};
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
