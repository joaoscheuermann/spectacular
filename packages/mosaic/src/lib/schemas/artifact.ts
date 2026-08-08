import * as z from 'zod';

const NonEmptyStringSchema = z.string().trim().min(1);

/** Artifact content carried directly by the MOSAIC runtime. */
export const InlineArtifactSchema = z
  .object({
    kind: z.literal('inline'),
    mime: NonEmptyStringSchema.describe('MIME type of the artifact.'),
    data: z.string().describe('Inline artifact content.'),
  })
  .strict();

/** Opaque artifact reference transported without runtime resolution. */
export const ArtifactReferenceSchema = z
  .object({
    kind: z.literal('reference'),
    mime: NonEmptyStringSchema.describe('MIME type of the artifact.'),
    reference: NonEmptyStringSchema.describe('Opaque artifact reference.'),
  })
  .strict();

/** Public artifact contract with strict variants distinguished by `kind`. */
export const ArtifactSchema = z.union([
  InlineArtifactSchema,
  ArtifactReferenceSchema,
]);
