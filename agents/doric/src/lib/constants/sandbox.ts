export const DEFAULT_SANDBOX_IMAGE = 'node:slim';
export const GIT_PROBE_COMMAND = [
  'sh',
  '-lc',
  'command -v git >/dev/null 2>&1',
] as const;
export const GIT_INSTALL_COMMAND = [
  'sh',
  '-lc',
  'apt-get update && apt-get install -y --no-install-recommends git ca-certificates && rm -rf /var/lib/apt/lists/*',
] as const;
