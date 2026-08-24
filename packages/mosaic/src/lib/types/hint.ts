import * as z from 'zod';
import type { Skill } from 'bundle';
import { SkillHintSchema } from '../schemas/hint.js';

export type SkillHint = z.infer<typeof SkillHintSchema>;

export interface SkillExtraction {
  readonly goalId: string;
  skill: Skill;
  hints: SkillHint[];
}
