import * as z from 'zod';
import type { Skill } from 'bundle';
import { SkillHintSchema } from '../schemas/hint/index.js';

export type SkillHint = z.infer<typeof SkillHintSchema>;

export interface SkillExtraction {
  goal: string;
  skill: Skill;
  hints: Array<SkillHint>;
}
