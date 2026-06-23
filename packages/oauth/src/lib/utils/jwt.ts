import { asRecord } from './json.js';

export const parseTokenClaims = (
  value: string,
): Readonly<Record<string, unknown>> | undefined => {
  const [, payload] = value.split('.');

  if (payload === undefined) {
    return undefined;
  }

  try {
    return asRecord(
      JSON.parse(Buffer.from(payload, 'base64url').toString('utf8')),
    );
  } catch {
    return undefined;
  }
};
