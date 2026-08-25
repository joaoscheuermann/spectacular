const SECRET_PATTERN =
  /\b(sk-[A-Za-z0-9_-]{6,}|Bearer\s+[A-Za-z0-9._-]{8,})\b/g;

export const redactSecrets = (value: string): string =>
  value.replace(SECRET_PATTERN, (match) =>
    match.startsWith('Bearer ') ? 'Bearer [redacted]' : 'sk-[redacted]',
  );

export const diagnosticExcerpt = (value: string, limit = 2048): string => {
  const redacted = redactSecrets(value);

  if (redacted.length <= limit) {
    return redacted;
  }

  return `${redacted.slice(0, limit)}...[truncated]`;
};

export const redactDiagnosticValue = (value: unknown): unknown => {
  if (typeof value === 'string') {
    return redactSecrets(value);
  }

  if (Array.isArray(value)) {
    return value.map(redactDiagnosticValue);
  }

  if (value !== null && typeof value === 'object') {
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [
        key,
        redactDiagnosticValue(child),
      ]),
    );
  }

  return value;
};
