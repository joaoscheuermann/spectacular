import type { ToolCallRecord } from 'agent';

import type { Observation } from '../../schemas/observation.js';

/** Copies the agent-owned tool ledger into one graph node's observation ledger. */
export const materializeObservations = (
  goalId: string,
  records: readonly ToolCallRecord[],
): Observation[] =>
  records.map((record) => ({
    id: record.id,
    goalId,
    toolName: record.toolName,
    callId: record.callId,
    input: record.input,
    output: record.output,
  }));
