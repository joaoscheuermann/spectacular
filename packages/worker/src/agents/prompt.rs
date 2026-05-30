use std::fs;
use std::path::PathBuf;

use lifecycle::event::WorkerEvent;
use lifecycle::identity::RequestId;

use crate::error::{WorkerError, WorkerResult};
use crate::event::prompt_agent_event_to_worker_event;
use crate::provider::{agent_config_for_runtime, WorkerProvider, WorkerRuntimeSelection};
use crate::repo::WorkerLayout;
use crate::state::WorkerId;
use crate::tooling::worker_tool_storage;

/// Prompt-agent work item with prepared worker storage and optional answered input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptJob {
    worker_id: WorkerId,
    layout: WorkerLayout,
    prompt: String,
    answers: Vec<PromptAnswer>,
}

impl PromptJob {
    pub fn new(worker_id: WorkerId, layout: WorkerLayout, prompt: impl Into<String>) -> Self {
        Self {
            worker_id,
            layout,
            prompt: prompt.into(),
            answers: Vec::new(),
        }
    }

    pub fn worker_id(&self) -> &WorkerId {
        &self.worker_id
    }

    pub fn layout(&self) -> &WorkerLayout {
        &self.layout
    }

    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    pub fn answers(&self) -> &[PromptAnswer] {
        &self.answers
    }

    pub fn with_answer(mut self, request_id: RequestId, answer: impl Into<String>) -> Self {
        self.answers.push(PromptAnswer {
            request_id,
            answer: answer.into(),
        });
        self
    }
}

/// Human answer correlated to a lifecycle request id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptAnswer {
    request_id: RequestId,
    answer: String,
}

impl PromptAnswer {
    pub fn request_id(&self) -> &RequestId {
        &self.request_id
    }

    pub fn answer(&self) -> &str {
        &self.answer
    }
}

/// Fakeable prompt runner dependency for prompt/requirements execution.
pub trait PromptAgentRunner {
    fn run(&self, job: &PromptJob) -> WorkerResult<PromptAgentReport>;
}

/// Worker-local production prompt-agent composition.
///
/// This builds the concrete agent from worker-owned provider/runtime selection
/// and worker tool registration without depending on `cli` or running a prompt.
#[derive(Debug)]
pub struct WorkerPromptAgent {
    agent: agent::Agent<WorkerProvider>,
    config: agent::AgentConfig,
}

impl WorkerPromptAgent {
    pub fn agent(&self) -> &agent::Agent<WorkerProvider> {
        &self.agent
    }

    pub fn config(&self) -> &agent::AgentConfig {
        &self.config
    }

    pub fn into_agent(self) -> agent::Agent<WorkerProvider> {
        self.agent
    }
}

/// Builds a production prompt agent from worker-local provider and tooling pieces.
pub fn prompt_agent_for_worker(
    provider: WorkerProvider,
    runtime: &WorkerRuntimeSelection,
    layout: &WorkerLayout,
    store: agent::Store,
    system_prompt: &str,
) -> Result<WorkerPromptAgent, agent::ToolRegistrationError> {
    let config = agent_config_for_runtime(runtime, system_prompt);
    let tools = worker_tool_storage(layout)?;
    let agent =
        agent::Agent::with_config_and_store(provider, config.clone(), store).with_tools(tools);

    Ok(WorkerPromptAgent { agent, config })
}

/// Runs a prompt-agent job and maps runner events into worker lifecycle-style events.
pub struct PromptAgentOrchestrator<'a> {
    runner: &'a dyn PromptAgentRunner,
}

impl<'a> PromptAgentOrchestrator<'a> {
    pub fn new(runner: &'a dyn PromptAgentRunner) -> Self {
        Self { runner }
    }

    pub fn run(&self, job: PromptJob) -> WorkerResult<PromptAgentReport> {
        let report = self.runner.run(&job)?;
        let events = report
            .prompt_events
            .iter()
            .cloned()
            .enumerate()
            .map(|(sequence, event)| {
                prompt_agent_event_to_worker_event(job.worker_id.clone(), sequence as u64, event)
            })
            .collect();

        Ok(report.with_worker_events(events))
    }
}

/// Prompt-agent domain events before worker event mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptAgentEvent {
    Started {
        message: String,
    },
    ArtifactWritten {
        artifact: String,
        message: String,
    },
    InputRequested {
        request_id: RequestId,
        request_text: String,
    },
    AnswerConsumed {
        request_id: RequestId,
        message: String,
    },
    Completed {
        message: String,
    },
    Failed {
        message: String,
    },
}

impl PromptAgentEvent {
    pub fn started(message: impl Into<String>) -> Self {
        Self::Started {
            message: message.into(),
        }
    }

    pub fn artifact_written(artifact: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ArtifactWritten {
            artifact: artifact.into(),
            message: message.into(),
        }
    }

    pub fn input_requested(request_id: RequestId, request_text: impl Into<String>) -> Self {
        Self::InputRequested {
            request_id,
            request_text: request_text.into(),
        }
    }

    pub fn answer_consumed(request_id: RequestId, message: impl Into<String>) -> Self {
        Self::AnswerConsumed {
            request_id,
            message: message.into(),
        }
    }

    pub fn completed(message: impl Into<String>) -> Self {
        Self::Completed {
            message: message.into(),
        }
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }
}

/// Prompt-agent run output and mapped worker events.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptAgentReport {
    prompt_events: Vec<PromptAgentEvent>,
    events: Vec<WorkerEvent>,
    output: String,
}

impl PromptAgentReport {
    pub fn new(
        prompt_events: impl IntoIterator<Item = PromptAgentEvent>,
        output: impl Into<String>,
    ) -> Self {
        Self {
            prompt_events: prompt_events.into_iter().collect(),
            events: Vec::new(),
            output: output.into(),
        }
    }

    pub fn events(&self) -> &[WorkerEvent] {
        &self.events
    }

    pub fn prompt_events(&self) -> &[PromptAgentEvent] {
        &self.prompt_events
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    fn with_worker_events(mut self, events: Vec<WorkerEvent>) -> Self {
        self.events = events;
        self
    }
}

/// Writes the prompt artifact under the worker artifacts directory, replacing old content.
pub fn write_prompt_artifact(layout: &WorkerLayout, content: &str) -> WorkerResult<PathBuf> {
    let path = layout.artifacts().join("PROMPT.md");
    fs::create_dir_all(layout.artifacts())
        .and_then(|_| fs::write(&path, content))
        .map_err(|source| WorkerError::prompt_artifact_write_failed(path.clone(), source))?;
    Ok(path)
}
