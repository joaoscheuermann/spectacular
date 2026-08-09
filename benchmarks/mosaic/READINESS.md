# MOSAIC: prontidão para um estudo válido

Este documento é o registro de prontidão do benchmark empírico MOSAIC. Ele
separa a validade do instrumento das decisões humanas e das condições
operacionais exigidas para iniciar um estudo oficial pago.

Status da auditoria: **2026-08-09 — GO do instrumento; NO-GO operacional para
o estudo oficial**.

Todos os bloqueios de implementação P0 e P1 identificados em 2026-08-08 foram
fechados e possuem testes. A execução paga continua proibida enquanto os gates
humanos e operacionais abaixo estiverem abertos. O resultado de
`--check-readiness` é a autoridade executável para um config e artifact root
concretos; aprovações humanas nunca são inferidas pelo código.

## 1. O que significa "válido"

Há três marcos diferentes:

1. **Execução técnica válida:** cada chamada paga pertence a uma tentativa
   imutável; respostas persistidas possuem usage/custo autenticado e trace
   verificável, enquanto uma interrupção ambígua vira record terminal e nunca é
   repetida.
2. **Estudo confirmatório válido:** além do item anterior, o outcome mede o
   pedido de forma determinística e neutra, as condições diferem somente pelos
   fatores declarados e todos os artefatos pré-confirmatórios estão congelados.
3. **Estudo publicável completo:** além dos dois itens anteriores, a revisão
   humana cega, diagnósticos escolhidos, relatórios e pacote auditado foram
   concluídos.

O instrumento satisfaz o primeiro marco por construção e fornece os gates
necessários para o segundo. Satisfazer esses gates em uma execução concreta
ainda depende dos insumos humanos e do ambiente. O terceiro marco sempre inclui
trabalho humano posterior ao resultado computacional.

### Legenda

- **P0:** afeta estimand, outcome, custo ou provenance e bloqueia o instrumento.
- **P1:** torna o fluxo oficial seguro e executável sem improvisação.
- **P2:** diagnóstico, resultado secundário ou etapa de publicação.
- **H:** decisão ou auditoria humana; o runner não pode fabricá-la.
- **O:** requisito operacional de uma execução concreta.

## 2. Bloqueios de implementação encerrados

### P0

| Item  | Estado    | Evidência implementada                                                                                                                                                                                          |
| ----- | --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0-01 | Resolvido | `CaseV1` usa um documento JSON canônico, campos de evidência e critérios ligados por `evidenceRefs`. Eco, omissão, fato incorreto, efeito incorreto e world divergente falham.                                  |
| P0-02 | Resolvido | B0–B3 e MOSAIC usam o mesmo runtime estruturado, duas tentativas de reparo, feedback sanitizado e medição de cada chamada.                                                                                      |
| P0-03 | Resolvido | `maxTurns: 16` está congelado em todas as condições e em `ConditionFactorsV1`.                                                                                                                                  |
| P0-04 | Resolvido | Todas as condições com skills usam a mesma fronteira híbrida body-aware e o mesmo reranker. B1 seleciona uma skill por pedido, B2 uma por subtarefa, B3 top-3 fixo por objetivo e M1 preserva seleção seletiva. |
| P0-05 | Resolvido | O índice é construído uma vez, content-addressed, ligado a catálogo, textos, embedder, dimensão e algoritmo. Setup possui reserva, journal, resume conservador, custo separado e hash no freeze.                |
| P0-06 | Resolvido | Usage autenticado de completion, embedding e rerank é capturado por resposta, com tokens, documentos, search units, requests, falhas parciais e journal por tentativa.                                          |
| P0-07 | Resolvido | O price contract exige USD, kind, timestamp, fonte, unidade aplicável, quantidade, valor positivo e tiers contíguos quando usados.                                                                              |
| P0-08 | Resolvido | A tentativa é reservada antes da atividade; resume reconcilia reservas interrompidas e testes em processo filho cobrem `SIGKILL` em todas as fronteiras críticas.                                               |

O contrato público de `packages/llms` não foi ampliado: a medição de retrieval
envolve o transporte HTTP dentro do benchmark e preserva a separação do
instrumento privado.

### P1

