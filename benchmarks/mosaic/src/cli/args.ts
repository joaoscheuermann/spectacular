export interface CliInvocation {
  readonly command: string;
  readonly subcommand?: string;
  readonly flags: Readonly<Record<string, string | true>>;
}

const flagName = (value: string): string => {
  const name = value.slice(2);
  if (!/^[a-z][a-z0-9-]*$/u.test(name)) {
    throw new TypeError(`invalid option: ${value}`);
  }
  return name;
};

/** Parses a deliberately small, shell-independent CLI grammar. */
export const parseInvocation = (argv: readonly string[]): CliInvocation => {
  const [command, ...tail] = argv;
  if (command === undefined || command.startsWith('-')) {
    throw new TypeError('a command is required');
  }

  let offset = 0;
  const subcommand =
    command === 'review' && tail[0] !== undefined && !tail[0].startsWith('-')
      ? tail[offset++]
      : undefined;
  const flags: Record<string, string | true> = {};

  while (offset < tail.length) {
    const argument = tail[offset];
    if (argument === undefined || !argument.startsWith('--')) {
      throw new TypeError(`unexpected positional argument: ${argument ?? ''}`);
    }
    const separator = argument.indexOf('=');
    const name = flagName(
      separator === -1 ? argument : argument.slice(0, separator),
    );
    if (name in flags) throw new TypeError(`duplicate option: --${name}`);
    if (separator !== -1) {
      const value = argument.slice(separator + 1);
      if (value.length === 0) throw new TypeError(`empty option: --${name}`);
      flags[name] = value;
      offset += 1;
      continue;
    }
    const next = tail[offset + 1];
    if (next !== undefined && !next.startsWith('--')) {
      flags[name] = next;
      offset += 2;
    } else {
      flags[name] = true;
      offset += 1;
    }
  }

  return {
    command,
    ...(subcommand === undefined ? {} : { subcommand }),
    flags,
  };
};

export const optionalFlag = (
  invocation: CliInvocation,
  name: string,
): string | undefined => {
  const value = invocation.flags[name];
  if (value === undefined) return undefined;
  if (value === true) throw new TypeError(`--${name} requires a value`);
  return value;
};

export const requiredFlag = (
  invocation: CliInvocation,
  name: string,
): string => {
  const value = optionalFlag(invocation, name);
  if (value === undefined) throw new TypeError(`--${name} is required`);
  return value;
};

export const booleanFlag = (
  invocation: CliInvocation,
  name: string,
): boolean => {
  const value = invocation.flags[name];
  if (value === undefined) return false;
  if (value !== true) throw new TypeError(`--${name} does not accept a value`);
  return true;
};

export const rejectUnknownFlags = (
  invocation: CliInvocation,
  allowed: readonly string[],
): void => {
  const accepted = new Set(allowed);
  const unknown = Object.keys(invocation.flags).filter(
    (name) => !accepted.has(name),
  );
  if (unknown.length > 0)
    throw new TypeError(`unknown option: --${unknown[0]}`);
};
