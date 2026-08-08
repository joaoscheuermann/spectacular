import { HARNESS_COMMANDS } from '../core/commands.js';
import { artifactHash } from '../core/index.js';
import type { CliInvocation } from './args.js';
import { rejectUnknownFlags, requiredFlag } from './args.js';
import { analyze, power } from './analysis.js';
import { calibrate } from './calibration.js';
import { freeze } from './freeze.js';
import { objectValue, stringArray, stringValue } from './io.js';
import { review } from './review.js';
import { runProductionSchedule } from './run.js';
import { score } from './score.js';
import { input, outputOrValue } from './shared.js';
import { validate } from './validate.js';

export interface CommandResult {
  readonly result: unknown;
  readonly exitCode?: number;
}

const pilot = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, ['study-id', 'seed', 'output']);
  return outputOrValue(
    invocation,
    HARNESS_COMMANDS.pilot(
      requiredFlag(invocation, 'study-id'),
      requiredFlag(invocation, 'seed'),
    ),
  );
};

const packageCommand = async (invocation: CliInvocation): Promise<unknown> => {
  rejectUnknownFlags(invocation, ['input']);
  const body = objectValue(await input(invocation));
  return HARNESS_COMMANDS.package(
    stringValue(body['root'], 'root'),
    stringValue(body['destination'], 'destination'),
    stringArray(body['relativePaths'], 'relativePaths'),
  );
};

/** Dispatches every command without ever writing non-JSON data to stdout. */
export const executeCommand = async (
  invocation: CliInvocation,
): Promise<CommandResult> => {
  let result: unknown;
  if (invocation.command === 'validate') {
    result = await validate(invocation);
  } else if (invocation.command === 'conformance') {
    rejectUnknownFlags(invocation, []);
    const issues = await HARNESS_COMMANDS.conformance();
    result = { valid: issues.length === 0, issues };
  } else if (invocation.command === 'calibrate-models') {
    result = await calibrate(invocation);
  } else if (invocation.command === 'pilot') {
    result = await pilot(invocation);
  } else if (invocation.command === 'power') {
    result = await power(invocation);
  } else if (invocation.command === 'freeze') {
    result = await freeze(invocation);
  } else if (invocation.command === 'run') {
    result = await runProductionSchedule(invocation);
  } else if (invocation.command === 'score') {
    result = await score(invocation);
  } else if (invocation.command === 'review') {
    result = await review(invocation);
  } else if (invocation.command === 'analyze') {
    result = await analyze(invocation);
  } else if (invocation.command === 'package') {
    result = await packageCommand(invocation);
  } else {
    throw new TypeError(`unknown command: ${invocation.command}`);
  }

  const gateFailed =
    ['validate', 'conformance'].includes(invocation.command) &&
    objectValue(result)['valid'] === false;
  return { result, ...(gateFailed ? { exitCode: 1 } : {}) };
};

export const commandHash = (value: unknown): string => artifactHash(value);
