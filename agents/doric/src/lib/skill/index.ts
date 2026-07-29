import { readFile } from 'node:fs/promises';
import { isMap, isScalar, parseDocument } from 'yaml';

export interface Skill {
  name: string;
  description: string;
  body: string;
  metadata: Record<string, string>;
  tools: Array<string>;
}

export default async function skill(path: string): Promise<Skill> {
  const source = await readFile(path, 'utf8');
  const opening = /^(?:---)\r?\n/.exec(source);

  if (!opening) {
    throw new Error(
      'Invalid SKILL.md frontmatter: expected an opening delimiter.',
    );
  }

  const frontmatterStart = opening[0].length;
  const closing = /^---\r?\n/m.exec(source.slice(frontmatterStart));

  if (!closing) {
    throw new Error(
      'Invalid SKILL.md frontmatter: expected a closing delimiter.',
    );
  }

  const frontmatterEnd = frontmatterStart + closing.index;
  const document = parseDocument(
    source.slice(frontmatterStart, frontmatterEnd),
    {
      uniqueKeys: true,
    },
  );

  if (document.errors.length > 0) {
    throw new Error(
      `Invalid SKILL.md frontmatter: ${document.errors[0].message}`,
    );
  }

  if (!isMap(document.contents)) {
    throw new Error('Invalid SKILL.md frontmatter: expected a YAML mapping.');
  }

  const name = document.get('name');
  const description = document.get('description');
  const metadataNode = document.get('metadata', true);
  const allowedTools = document.get('allowed-tools');

  if (typeof name !== 'string') {
    throw new Error('Invalid SKILL.md frontmatter: "name" must be a string.');
  }

  if (typeof description !== 'string') {
    throw new Error(
      'Invalid SKILL.md frontmatter: "description" must be a string.',
    );
  }

  if (allowedTools !== undefined && typeof allowedTools !== 'string') {
    throw new Error(
      'Invalid SKILL.md frontmatter: "allowed-tools" must be a string.',
    );
  }

  const metadata: Record<string, string> = {};

  if (metadataNode !== undefined) {
    if (!isMap(metadataNode)) {
      throw new Error(
        'Invalid SKILL.md frontmatter: "metadata" must be a mapping.',
      );
    }

    for (const item of metadataNode.items) {
      if (
        !isScalar(item.key) ||
        typeof item.key.value !== 'string' ||
        !isScalar(item.value) ||
        typeof item.value.value !== 'string'
      ) {
        throw new Error(
          'Invalid SKILL.md frontmatter: "metadata" must contain only string keys and values.',
        );
      }

      metadata[item.key.value] = item.value.value;
    }
  }

  return {
    name,
    description,
    body: source.slice(frontmatterEnd + closing[0].length),
    metadata,
    tools: allowedTools?.trim().split(/\s+/).filter(Boolean) ?? [],
  };
}
