import type { LlmProvider } from 'llms';
import type * as z from 'zod';

export interface CompleteContext<Schema extends z.ZodObject> {
  readonly provider: LlmProvider;
  readonly schema: Schema;
  readonly model: string;
}
