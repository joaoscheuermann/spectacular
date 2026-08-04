import type * as z from 'zod';

import type { CompleteContext } from '../types/complete-context.js';

export const complete = async <Schema extends z.ZodObject>(
  system: string,
  user: string,
  { provider, schema, model }: CompleteContext<Schema>,
): Promise<z.output<Schema>> => {
  const result = await provider.complete({
    model,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
    schema,
  });

  return result.structured;
};