| Item  | Estado    | Evidência implementada                                                                                                                                                                 |
| ----- | --------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P1-01 | Resolvido | `scripts/cases.mjs compile` transforma drafts semânticos estritos em casos, executa tools determinísticas, deriva hashes/effects/world/delivery e recusa overwrite.                    |
| P1-02 | Resolvido | `study.mjs --prepare` para depois do power e publica `nFinal`; `--continue` exige config separado, mesmo study identity e corpus confirmatório exato antes do freeze.                  |
| P1-03 | Resolvido | `scripts/cases.mjs validate` agrega todas as issues de schema, fase, referências, hashes, execução, balanceamento e isolamento.                                                        |
| P1-04 | Resolvido | Metadata gratuita valida IDs, tools, structured output e effort antes do piloto; probes de embedding/rerank são explícitos, pagos, medidos e não são repetidos após fronteira ambígua. |
| P1-05 | Resolvido | Seeds controlam schedule, ordem, shuffles e hooks determinísticos; não há alegação de controle sobre sampling do provider.                                                             |
| P1-06 | Resolvido | Power exige rationale/approval ligado por hash, registra recursos e faz checkpoint/resume preservando seed e sequência de candidatos.                                                  |
| P1-07 | Resolvido | `--check-readiness` agrega blockers de input, implementação, auditoria humana e operação, distinguindo `envelopeValid`, `stageReady` e `studyReady`.                                   |

O gate de schema compara o conteúdo gerado a partir dos contratos Zod com cada
JSON Schema versionado; a mera existência do arquivo não é suficiente.

## 3. Insumos e gates humanos obrigatórios

Estes itens não são bugs e não podem ser inventados pelo runner.

### H-01 — Corpus de calibração

O repositório contém um draft oficial e 60 casos compilados:

- `cases/calibration-draft.v1.json`;
- `cases/calibration-cases.v1.json`;
- `cases/calibration-h01-review-template.md`;
- `cases/calibration-audit.v1.json`.

A validação estrutural confirma 60 IDs e families únicos, 15 casos por domínio,
10 por classe A–F, fase `calibration`, hashes e execução determinística sem
issues. O hash canônico do corpus é
`sha256:937d781ee6512adb3a7bd2af58550e672e29719d306aa476b4761ed9efb986c6`.
João Vitor Scheuermann concluiu e atestou a revisão humana em
2026-08-09T15:52:27.000Z, antes de observar resultados pagos. O registro cobre
inglês, neutralidade, dificuldade, ausência de marcador de resposta,
equivalência procedural, overlap, distratores, conflitos e isolamento
semântico. A aprovação machine-readable está ligada ao hash canônico exato do
corpus; qualquer mudança o invalida.

### H-02 — Candidato de replicação

Luna é o modelo primário congelado. O `candidate` do config deve ser uma família
não OpenAI. Qwen é o primeiro candidato, não uma aprovação antecipada.

O continue só pode avançar se, nos 60 casos de calibração e três repetições M1
pareadas, o IC bootstrap de 95% da diferença para Luna estiver em
`[-0,05, +0,05]`. Reprovar é um resultado científico válido; trocar candidato
exige decisão documentada e novo config/root quando a identidade congelada
mudar.

### H-03 — Parâmetros de power

Antes de `--prepare`, um responsável estatístico deve justificar e aprovar,
ligado ao hash exato do config:

- probabilidade de sucesso do baseline e desvio-padrão do intercepto aleatório;
- classes adaptativas, efeito mínimo de 10 pontos percentuais e alpha 0,05;
- 10.000 simulações, seed e regra do menor tamanho elegível;
- `N_final = max(240, N_power)`, arredondado ao múltiplo seguinte de 120;
- estimativa de CPU, memória e disco.

### H-04 — Corpus confirmatório

Somente depois de `--prepare` publicar `nFinal`:

- autorar exatamente `nFinal` families independentes;
- garantir `nFinal >= 240` e múltiplo de 120;
- balancear exatamente a matriz 6 classes × 4 domínios;
- usar `phase: "confirmatory"`;
- isolar de piloto e calibração por family, fingerprint e auditoria semântica;
- preencher a auditoria confirmatória ligada ao hash do corpus;
- congelar antes de observar qualquer resultado confirmatório.

### H-05 — Snapshot de preços

Capturar os preços vigentes dos modelos primary, candidate, embedder e reranker,
com unidades, tiers, timestamp e fontes. Zeros, valores ilustrativos e
estimativas sem provenance são rejeitados.

