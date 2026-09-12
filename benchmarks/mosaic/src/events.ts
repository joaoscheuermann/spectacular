import type { AgentEvent } from 'agent';
import type { MosaicEvent, MosaicResult } from 'mosaic';

import type { RunEvent } from './run.js';

/** Converts one public Agent event into the benchmark's provider-neutral event. */
export const directEvent = (event: AgentEvent): RunEvent | undefined => {
  switch (event.type) {
    case 'agent.started':
      return { type: 'status', status: 'running' };

    case 'agent.finished':
      return { type: 'status', status: 'completed' };

    case 'text.delta':

    case 'refusal.delta':
      return { type: 'message_delta', delta: event.delta };

    case 'tool.started':
      return {
        type: 'tool_started',
        callId: event.call.id,
        name: event.call.name,
        input: event.call.payload,
      };

    case 'tool.finished':
      return {
        type: 'tool_completed',
        callId: event.call.id,
        name: event.call.name,
        output: event.result,
      };

    case 'tool.failed':
      return {
        type: 'tool_failed',
        callId: event.call.id,
        name: event.call.name,
      };

    case 'error':
      return { type: 'status', status: 'failed' };

    default:
      return undefined;
  }
};

/** Converts safe public MOSAIC hooks without exposing provider reasoning. */
export const mosaicEvent = (event: MosaicEvent): RunEvent | undefined => {
  switch (event.type) {
    case 'tool.started':
      return {
        type: 'tool_started',
        callId: event.callId,
        name: event.toolName,
        ...(event.input === undefined ? {} : { input: event.input }),
      };

    case 'tool.finished':
      return {
        type: 'tool_completed',
        callId: event.callId,
        name: event.toolName,
        ...(event.output === undefined ? {} : { output: event.output }),
      };

    case 'tool.failed':
      return {
        type: 'tool_failed',
        callId: event.callId,
        name: event.toolName,
      };

    case 'model.response':
      return visibleText(event.content);

    case 'node.status':
      return { type: 'status', status: event.status };

    case 'run.started':
      return { type: 'status', status: 'running' };

    case 'run.finished':
      return { type: 'status', status: event.status };

    case 'run.failed':
      return { type: 'status', status: 'failed' };

    case 'stage.started':
      return { type: 'status', status: `${event.stage}:started` };

    case 'stage.finished':
      return { type: 'status', status: `${event.stage}:completed` };

    case 'stage.failed':
      return { type: 'status', status: `${event.stage}:failed` };

    default:
      return undefined;
  }
};

/** Converts the deterministic MOSAIC delivery into the final visible message. */
export const mosaicResult = (result: MosaicResult): RunEvent | undefined =>
  result.status === 'completed' && result.delivery.markdown.length > 0
    ? { type: 'message_delta', delta: result.delivery.markdown }
    : undefined;

const visibleText = (content: unknown): RunEvent | undefined => {
  if (typeof content !== 'object' || content === null || !('text' in content)) {
    return undefined;
  }

  const text = content.text;

  return typeof text === 'string' && text.length > 0
    ? { type: 'message_delta', delta: text }
    : undefined;
};
