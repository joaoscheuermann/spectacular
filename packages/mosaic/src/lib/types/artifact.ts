import type * as z from 'zod';

import type {
  ArtifactReferenceSchema,
  ArtifactSchema,
  InlineArtifactSchema,
} from '../schemas/artifact.js';

export type InlineArtifact = z.output<typeof InlineArtifactSchema>;
export type ArtifactReference = z.output<typeof ArtifactReferenceSchema>;
export type Artifact = z.output<typeof ArtifactSchema>;
