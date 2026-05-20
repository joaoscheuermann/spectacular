use super::recorder::RunRecorder;
use crate::context::TokenCounter;
use crate::error::AgentError;
use spectacular_llms::{LlmProvider, ProviderError, ProviderRequest, ProviderStreamEvent};
use std::time::Duration;

/// Retry settings shared by visible completions and hidden summary completions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProviderRetryConfig {
    pub(super) max_provider_retries: usize,
    pub(super) provider_retry_delay: Duration,
}

/// Handles provider stream events and returns a caller-specific terminal output.
pub(super) trait ProviderStreamHandler<P, C>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    type Output;

    /// Processes one provider stream event and optionally returns a terminal output.
    async fn handle_event(
        &mut self,
        recorder: &mut RunRecorder<'_, P, C>,
        event: ProviderStreamEvent,
    ) -> Result<Option<Self::Output>, AgentError>;

    /// Produces an output when a provider stream ends without an explicit finish event.
    async fn stream_finished_without_event(
        &mut self,
        recorder: &mut RunRecorder<'_, P, C>,
        saw_provider_event: bool,
    ) -> Result<Self::Output, AgentError>;
}

/// Runs a provider request with shared retry, stream-error, and cancellation handling.
pub(super) async fn run_retryable_provider_stream<P, C, H>(
    provider: &P,
    request: ProviderRequest,
    recorder: &mut RunRecorder<'_, P, C>,
    retry_config: ProviderRetryConfig,
    handler: &mut H,
) -> Result<H::Output, AgentError>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
    H: ProviderStreamHandler<P, C>,
{
    let mut provider_retries = 0usize;
    'provider_attempt: loop {
        recorder.return_if_cancelled().await?;

        let mut stream = match provider
            .stream_completion(request.clone(), recorder.cancellation())
            .await
        {
            Ok(stream) => stream,
            Err(ProviderError::CancellationError) => {
                return cancel_from_provider(recorder, handler, false).await;
            }
            Err(error)
                if should_retry_provider_error(&error, provider_retries, false, retry_config) =>
            {
                provider_retries += 1;
                wait_before_provider_retry(recorder, retry_config.provider_retry_delay).await?;
                continue;
            }
            Err(error) => return Err(error.into()),
        };

        recorder.return_if_cancelled().await?;

        let mut saw_provider_event = false;
        while let Some(provider_event) = stream.next().await {
            return_if_cancelled_after_stream_event(recorder, handler, saw_provider_event).await?;

            let provider_event = match provider_event {
                Ok(provider_event) => provider_event,
                Err(ProviderError::CancellationError) => {
                    return cancel_from_provider(recorder, handler, saw_provider_event).await;
                }
                Err(error)
                    if should_retry_provider_error(
                        &error,
                        provider_retries,
                        saw_provider_event,
                        retry_config,
                    ) =>
                {
                    provider_retries += 1;
                    wait_before_provider_retry(recorder, retry_config.provider_retry_delay).await?;
                    continue 'provider_attempt;
                }
                Err(error) => {
                    let agent_error = error.into();
                    if saw_provider_event {
                        let _ = handler
                            .stream_finished_without_event(recorder, saw_provider_event)
                            .await;
                    }
                    return Err(agent_error);
                }
            };

            saw_provider_event = true;
            if let Some(output) = handler.handle_event(recorder, provider_event).await? {
                return Ok(output);
            }
        }

        return_if_cancelled_after_stream_event(recorder, handler, saw_provider_event).await?;
        return handler
            .stream_finished_without_event(recorder, saw_provider_event)
            .await;
    }
}

/// Closes any active lifecycle state before run cancellation records the terminal event.
async fn return_if_cancelled_after_stream_event<P, C, H>(
    recorder: &mut RunRecorder<'_, P, C>,
    handler: &mut H,
    saw_provider_event: bool,
) -> Result<(), AgentError>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
    H: ProviderStreamHandler<P, C>,
{
    if recorder.cancellation().is_cancelled() && saw_provider_event {
        let _ = handler
            .stream_finished_without_event(recorder, saw_provider_event)
            .await;
    }

    recorder.return_if_cancelled().await
}

/// Converts provider-side cancellation into an agent cancellation error and event.
async fn cancel_from_provider<P, C, H>(
    recorder: &mut RunRecorder<'_, P, C>,
    handler: &mut H,
    saw_provider_event: bool,
) -> Result<H::Output, AgentError>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
    H: ProviderStreamHandler<P, C>,
{
    if saw_provider_event {
        let _ = handler
            .stream_finished_without_event(recorder, saw_provider_event)
            .await;
    }
    recorder.cancel();
    recorder.return_if_cancelled().await?;
    Err(AgentError::CancellationError)
}

/// Waits between retry attempts while respecting run cancellation.
async fn wait_before_provider_retry<P, C>(
    recorder: &mut RunRecorder<'_, P, C>,
    delay: Duration,
) -> Result<(), AgentError>
where
    P: LlmProvider,
    C: TokenCounter + Clone,
{
    recorder.return_if_cancelled().await?;
    if !delay.is_zero() {
        tokio::time::sleep(delay).await;
    }
    recorder.return_if_cancelled().await
}

/// Returns true when a provider error can be retried before any stream output escapes.
fn should_retry_provider_error(
    error: &ProviderError,
    retries_used: usize,
    saw_provider_event: bool,
    retry_config: ProviderRetryConfig,
) -> bool {
    if saw_provider_event || retries_used >= retry_config.max_provider_retries {
        return false;
    }

    matches!(
        error,
        ProviderError::NetworkError { .. } | ProviderError::ProviderUnavailable { .. }
    )
}
