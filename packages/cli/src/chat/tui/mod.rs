mod adapter;
mod controller;
mod display;
mod launch;
mod runner;
pub(crate) mod state;

pub(crate) use adapter::TuiEventAdapter;
pub(crate) use launch::run;

#[cfg(test)]
pub(crate) use controller::{Bootstrap as TuiBootstrap, Controller as TuiRuntimeController};
#[cfg(test)]
pub(crate) use launch::run_controller_loop;
#[cfg(test)]
pub(crate) use runner::{TurnFuture as TuiTurnFuture, TurnRunner as TuiTurnRunner};

#[cfg(test)]
mod tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/unit/chat/tui_runtime.rs"
    ));
}
