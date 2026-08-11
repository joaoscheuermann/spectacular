import * as z from 'zod';

const NonEmptyStringSchema = z.string().trim().min(1);

/** Artifact content carried directly by the MOSAIC runtime. */
export const InlineArtifactSchema = z
  .object({
    kind: z
      .literal('inline')
      .describe(
        'Artifact variant discriminator; use inline only when data contains the complete artifact content.',
      ),
    mime: NonEmptyStringSchema.describe(
      'Non-empty MIME type describing the inline data.',
    ),
    data: z
      .string()
      .describe('Complete inline artifact content; an empty string is valid.'),
  })
  .strict();

/** Opaque artifact reference transported without runtime resolution. */
export const ArtifactReferenceSchema = z
  .object({
    kind: z
      .literal('reference')
      .describe(
        'Artifact variant discriminator; use reference when reference identifies externally stored content.',
      ),
    mime: NonEmptyStringSchema.describe(
      'Non-empty MIME type describing the referenced artifact.',
    ),
    reference: NonEmptyStringSchema.describe(
      'Non-empty opaque reference to the artifact; provide an identifier or location, not the artifact content.',
    ),
  })
  .strict();

/** Public artifact contract with strict variants distinguished by `kind`. */
export const ArtifactSchema = z.union([
  InlineArtifactSchema,
  ArtifactReferenceSchema,
]);
