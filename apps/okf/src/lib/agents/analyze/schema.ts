import * as z from 'zod';

export const schema = z.object({
  summary: z
    .string()
    .describe(
      'Concise Markdown summary of the supplied file, without YAML frontmatter.',
    ),
});

export type Analysis = z.infer<typeof schema>;
