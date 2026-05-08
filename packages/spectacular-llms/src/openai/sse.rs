use crate::ProviderError;

#[derive(Default)]
pub(crate) struct OpenAiSseParser {
    buffer: Vec<u8>,
}

impl OpenAiSseParser {
    /// Adds bytes to the SSE buffer and returns complete data payloads.
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, ProviderError> {
        self.buffer.extend_from_slice(chunk);
        let mut payloads = Vec::new();

        while let Some((event_end, boundary_len)) = sse_event_boundary(&self.buffer) {
            let event = self.buffer.drain(..event_end).collect::<Vec<_>>();
            self.buffer.drain(..boundary_len);
            let event =
                String::from_utf8(event).map_err(|error| ProviderError::ResponseParsingFailed {
                    provider_name: "OpenAI".to_owned(),
                    reason: error.to_string(),
                })?;
            if let Some(payload) = sse_payload(&event) {
                payloads.push(payload);
            }
        }

        Ok(payloads)
    }
}

/// Finds the next SSE event boundary in a byte buffer.
fn sse_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| (index, 4))
        .or_else(|| {
            bytes
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| (index, 2))
        })
}

/// Extracts joined data lines from a single SSE event.
fn sse_payload(event: &str) -> Option<String> {
    let payload = event
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");

    (!payload.is_empty()).then_some(payload)
}