### H-06 — Autorização de custo

No menor desenho permitido (`nFinal = 240`), o fluxo agenda pelo menos:

- piloto: `60 × 6 × 5 = 1.800` runs;
- calibração: `60 × 3 × 2 = 360` runs;
- primário: `240 × 2 × 5 = 2.400` runs;
- replicação: `2.400` runs;
- sensibilidade: `2.400` runs.

Total mínimo: **9.360 runs**, fora probes, indexação, ablações, oracles e judge
auxiliar. O custo máximo aprovado deve incluir setup e estar ligado ao snapshot
de preços exato.

## 4. Requisitos operacionais

### O-01 — Repositório congelado

- worktree limpo e commit exato registrado;
- `npx nx sync --check`, projetos, typecheck, build e testes verdes;
- JSON Schemas regenerados sem diff;
- `git diff --check` verde.

O worktree de desenvolvimento atual é necessariamente sujo por estas mudanças.
Ele precisa ser revisado e commitado antes de uma execução oficial.

### O-02 — Imagem R imutável

- reconstruir a imagem com o conteúdo atualizado do benchmark;
- restaurar `renv.lock` e executar a análise sem rede;
- executar `analysis/test-analysis.R` no sandbox do container;
- disponibilizar a imagem como `repository@sha256:<64-hex>`;
- provar que `/benchmark/analysis` e `renv.lock` correspondem ao commit.

Uma imagem local atualizada foi construída nesta auditoria como
`sha256:204cfae565a61b0423ca5e5393eb3b867823b77c863f740d50dbf42907f1f198`.
Ela passou o smoke R offline e contém os mesmos hashes dos 18 arquivos locais
em `analysis/`. Uma execução distribuída ainda deve publicar essa imagem e usar
o digest retornado pelo registry.

### O-03 — Ambiente de execução

- Node e dependências instalados;
- Docker com CPU, memória e disco aprovados;
- artifact root absoluto, novo e fora do repositório;
- rede disponível somente para as chamadas externas previstas;
- `OPENROUTER_API_KEY` carregada em runtime pelo secret manager;
- nenhuma credencial em config, manifest, trace, record ou receipt.

### O-04 — Configs finais

O config de preparação aponta para preços, calibração/auditoria, power/approval,
candidate e imagem por digest. O config de continuação mantém a mesma identidade
e acrescenta o corpus confirmatório/auditoria. Os bytes são copiados uma única
vez para o root externo; mudar um input exige uma nova identidade/root.

## 5. Gate atual de aceite

### Instrumento

- [x] P0-01: oracle substantivo e testes adversariais.
- [x] P0-02: reparo estruturado equivalente e rastreado.
- [x] P0-03: limite de turnos comum e congelado.
- [x] P0-04: retrieval compartilhada conforme os fatores declarados.
- [x] P0-05: índice content-addressed com lifecycle, resume e custo.
- [x] P0-06: usage/custo de completion, embedding e rerank.
- [x] P0-07: price contract com cobertura, fontes e tiers.
- [x] P0-08: morte abrupta não repete atividade paga nem perde outcome terminal.
- [x] Matriz de condições e D01–D25 sem issues.
- [x] Compilador, prepare/continue, preflight, power resume e readiness agregado.

### Insumos pré-confirmatórios

- [x] 60 casos de calibração compilados e estruturalmente válidos.
- [x] auditoria humana H-01 do corpus de calibração.
- [ ] candidato não OpenAI aprovado pelo IC de calibração.
- [ ] power config justificado/aprovado e power result reproduzido byte a byte.
- [ ] corpus confirmatório com exatamente `nFinal`, balanceado e auditado.
- [ ] snapshot de preços vigente e custo máximo aprovado.
- [ ] freeze criado uma vez com todos os hashes de inputs e approvals.

### Ambiente

- [x] imagem OCI local atualizada por digest e teste R verde no sandbox.
- [ ] imagem publicada por digest se a execução ocorrer em outro host.
- [ ] `OPENROUTER_API_KEY` disponível somente no ambiente oficial.
- [ ] root externo novo com a capacidade aprovada.
- [ ] commit oficial limpo com todos os gates verdes.
- [ ] `--check-readiness ... --stage prepare` retorna `stageReady: true`.
- [ ] depois do power/corpus confirmatório, readiness de continue retorna
      `studyReady: true`.

