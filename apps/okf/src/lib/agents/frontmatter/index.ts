import * as z from 'zod';

import { agent } from '../../agent.js';

const system = `
  # 1. Goal
  Extract the following information.

  * **\`type\`**
    A short string identifying the kind of concept (such as \`BigQuery Table\`, \`BigQuery Dataset\`, \`API Endpoint\`, \`Metric\`, \`Playbook\`, or \`Reference\`). Consumers rely on this field for routing, filtering, and presentation. While values are not centrally registered, they must be descriptive and self-explanatory, and consumers must gracefully tolerate and process unknown type values.

    * **\`title\`**
    The human-readable display name for the concept. If this field is omitted or cannot be read, consumers will fall back to deriving a title directly from the document's filename.

  * **\`description\`**
    A single sentence summarizing the concept. This summary is consumed by index generators, search snippets, and previews to provide quick context about the file's contents.

  * **\`tags\`**
    A YAML list of short strings used for cross-cutting categorization. These tags allow consumption systems to aggregate, filter, and browse documents dynamically without needing a separate taxonomic file structure.

  ## 2. Terminology
  - **Knowledge Bundle** — A self-contained, hierarchical collection of
    knowledge documents. The unit of distribution.
  - **Concept** — A single unit of knowledge within a bundle. Represented
    as one markdown document. May describe a tangible asset (a table, an
    API), an abstract idea (a metric, a business process), or anything in
    between.
  - **Concept ID** — The path of the concept's file within the bundle,
    with the \`.md\` suffix removed. For example, \`tables/users.md\` has
    concept ID \`tables/users\`.
    - **Frontmatter** — YAML metadata block delimited by \`---\` at the top of
    a markdown file.
  - **Body** — Everything in the file after the frontmatter.
  - **Link** — A standard markdown link from one concept to another, used
    to express relationships beyond the implicit parent/child hierarchy.
  - **Citation** — A link from a concept to an external source that
    supports a claim in the body.
`;

const schema = z.object({
  type: z.string(),
  title: z.string(),
  description: z.string(),
  tags: z.array(z.string()),
});

const prompt = (path: string, body: string) => `
  ~~~\`${path}\`
  ${body}
  ~~~
`;

export const frontmatter = (path: string, body: string) =>
  agent('google/gemma-4-e4b', system, 'low').complete(prompt(path, body), {
    schema,
  });
