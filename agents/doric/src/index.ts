import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import pino from 'pino';
import pretty from 'pino-pretty';

import { loadBundles, type Skill } from 'bundle';
import { createDockerClient } from 'docker';
import { createFetchTransport, createOpenAiProvider } from 'llms';
import mosaic from 'mosaic';
import { createSandbox } from 'sandbox';
import { createSandpool } from 'sandpool';
import type { Tool } from 'tool';
import { createVectorDatabase } from 'victor';

async function main() {
  const logger = pino(
    { level: 'debug' },
    pino.multistream([
      { level: 'info', stream: pretty() },
      {
        level: 'debug',
        stream: pino.destination(process.env.DORIC_LOG_FILE ?? 'doric.log'),
      },
    ]),
  );

  const provider = createOpenAiProvider({
    transport: createFetchTransport(),
    baseUrl: 'https://openrouter.ai/api/v1',
    apiKey: process.env.OPENROUTER_API_KEY,
    logger,
  });

  const models = {
    default: 'openai/gpt-5.6-luna',
    reranker: 'voyageai/rerank-2.5-lite',
    embedder: 'voyageai/voyage-4-large',
  } as const;
  const embeddingDimensions = 2048;

  logger.info({ msg: 'initializing' });

  const pool = createSandpool({
    minIdle: 1,
    maxContainers: 10,
    logger,
    create: () =>
      createSandbox({
        docker: createDockerClient(),
        image: 'node:22-slim',
        network: { mode: 'disabled' },
      }),
  });

  try {
    const bundles = await loadBundles(
      join(dirname(fileURLToPath(import.meta.url)), '..', 'bundles'),
    );

    const lease = await pool.acquire();

    logger.info({ msg: 'heating sandpool' });
    await pool.waitUntilHeated();

    try {
      const bundleTools = bundles.flatMap((bundle) => bundle.tools);
      const bundleSkills = bundles.flatMap((bundle) => bundle.skills);

      const skills = bundleSkills.map(({ skill }) => skill);
      const tools = bundleTools.map(({ factory, alwaysAvailable }) => ({
        tool: factory(lease.sandbox),
        alwaysAvailable,
      }));

      const skillEmbeddings = createVectorDatabase<Skill>({
        dimensions: embeddingDimensions,
        logger,
        embedding: async (input) =>
          provider.embedding({
            model: models.embedder,
            input,
            dimensions: embeddingDimensions,
          }),
      });

      const toolEmbeddings = createVectorDatabase<Tool>({
        dimensions: embeddingDimensions,
        logger,
        embedding: async (input) =>
          provider.embedding({
            model: models.embedder,
            input,
            dimensions: embeddingDimensions,
          }),
      });

      logger.info({ msg: 'loaded bundles' });

      for (const skill of skills) {
        await skillEmbeddings.add(
          skill,
          ({ name, description, allowedTools, body }) =>
            `${name} | ${description} | ${allowedTools.join(',')} | ${body}`,
        );
      }

      for (const { tool } of tools) {
        await toolEmbeddings.add(
          tool,
          ({ name, description }) => `${name} | ${description ?? ''}`,
        );
      }

      logger.info({ msg: 'embedded all skills' });

      const prompt = `
        Extend the existing MOSAIC catalog with a new reusable capability for publishing finalized messages to Slack.

        The runtime currently has no Slack-specific operations. The capability must allow an agent to:

        1. discover available Slack channels;
        2. resolve a channel from a human-readable name;
        3. send a finalized message to the selected channel;
        4. report the observable result of the operation.

        Inspect the existing bundle before making changes. Determine whether this capability requires behavioral instructions, executable operations, or both. Create only the minimum coherent set of artifacts and do not duplicate capabilities that already exist.

        Authoring requirements:

        - Write all artifact contents in English.
        - Represent executable operations as JSON tool descriptors containing their names, descriptions, input schemas, and output schemas.
        - Do not implement handlers or runtime logic.
        - Represent reusable behavioral guidance as focused \`SKILL.md\` micro-skills.
        - Keep each skill centered on one coherent behavioral concern.
        - Do not create a monolithic “Slack agent” skill.
        - Include clear applicability, non-applicability, procedure, and completion guidance in each skill body.
        - Declare only the tools that the skill may actually require in \`allowed-tools\`.
        - Do not create a skill that merely repeats a tool description.
        - Register every new skill and tool in \`manifest.json\`.
        - Use \`alwaysAvailable: false\` unless an artifact is genuinely required by almost every unrelated objective.
        - Preserve the existing \`skills/*\`, \`tools/*\`, and \`manifest.json\` structure.
        - Reuse the existing core file, search, editing, shell, and validation capabilities instead of recreating them.

        Before completing the task, validate:

        - JSON syntax;
        - YAML frontmatter;
        - unique skill and tool names;
        - manifest paths;
        - resolution of every \`allowed-tools\` entry against the tool registry;
        - absence of redundant or overlapping artifacts.

        The result is complete when the MOSAIC bundle contains the smallest valid set of reusable skills and tool descriptors required for Slack channel discovery and message publication, all entries are registered, and the catalog remains internally consistent.

        Return a concise summary explaining:

        - which files were created or modified;
        - why each new artifact is a skill or a tool;
        - why no additional artifacts were necessary;
        - how the final capability should be composed during execution.
        `;

      const agent = mosaic({
        logger,
        provider,
        models,
        skills: {
          required: bundleSkills
            .filter(({ alwaysAvailable }) => alwaysAvailable)
            .map(({ skill }) => skill),
          menu: skills,
          embeddings: skillEmbeddings,
        },
        tools: {
          required: tools
            .filter(({ alwaysAvailable }) => alwaysAvailable)
            .map(({ tool }) => tool),
          menu: tools.map(({ tool }) => tool),
          embeddings: toolEmbeddings,
        },
      });

      await agent.prompt(prompt);
    } finally {
      await lease.release();
    }
  } finally {
    await pool.dispose();
  }
}

main().then().catch();
