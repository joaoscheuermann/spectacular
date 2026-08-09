import { fail } from './study-runtime.mjs';

/** Validates one explicit human approval against its immutable subject. */
export const validateHumanApproval = (context, value, expected) => {
  const keys = [
    'approvedAt',
    'approvedBy',
    'artifactHash',
    'checks',
    ...(expected.maximumCost ? ['maximumCostUsd'] : []),
    'rationale',
    'schemaVersion',
    'subject',
  ].sort();
  if (
    typeof value !== 'object' ||
    value === null ||
    Array.isArray(value) ||
    Object.keys(value).sort().join('\0') !== keys.join('\0') ||
    value.schemaVersion !== 1 ||
    value.subject !== expected.subject ||
    value.artifactHash !== context.api.core.artifactHash(expected.artifact) ||
    typeof value.approvedBy !== 'string' ||
    value.approvedBy.trim().length === 0 ||
    typeof value.rationale !== 'string' ||
    value.rationale.trim().length === 0 ||
    !Number.isFinite(new Date(value.approvedAt).valueOf()) ||
    !Array.isArray(value.checks) ||
    [...value.checks].sort().join('\0') !==
      [...expected.checks].sort().join('\0') ||
    new Set(value.checks).size !== value.checks.length ||
    (expected.maximumCost &&
      (typeof value.maximumCostUsd !== 'number' || value.maximumCostUsd <= 0))
  ) {
    fail(
      `${expected.subject} approval is incomplete or does not bind its artifact`,
    );
  }
};

/** Validates the approval that binds rationale and resources to power. */
export const validatePowerApproval = (context, powerConfig, approval) => {
  const keys = [
    'approvedAt',
    'approvedBy',
    'parameters',
    'rationale',
    'resourceEstimate',
    'schemaVersion',
  ];
  if (
    typeof approval !== 'object' ||
    approval === null ||
    Array.isArray(approval) ||
    Object.keys(approval).sort().join('\0') !== keys.sort().join('\0') ||
    approval.schemaVersion !== 1 ||
    typeof approval.approvedBy !== 'string' ||
    approval.approvedBy.trim().length === 0 ||
    typeof approval.rationale !== 'string' ||
    approval.rationale.trim().length === 0 ||
    !Number.isFinite(new Date(approval.approvedAt).valueOf()) ||
    typeof approval.parameters !== 'object' ||
    approval.parameters === null ||
    context.api.core.artifactHash(approval.parameters) !==
      context.api.core.artifactHash(powerConfig) ||
    typeof approval.resourceEstimate !== 'object' ||
    approval.resourceEstimate === null ||
    Object.keys(approval.resourceEstimate).sort().join('\0') !==
      ['cpuHours', 'diskGiB', 'memoryGiB'].sort().join('\0') ||
    !['cpuHours', 'memoryGiB', 'diskGiB'].every(
      (name) =>
        typeof approval.resourceEstimate[name] === 'number' &&
        approval.resourceEstimate[name] > 0,
    )
  ) {
    fail(
      'power approval must bind the exact config, rationale, reviewer, and resource estimate',
    );
  }
};
