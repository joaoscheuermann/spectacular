import * as z from 'zod';
import { SkillHintSchema } from '../schemas/hint/index.js';
import { Skill } from './skill.js';

export type SkillHint = z.infer<typeof SkillHintSchema>;

export interface SkillExtraction {
  goal: string,
  skill: Skill,
  hints: Array<SkillHint>
}
