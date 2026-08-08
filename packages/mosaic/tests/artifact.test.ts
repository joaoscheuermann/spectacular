import assert from 'node:assert/strict';
import test from 'node:test';
import * as z from 'zod';

import {
  ArtifactReferenceSchema,
  ArtifactSchema,
  InlineArtifactSchema,
} from '../src/index.js';

test('accepts and normalizes both discriminated artifact variants', () => {
  assert.deepEqual(
    InlineArtifactSchema.parse({
      kind: 'inline',
      mime: ' text/plain ',
      data: '',
    }),
    { kind: 'inline', mime: 'text/plain', data: '' },
  );
  assert.deepEqual(
    ArtifactReferenceSchema.parse({
      kind: 'reference',
      mime: ' application/json ',
      reference: ' urn:artifact:1 ',
    }),
    {
      kind: 'reference',
      mime: 'application/json',
      reference: 'urn:artifact:1',
    },
  );
});

test('rejects ambiguous legacy hybrid empty and unknown artifact shapes', () => {
  for (const artifact of [
    { mime: 'text/plain', data: 'legacy' },
    {
      kind: 'inline',
      mime: 'text/plain',
      data: 'inline',
      reference: 'hybrid',
    },
    { kind: 'reference', mime: 'text/plain', reference: ' ' },
    { kind: 'reference', mime: ' ', reference: 'opaque' },
    {
      kind: 'reference',
      mime: 'text/plain',
      reference: 'opaque',
      extra: true,
    },
  ]) {
    assert.equal(ArtifactSchema.safeParse(artifact).success, false);
  }
});

test('uses provider-compatible anyOf for artifact JSON Schema variants', () => {
  const schema = z.toJSONSchema(ArtifactSchema, {
    io: 'output',
    unrepresentable: 'throw',
  });

  assert.ok('anyOf' in schema);
  assert.equal('oneOf' in schema, false);
});
