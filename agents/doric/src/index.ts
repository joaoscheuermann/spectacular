import type * as z from 'zod';
import pino from 'pino';
import pretty from 'pino-pretty';

import { createVectorDatabase } from 'victor';
import { createFetchTransport, createOpenAiProvider, LlmProvider } from 'llms';

import { K_RETRIEVE, WAVE_MAX } from './lib/constants/index.js';
import { Skill } from './lib/types/skill.js';
import { bundles } from './lib/loaders/index.js';
import { decompose } from './lib/decomposition/index.js';
import { Node } from './lib/types/graph.js';
import { markdownNumberedList } from './lib/decomposition/utils/index.js';
import { rerankQuery } from './lib/prompts/rerank/index.js';

import * as bundle from './lib/prompts/bundle/index.js';
import { BundleSchema } from './lib/schemas/bundle/index.js';

interface CompleteContext<Schema extends z.ZodObject> {
  provider: LlmProvider;
  schema: Schema;
  model: string;
}

const complete = async <Schema extends z.ZodObject>(
  system: string,
  user: string,
  { provider, schema, model }: CompleteContext<Schema>,
): Promise<z.output<Schema>> => {
  const result = await provider.complete({
    model: model,
    messages: [
      {
        role: 'system',
        content: system,
      },
      {
        role: 'user',
        content: user,
      },
    ],
    schema: schema,
  });

  return result.structured;
};

/**
 * Main entry point!
 */
async function main() {
  const logger = pino(pretty());

  const provider = createOpenAiProvider({
    transport: createFetchTransport(),
    baseUrl: 'https://openrouter.ai/api/v1',
    apiKey: process.env.OPENROUTER_API_KEY,
  });

  const vectors = createVectorDatabase<Skill>({
    dimensions: 2560,
    embedding: async (data: string) =>
      provider.embedding({ model: 'perplexity/pplx-embed-v1-4b', input: data }),
  });

  logger.info({ msg: 'initializing' });

  // Find all skill locally.
  const { skills, tools } = await bundles(
    'C:/Users/jvito/Documents/git/spectacular/doric/agents/doric/bundles',
  );

  logger.info({ msg: 'loaded bundles' });

  for (const skill of skills) {
    await vectors.add(
      skill,
      ({ name, description, allowedTools, body }) =>
        `${name} | ${description} | ${allowedTools.join(',')} | ${body}`,
    );
  }

  logger.info({ msg: 'embedded all skills' });

  const prompt = `
    Localize o relatório financeiro mais recente da empresa e confirme que ele
    corresponde ao segundo trimestre de 2026.

    Depois:

    1. extraia os principais dados de receita, margem, custos e geração de caixa;
    2. compare esses resultados com o trimestre anterior;
    3. identifique a melhor oportunidade de melhoria com base nas evidências;
    4. produza um resumo executivo em Markdown;
    5. salve o resumo em um arquivo chamado financial-review.md;
    6. envie uma mensagem ao canal #finance do Slack contendo a recomendação e a
       referência para o arquivo criado.

    Não invente dados ausentes e explicite limitações encontradas no relatório.
  `;

  const graph = await decompose(prompt, { logger, provider, vectors });

  logger.info({ msg: 'generated the execution plan' });

  console.log(JSON.stringify(graph));

  // Only stops when all nodes are delivered
  while (!graph.nodes.every((node) => node.status === 'completed')) {
    // Get all ready nodes for this execution
    const nodes = graph.nodes
      .reduce((ready, evaluationNode) => {
        // If the node ran already, we skip
        if (evaluationNode.status !== 'pending') return ready;

        // If it have no depedency, it's already ready
        if (!evaluationNode.dependsOn.length) return [...ready, evaluationNode];
        // Check all other dependencies for necessary states
        else {
          const dependenciesCompleted = graph.nodes
            .filter((graphNode) =>
              evaluationNode.dependsOn.includes(graphNode.id),
            )
            .every((dependency) => dependency.status === 'completed');

          if (dependenciesCompleted) return [...ready, evaluationNode];
        }

        return ready;
      }, [] as Array<Node>)
      .sort((a, b) => b.index - a.index); // Order

    // Maximum ammount of nodes for this run, this is meant to adjust parallelism
    const wave = nodes.slice(0, WAVE_MAX);

    if (!nodes.length)
      throw new Error('Impossible to continue, missing ready nodes!');

    for (const node of wave) {
      // TODO: improve the side effect, this should not mutate the original graph state...
      node.status = 'ready';

      // Search for matches for the current goal
      const matches = await vectors.search(
        [
          `Original Request:\n${prompt}`,
          `Current Goal::\n${node.goal}`,
          `Completion Criteria:\n${markdownNumberedList(node.doneWhen)}`,
          // TODO: get the context from previous complete nodes, we are missing the global state management
        ].join('\n\n'),
        K_RETRIEVE,
      );

      const candidates = matches.map((result) => result.data);

      // Re-rank all the candidates, so they are meaningfull for the end result
      const ranking = await provider.rerank({
        model: 'voyageai/rerank-2.5-lite',
        query: rerankQuery(prompt, node),
        documents: candidates.map((skill) =>
          [
            `Skill name: ${skill.name}`,
            `Description:\n${skill.description.trim()}`,
            `Canonical body:\n${skill.body.trim()}`,
          ].join('\n\n'),
        ),
        topN: K_RETRIEVE,
      });

      // Rebuild the Skill map with the reranked skills
      const reranked = ranking.map((result) => candidates[result.index]);

      // Decides which skills are kept
      const selectedSkills = await complete(
        bundle.system(),
        bundle.user(prompt, node, reranked),
        {
          provider,
          schema: BundleSchema,
          model: `openai/gpt-5.6-luna`,
        },
      );

      // Pick all the skills documents from the already retrieved skills
      const skillsMenu = selectedSkills.skills.map((pick) =>
        candidates.find((skill) => skill.name === pick),
      );

      // TODO: we should have a base set of tools, we don't have a mechanism to define that yet...
      // probably it will be defined in the manifest.json file when we implement the complete bundle loading in the monorepo.

      // Pick all tools available from the always available toolset and the tools defined by the 'allowedTools' property into a skill
      const toolsMenu = [
        ...new Set(skillsMenu.flatMap((skill) => skill?.allowedTools ?? [])),
      ]
        .map((name) => tools.find((tool) => tool.name === name));

      console.log(JSON.stringify(selectedSkills));
      console.log(JSON.stringify(skillsMenu));
      console.log(JSON.stringify(toolsMenu));
    }
  }
}

main().then().catch();
