import type { ProviderMessage, ProviderToolCall } from 'llms';

import type { Observation } from '../../types/revision.js';

/** Correlates executable calls and results into runtime-owned observations. */
export const materializeObservations = (
  goalId: string,
  messages: readonly ProviderMessage[],
): Observation[] => {
  // Calls are indexed by provider ID only inside this runtime correlation boundary.
  const calls = new Map<string, ProviderToolCall>();

  // A separate set makes duplicate tool returns distinguishable from duplicate calls.
  const results = new Set<string>();

  // Append order follows tool-result history, as required by section 4.9.
  const observations: Observation[] = [];

  for (const message of messages) {
    // Assistant messages register executable calls before their results arrive.
    if (message.role === 'assistant') {
      for (const call of message.toolCalls ?? []) {
        if (call.id.length === 0) {
          throw new Error(`Node ${goalId} has a tool call without an ID.`);
        }
        if (calls.has(call.id)) {
          throw new Error(`Node ${goalId} has a duplicate tool call ID.`);
        }
        calls.set(call.id, call);
      }
      continue;
    }

    // Model and system messages do not participate in the observation ledger.
    if (message.role !== 'tool') continue;

    // Every tool result must point to exactly one previously registered call.
    const callId = message.toolCallId;
    if (callId === undefined || callId.length === 0) {
      throw new Error(`Node ${goalId} has an uncorrelated tool result.`);
    }

    const call = calls.get(callId);
    if (call === undefined) {
      throw new Error(`Node ${goalId} has an uncorrelated tool result.`);
    }
    if (results.has(callId)) {
      throw new Error(`Node ${goalId} has a duplicate tool result.`);
    }

    // callId remains internal while name, input, and output become planner evidence.
    results.add(callId);
    observations.push({
      goalId,
      toolName: call.name,
      callId,
      input: call.arguments,
      output:
        typeof message.content === 'string'
          ? message.content
          : JSON.stringify(message.content ?? ''),
    });
  }

  // A call without a result makes the node history incomplete and unusable.
  if ([...calls.keys()].some((callId) => !results.has(callId))) {
    throw new Error(`Node ${goalId} has a tool call without a result.`);
  }

  return observations;
};
