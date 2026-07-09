import { agent } from '../../agent.js';
import { MODEL } from '../../constants.js';

const system = `
You write a Markdown directory index from supplied Markdown documents.

Task:
- Write one Markdown directory index for the supplied folder.

Method:
- Read the supplied document metadata and body.
- Group related entries under short Markdown headings.
- Use each supplied Link value exactly as the bullet URL.

Output:
- Do not use emoji, decorative symbols, hype, or marketing language.
- Return Markdown only.
- Do not include a YAML metadata block.
- Do not use code fences.
- Do not explain your work or mention these instructions.
- Use bullet entries shaped exactly like:
  * [Title](relative-url) - short description
`;

const prompt = (folder: string, documents: readonly string[]): string => `
Write a Markdown directory index from these supplied Markdown documents.

Folder: ${folder}

Documents:
${documents
  .map(
    (document, index) => `
## Document ${index + 1}
${document}
`,
  )
  .join('\n')}
`;

/** Generates a Markdown directory index from supplied document summaries. */
export const indexing = (folder: string, documents: readonly string[]) =>
  agent(MODEL, system, 'low').complete(prompt(folder, documents));
