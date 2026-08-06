import type { ToolFactory } from 'tool';
import type { z } from 'zod';

import type { SkillSchema } from '../schemas/skill.js';

export type BundleManifestTool = {
  readonly path: string;
  readonly alwaysAvailable: boolean;
};

export type BundleManifestSkill = {
  readonly path: string;
  readonly alwaysAvailable: boolean;
};

export type BundleManifest = {
  readonly name: string;
  readonly description: string;
  readonly tools: readonly BundleManifestTool[];
  readonly skills: readonly BundleManifestSkill[];
};

export type BundleTool = {
  readonly factory: ToolFactory;
  readonly alwaysAvailable: boolean;
};

type SkillValue = z.output<typeof SkillSchema>;

export type Skill = SkillValue;

export type BundleSkill = {
  readonly skill: Skill;
  readonly alwaysAvailable: boolean;
};

export type Bundle = {
  readonly name: string;
  readonly description: string;
  readonly tools: readonly BundleTool[];
  readonly skills: readonly BundleSkill[];
};
