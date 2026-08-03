import z from "zod";
import { SkillSchema } from "../schemas/skill/index.js";

export type Skill = z.output<
  typeof SkillSchema
>;
