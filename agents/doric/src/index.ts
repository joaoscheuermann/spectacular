import * as z from 'zod';
import pino from 'pino';
import pretty from 'pino-pretty';

import { glob } from 'glob';

import { createVectorDatabase } from 'victor';

import { createFetchTransport, createOpenAiProvider } from 'llms';

import skill, { type Skill } from './lib/skill/index.js';

const logger = pino(pretty());

const provider = createOpenAiProvider({
  transport: createFetchTransport(),
  baseUrl: 'https://openrouter.ai/api/v1',
  apiKey: process.env.OPENROUTER_API_KEY
});

const vectors = createVectorDatabase<Skill>({
  dimensions: 2560,
  embedding: async (data: string) => provider.embedding({ model: 'perplexity/pplx-embed-v1-4b', input: data }),
})

async function main() {
  logger.info({ msg: 'initializing' });

  // Find all skill locally.
  const paths =  await glob(
    'C:/Users/jvito/Documents/git/spectacular/doric/.agents/skills/**/SKILL.md',
  )

  logger.info({ msg: 'skills found', paths });

  // Converts each skill path into an entry in the vector database
  for (const path of paths) {
    const { name, description, body, tools, metadata } = await skill(path);

    await vectors.add(
      { name, description, body, tools, metadata },
      ({ name, description, body }) => `${name} | ${description} | ${body}`,
    )

    logger.info({ msg: 'skill ingested', title: name, description });
  }

  const prompt = "Prompt: Crie um prompt para meu agente, esse prompt vai instruir meu agente à extrair todos os numeros encontrados em um texto."

  const system = `
    You are a task decomposition assistant.
    Given a complex user query, break it down into atomic sub-tasks.
    Each string should be a concise, actionable sub-task description.
  `

  const test = await provider.complete({
    messages: [
      {
        role: "system",
        content: system
      },
      {
        role: "user",
        content: prompt
      }
    ],
    model: `google/gemini-3.5-flash-lite`,
    schema: z.object({
      steps: z.array(z.string()).describe("an array of steps")
    })
  })

  console.log(test.structured)
}

main()
  .then()
  .catch();
