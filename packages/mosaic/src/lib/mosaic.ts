import { K_RETRIEVE, WAVE_MAX } from './constants/index.js';
import { decompose } from './decomposition/index.js';
import * as bundlePrompt from './prompts/bundle/index.js';
import { rerankQuery } from './prompts/rerank/index.js';
import { markdownNumberedList } from './prompts/utils/index.js';
import { BundleSchema } from './schemas/bundle/index.js';
import type { Node } from './types/graph.js';
import type { MosaicAgent } from './types/mosaic-agent.js';
import type { MosaicOptions } from './types/mosaic-options.js';
import { complete } from './utils/complete.js';
import { resolveSkills } from './utils/resolve-skills.js';
import { resolveTools } from './utils/resolve-tools.js';
import { unique } from './utils/unique.js';

export function mosaic(options: MosaicOptions): MosaicAgent {
  const { logger, provider, models, skills, tools } = options;

  return {
    async prompt(input: string): Promise<void> {
      const graph = await decompose(input, {
        logger,
        provider,
        vectors: skills.embeddings,
        model: models.default,
      });

      logger.info({ msg: 'generated the execution plan' });
      console.log(JSON.stringify(graph));

      while (!graph.nodes.every((node) => node.status === 'completed')) {
        const nodes = graph.nodes
          .reduce((ready, evaluationNode) => {
            if (evaluationNode.status !== 'pending') return ready;
            if (!evaluationNode.dependsOn.length)
              return [...ready, evaluationNode];

            const dependenciesCompleted = graph.nodes
              .filter((graphNode) =>
                evaluationNode.dependsOn.includes(graphNode.id),
              )
              .every((dependency) => dependency.status === 'completed');

            if (dependenciesCompleted) return [...ready, evaluationNode];
            return ready;
          }, [] as Node[])
          .sort((a, b) => b.index - a.index);

        const wave = nodes.slice(0, WAVE_MAX);

        if (!nodes.length) {
          throw new Error('Impossible to continue, missing ready nodes!');
        }

        for (const node of wave) {
          node.status = 'ready';

          // TODO: agregar resultados anteriores quando disponiveis
          // filtar os resultados de nós anteriores do qual esse nó depende
          const matches = await skills.embeddings.search(
            [
              `Original Request:\n${input}`,
              `Current Goal::\n${node.goal}`,
              `Completion Criteria:\n${markdownNumberedList(node.doneWhen)}`,
            ].join('\n\n'),
            K_RETRIEVE,
          );

          const candidates = matches.map(({ data }) => data);

          const ranking = await provider.rerank({
            model: models.reranker,
            query: rerankQuery(input, node),
            documents: candidates.map((skill) =>
              [
                `Skill name: ${skill.name}`,
                `Description:\n${skill.description.trim()}`,
                `Canonical body:\n${skill.body.trim()}`,
              ].join('\n\n'),
            ),
            topN: K_RETRIEVE,
          });
          const reranked = ranking.map(({ index }) => candidates[index]);
          const skillCandidates = unique([...skills.required, ...reranked]);
          const selected = await complete(
            bundlePrompt.system(),
            bundlePrompt.user(input, node, skillCandidates),
            { provider, schema: BundleSchema, model: models.default },
          );
          const skillMenu = resolveSkills(
            skills.required,
            selected.skills,
            skills.menu,
          );
          const toolMenu = resolveTools(tools.required, skillMenu, tools.menu);

          console.log(JSON.stringify(selected));
          console.log(JSON.stringify(skillMenu));
          console.log(JSON.stringify(toolMenu));
        }
      }
    },
  };
}

export default mosaic;
