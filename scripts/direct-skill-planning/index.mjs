#!/usr/bin/env node

import { createFetchTransport, createUnifiedProvider } from 'llms';
import pino from 'pino';
import * as z from 'zod';

import { planningCase } from './case.mjs';
import {
  requestOnlySystem,
  requestOnlyUser,
  skillAwareSystem,
  skillAwareUser,
} from './prompts.mjs';

const model = 'deepseek/deepseek-v4-pro';
const temperature = 0;
const planSchema = z
  .object({
    goals: z
      .array(
        z
          .object({
            goal: z.string().trim().min(1),
            doneWhen: z.array(z.string().trim().min(1)).min(1),
          })
          .strict(),
      )
      .min(1),
  })
  .strict();

const createProvider = () => {
  const apiKey = process.env.OPENROUTER_API_KEY?.trim();
  if (!apiKey) throw new Error('OPENROUTER_API_KEY is required.');

  return createUnifiedProvider({
    transport: createFetchTransport(),
    apiKey,
    logger: pino({ enabled: false }),
  });
};

const completePlan = async (provider, system, user) => {
  const result = await provider.complete({
    model,
    temperature,
    messages: [
      { role: 'system', content: system },
      { role: 'user', content: user },
    ],
    schema: planSchema,
  });

  return result.structured;
};

const provider = createProvider();
const [requestOnly, directWithSkills] = await Promise.all([
  completePlan(
    provider,
    requestOnlySystem,
    requestOnlyUser(planningCase.objective),
  ),
  completePlan(provider, skillAwareSystem, skillAwareUser(planningCase)),
]);

console.log(
  JSON.stringify(
    {
      case: planningCase.name,
      objective: planningCase.objective,
      retrievedSkills: planningCase.skills.map(({ name }) => name),
      requestOnly,
      directWithSkills,
    },
    null,
    2,
  ),
);