Portanto, o repositório está **GO para revisão e preparação dos insumos**, mas o
estudo oficial pago permanece **NO-GO** nesta auditoria.

## 6. Fluxo operacional

Validações sem custo:

```sh
npx nx sync --check
npx nx run mosaic-benchmark:schemas
node benchmarks/mosaic/scripts/cases.mjs validate \
  --calibration-cases benchmarks/mosaic/cases/calibration-cases.v1.json
node benchmarks/mosaic/scripts/study.mjs \
  --validate-config /path/study.prepare.json --stage prepare
node benchmarks/mosaic/scripts/study.mjs \
  --check-readiness /path/study.prepare.json --stage prepare
```

Depois que o config de preparação estiver `stageReady: true`, carregue a
credencial pelo mecanismo de secrets e execute:

```sh
node benchmarks/mosaic/scripts/study.mjs \
  --prepare /path/study.prepare.json --yes-paid-study
```

O comando para, de forma deliberada, depois do power. Após autorar e auditar
exatamente `nFinal` casos confirmatórios:

```sh
node benchmarks/mosaic/scripts/study.mjs \
  --check-readiness /path/study.continue.json --stage continue
node benchmarks/mosaic/scripts/study.mjs \
  --continue /path/study.continue.json --yes-paid-study
```

Repetir o mesmo comando retoma o mesmo root sem reutilizar tentativas nem
sobrescrever artefatos imutáveis.

## 7. Etapas P2 depois do resultado computacional

- **P2-01:** executar oracles apenas para failures elegíveis, em root separado.
- **P2-02:** escolher A1–A5 se fizerem parte do relatório diagnóstico.
- **P2-03:** executar os seis smokes Doric opt-in fora do estudo.
- **P2-04:** materializar e cegar 20% das entregas de M1/baseline.
- **P2-05:** fazer duas avaliações humanas com intervalo mínimo de 14 dias.
- **P2-06:** se usado, implementar e medir o judge auxiliar; não extrapolar se a
  estabilidade humana for menor que 0,80.
- **P2-07:** auditar secrets/PII, hashes e licenças antes do pacote público.

## 8. Arquivos de autoridade relacionados

- `GROUNDING.md`: contrato de validade do repositório.
- `benchmarks/mosaic/protocol/D01-D25.md`: gates do harness.
- `benchmarks/mosaic/src/schemas` e `schemas/v1`: contratos fonte e gerados.
- `benchmarks/mosaic/src/study`: compilação, evidência e provenance.
- `benchmarks/mosaic/src/conditions`: condições e controles compartilhados.
- `benchmarks/mosaic/src/runtime`: reservas, traces, records e resume.
- `benchmarks/mosaic/src/cli`: provider, índice, preços, metering e workflow.
- `benchmarks/mosaic/scripts/study*.mjs`: orquestração oficial.
- `benchmarks/mosaic/analysis` e `container`: protocolo R congelado.
- `benchmarks/mosaic/STUDY.md`: manual operacional.

Mudanças futuras em schema, estimand, provider, runtime ou provenance exigem a
atualização coordenada de `GROUNDING.md`, schemas gerados, testes, hashes e
deste registro.

## 9. Evidência desta auditoria

Em 2026-08-09:

- o corpus oficial passou a validação standalone com 60 casos e zero issues;
- a suíte integrada de `mosaic-benchmark` passou 148 testes TypeScript e 7
  testes MJS;
- os testes incluem `SIGKILL`, resume conservador do índice, metering, price
  tiers, reparo compartilhado, retrieval compartilhada e readiness agregado;
- `npx nx sync --check`, geração de 10 JSON Schemas e `git diff --check`
  passaram durante a integração;
- a imagem R local
  `sha256:204cfae565a61b0423ca5e5393eb3b867823b77c863f740d50dbf42907f1f198`
  passou o smoke sem rede e reproduziu os hashes dos 18 arquivos de análise;
- nenhuma chamada de modelo, probe pago ou execução oficial foi feita nesta
  auditoria.

As contagens finais dos targets `agent`, `mosaic` e `mosaic-benchmark`, além do
estado operacional de Docker/credencial/commit, devem ser registradas pelo gate
do config oficial e não presumidas a partir deste worktree de desenvolvimento.
