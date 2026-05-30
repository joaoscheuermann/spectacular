use super::{
    cli_types::{Command, LifecycleAddressArgs, LifecycleAnswerArgs, LifecycleDispatchArgs},
    cli_types::{LifecycleDaemonArgs, LifecycleWorkerArgs},
    output::{
        format_lifecycle_answer_output, format_lifecycle_daemon_output,
        format_lifecycle_dispatch_output, format_lifecycle_list_output,
        format_lifecycle_worker_output,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LifecycleDispatchMode {
    Feature,
    Debug,
}

pub(super) fn handle_lifecycle_command(command: Command) -> Option<String> {
    match command {
        Command::Daemon(args) => Some(handle_daemon(args)),
        Command::Feature(args) => Some(handle_dispatch(LifecycleDispatchMode::Feature, args)),
        Command::Debug(args) => Some(handle_dispatch(LifecycleDispatchMode::Debug, args)),
        Command::List(args) => Some(handle_list(args)),
        Command::Worker(args) => Some(handle_worker(args)),
        Command::Answer(args) => Some(handle_answer(args)),
        Command::Config(_) => None,
    }
}

fn handle_daemon(args: LifecycleDaemonArgs) -> String {
    format_lifecycle_daemon_output(args.addr.as_deref(), args.worker_root.as_deref())
}

fn handle_dispatch(mode: LifecycleDispatchMode, args: LifecycleDispatchArgs) -> String {
    format_lifecycle_dispatch_output(mode, &args.prompt, &args.repo, args.addr.as_deref())
}

fn handle_list(args: LifecycleAddressArgs) -> String {
    format_lifecycle_list_output(args.addr.as_deref())
}

fn handle_worker(args: LifecycleWorkerArgs) -> String {
    format_lifecycle_worker_output(&args.id, args.addr.as_deref())
}

fn handle_answer(args: LifecycleAnswerArgs) -> String {
    format_lifecycle_answer_output(
        &args.worker_id,
        &args.request_id,
        &args.text,
        args.addr.as_deref(),
    )
}
