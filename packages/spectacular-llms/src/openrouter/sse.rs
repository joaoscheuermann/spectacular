use crate::ProviderError;

#[derive(Default)]
pub(crate) struct OpenRouterSseParser {
    buffer: Vec<u8>,
}

impl OpenRouterSseParser {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, ProviderError> {
        self.buffer.extend_from_slice(bytes);
        let mut payloads = Vec::new();

        while let Some((event_end, boundary_len)) = openrouter_sse_event_boundary(&self.buffer) {
            let raw_event = self
                .buffer
                .drain(..event_end + boundary_len)
                .collect::<Vec<_>>();
            let event = std::str::from_utf8(&raw_event[..event_end]).map_err(|error| {
                ProviderError::ResponseParsingFailed {
                    provider_name: "OpenRouter".to_owned(),
                    reason: error.to_string(),
                }
            })?;

            if let Some(payload) = openrouter_sse_payload(event) {
                payloads.push(payload);
            }
        }

        Ok(payloads)
    }
}

fn openrouter_sse_event_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    [
        b"\r\n\r\n".as_slice(),
        b"\n\n".as_slice(),
        b"\r\r".as_slice(),
    ]
    .into_iter()
    .filter_map(|boundary| {
        bytes
            .windows(boundary.len())
            .position(|window| window == boundary)
            .map(|position| (position, boundary.len()))
    })
    .min_by_key(|(position, _)| *position)
}

fn openrouter_sse_payload(event: &str) -> Option<String> {
    let mut data_lines = Vec::new();

    for raw_line in event.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        let Some((field, value)) = line.split_once(':') else {
            continue;
        };
        if field == "data" {
            data_lines.push(value.strip_prefix(' ').unwrap_or(value).to_owned());
        }
    }

    if data_lines.is_empty() {
        None
    } else {
        Some(data_lines.join("\n"))
    }
}
