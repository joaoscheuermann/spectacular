# MOSAIC: pendências para um estudo válido

Este documento é o registro de prontidão do benchmark empírico MOSAIC. Ele
descreve o que ainda precisa ser implementado, decidido, produzido e
verificado antes de iniciar um estudo oficial pago.

Status da auditoria: **2026-08-08 — NO-GO**.

Enquanto qualquer item **P0** estiver aberto, não execute
`scripts/study.mjs` com `--yes-paid-study`. Os testes atuais provam contratos
importantes do instrumento, mas não cobrem os bloqueios de validade listados
abaixo.

## 1. O que significa "válido"

Há três marcos diferentes:

1. **Execução técnica válida:** cada chamada paga pertence a uma tentativa
   imutável, possui trace verificável, custo correto e record terminal, inclusive
   depois de interrupção abrupta.
2. **Estudo confirmatório válido:** além do item anterior, o outcome mede o
   pedido de forma determinística e neutra, as condições diferem somente pelos
   fatores declarados e todos os artefatos pré-confirmatórios estão congelados.
3. **Estudo publicável completo:** além dos dois itens anteriores, a revisão
   humana cega, diagnósticos escolhidos, relatórios e pacote auditado foram
   concluídos.

O código atual ainda não satisfaz os dois primeiros marcos. O terceiro sempre
inclui etapas humanas deliberadas.

### Legenda

- **P0:** bloqueia uma execução oficial porque pode alterar o estimand,
  outcome, custo ou provenance.
- **P1:** precisa ser fechado para o fluxo oficial ser seguro e executável sem
  improvisação, mas possui contorno manual auditável.
- **P2:** diagnóstico, resultado secundário ou etapa de publicação; não entra no
  contraste primário.
- **H:** decisão ou auditoria humana; não deve ser "automatizada" como se fosse
  uma certificação semântica.
- **O:** requisito operacional, não uma mudança de código.

## 2. O que já está implementado

O instrumento já possui:

- projeto Nx privado, schemas V1 e JSON Schemas versionados;
- 60 micro-skills, 24 tools determinísticas e `World` isolado por run;
- condições B0–B3, M0/M1, A1–A5 e oracles;
- observer aguardado e sanitizado em `agent` e `mosaic`;
- traces append-only, encadeados por hash, e records por tentativa;
- scoring derivado do trace, validação de grids congelados e eligibility;
- seleção determinística do baseline, calibração, power, freeze e análise R
  duas vezes em container sem rede;
- review cego, empacotamento explícito e gates D01–D25;
- orquestrador retomável para o caminho computacional atualmente suportado.

Esses componentes são base reaproveitável. A existência deles não elimina os
confounds e as lacunas de recuperação abaixo.

## 3. Bloqueios P0 de implementação

### P0-01 — Substituir o oracle de entrega por evidência semântica substantiva

**Estado atual**

`src/study/evidence.ts` considera a entrega correta quando todo texto em
`gold.expectedDelivery.contains` aparece em `delivery.markdown`, nas partes ou
nos outputs dos goals. `gold.criteria` não é avaliado. Os casos piloto gerados
por `src/study/cases.ts` revelam no próprio pedido marcadores como
`pilot-NN-complete` e `world-v1` e depois cobram apenas esses marcadores.

**Por que bloqueia**

Um modelo pode ignorar a tarefa, ecoar os dois marcadores e passar o componente
de delivery. Em casos sem tools, isso pode bastar para sucesso end-to-end. O
outcome primário, portanto, ainda não prova que o pedido foi concluído.

**Implementação necessária**

- evoluir `ExpectedDeliveryV1` para representar fatos, campos ou artefatos
  canônicos verificáveis, e não somente substrings;
- ligar cada critério de `gold.criteria` a evidência determinística observável;
- derivar expectativas de fixtures e outputs determinísticos sem revelar uma
  senha de sucesso no pedido;
- manter o oracle neutro à arquitetura: skills, bundle e revisão permanecem
  métricas diagnósticas, não requisitos de sucesso;
- migrar casos, schemas gerados, scorer, fixtures e hashes de provenance;
- auditar o catálogo/corpus antes de observar qualquer resultado pago.

**Prova de aceite**

- ecoar os marcadores atuais falha;
- omitir ou errar um fato solicitado falha;
- produzir artefato, destinatário, valor ou estado incorreto falha;
- a resposta canônica correta passa em todas as seis classes;
- B0–B3 e M1 são julgados pela mesma evidência de tarefa.

### P0-02 — Igualar a política de reparo estruturado

**Estado atual**

MOSAIC executa pelo runtime de `packages/agent`, cujo limite padrão permite
reparos de structured output/tool call. Os baselines chamam `provider.complete`
diretamente em `src/conditions/provider.ts`; `callModel` registra somente a
tentativa 1 e encerra em `invalid_structured_output`.

**Por que bloqueia**

Uma condição pode recuperar uma submissão inválida e outra não, embora
"política de reparo" não seja um fator declarado. Isso confunde o efeito da
arquitetura com tolerância diferente a erro de formato.

**Implementação necessária**

- usar uma política de reparo compartilhada e congelada para B0–B3, M0 e M1,
  ou registrar explicitamente a política como fator experimental;
- emitir uma tentativa estruturada para cada submissão, com feedback e
  esgotamento corretamente registrados;
- contabilizar cada nova chamada e seu custo no budget do run;
- evitar incluir payload rejeitado, reasoning ou erro bruto no trace.

**Prova de aceite**

Testes equivalentes de aceitação imediata, rejeição seguida de reparo e
esgotamento precisam produzir a mesma regra de chamadas em todas as condições
primárias.

### P0-03 — Igualar ou declarar o limite de turnos

**Estado atual**

`src/conditions/baselines.ts` limita baselines a 8 turnos. A composição de
produção em `src/cli/run.ts` configura MOSAIC com 16 turnos. Esse limite não
faz parte de `ConditionFactorsV1`.

**Por que bloqueia**

O tratamento recebe o dobro do budget de interação sem que isso apareça na
matriz de condições.

**Implementação necessária**

Escolher e congelar um limite comum ou declarar formalmente o limite como
fator. O valor efetivo deve aparecer na configuração/provenance do run e na
matriz de conformance.

**Prova de aceite**

Um teste deve demonstrar, por condition ID, o mesmo limite ou exatamente a
diferença registrada no contrato congelado.

### P0-04 — Tornar a retrieval dos baselines fiel ao fator declarado

**Estado atual**

B1–B3 usam `rankSkills()` em `src/conditions/baselines.ts`, um ranking local por
sobreposição de tokens. M1 usa retrieval híbrida com embeddings Voyage,
reranking e seleção. Mesmo assim, os contratos de B1–B3 declaram retrieval
`top-k` body-aware, sem registrar "token overlap" como diferença.

**Por que bloqueia**

Qualidade de retrieval, e não apenas decomposição/bundle/menu/feedback, pode
explicar a diferença entre condições.

**Implementação necessária**

- injetar uma fronteira de retrieval comum;
- B1 deve recuperar pelo pedido inteiro e selecionar exatamente uma skill;
- B2 deve recuperar por subtarefa e selecionar exatamente uma skill;
- B3 deve recuperar/reranquear por objetivo e materializar top-3 fixo, sem
  feedback P0→P1;
- M1 continua com seleção seletiva, que é o fator declarado;
- registrar o mesmo K, candidatos e provenance em todas as condições
  aplicáveis.

**Prova de aceite**

Goldens devem mostrar que, para a mesma query/corpus/modelos, candidatos e
ranking de entrada são idênticos; somente o fator declarado pode alterar o
bundle final.

### P0-05 — Colocar a indexação paga no lifecycle auditável

**Estado atual**

`src/cli/run.ts` chama `buildProductionRetrievers()` antes de criar/executar
qualquer tentativa. `src/cli/retrievers.ts` embeda as 60 skills em duas views,
totalizando 120 requests pagas por invocação de `run`. Em seguida,
`meter.reset()` descarta esse uso. O orquestrador invoca `run` no piloto, nas
duas metades da calibração, no primário, na replicação e na sensibilidade.

**Por que bloqueia**

Uma falha durante indexação pode repetir chamadas pagas sem tentativa, trace
ou record. O custo desaparece. Os bytes exatos do índice usado também não são
um artefato congelado.

**Implementação necessária**

Preferencialmente:

- construir uma vez um índice imutável e content-addressed;
- ligar seu hash ao catálogo, texto indexado, embedder, dimensão e algoritmo;
- gravar lifecycle, uso, custo e recovery dessa construção;
- congelar o hash do índice e recusar bytes divergentes;
- decidir previamente se o custo de setup será reportado separadamente ou
  amortizado, sem descartá-lo;
- manter queries e reranks de cada run dentro da medição daquele run.

**Prova de aceite**

Testar interrupção e resume em cada fronteira da construção, mudança de um
byte do corpus/modelo/dimensão e reuso do índice sem nova chamada paga.

### P0-06 — Medir tokens e custo de embedding/rerank

**Estado atual**

`src/cli/provider.ts` usa metadata de tokens para completion/stream. Para
`embedding()` e `rerank()`, acrescenta somente `perRequest`. Preços reais desses
modelos podem ser por token processado; com `perRequest: 0`, o custo gravado é
zero.

**Por que bloqueia**

Os custos descritivos ficam errados e, depois que P0-04 compartilhar retrieval
com os baselines, o tie-break por custo do piloto também pode mudar.

**Implementação necessária**

- preservar usage autenticado de embedding e rerank no contrato do provider,
  ou implementar uma fronteira de medição reproduzível equivalente;
- distinguir tokens de entrada, itens/documentos reranqueados e requests;
- aplicar a unidade correta do snapshot de preço;
- somar uso por run e uso de setup separadamente;
- atualizar schemas, provider OpenRouter, meter, fixtures e goldens.

Se o contrato público de provider mudar, `GROUNDING.md` também deve ser
atualizado.

**Prova de aceite**

Goldens com usage conhecido precisam reproduzir custo exato, inclusive cache,
embedding, rerank, falha parcial e zero requests.

### P0-07 — Endurecer o contrato de preços

**Estado atual**

`src/cli/pricing.ts` fornece zero como default para os componentes de preço,
aceita modelos totalmente gratuitos e possui uma única URL `source`. O schema
não representa tiers de contexto nem prova qual tier se aplicou.

**Por que bloqueia**

Um arquivo com zeros ou preço inadequado ao tamanho do request passa no gate e
pode contaminar custo, tie-break e provenance.

**Implementação necessária**

- rejeitar modelo ativo sem uma unidade de preço positiva/aplicável;
- registrar fonte e timestamp por modelo, ou um snapshot composto auditável;
- representar tiers/limiares ou provar e registrar que nenhum request os
  ultrapassou;
- validar, antes de qualquer chamada paga, primary, candidate, embedder e
  reranker;
- congelar os bytes exatos e incluir testes de cada unidade de cobrança.

**Prova de aceite**

Zero/ausente, moeda errada, tier incompatível e fonte incompleta falham; um
snapshot completo reproduz o custo calculado para fixtures conhecidas.

### P0-08 — Recuperar interrupção real depois da primeira chamada

**Estado atual**

`executeRun()` persiste corretamente erros lançados e capturados dentro do
processo. Porém, ele calcula `attempt` pela quantidade de records existentes e
só grava o record no fim. Um `SIGKILL`, queda do host ou falta de energia depois
de um evento/chamada paga e antes de `records.append()` deixa trace órfão. No
resume, a tentativa volta a ser 1 e a chamada pode ser repetida.

Os testes atuais de crash injetam exceções que o mesmo processo captura; eles
não simulam morte abrupta.

**Por que bloqueia**

Isso viola a regra de que qualquer interrupção depois da primeira chamada
permanece como outcome de infraestrutura na análise e nunca é repetida.

**Implementação necessária**

- reservar a tentativa de forma imutável antes da primeira atividade paga;
- usar WAL/lease/marker recuperável para distinguir tentativa em curso;
- no resume, reconciliar trace órfão em record terminal de infraestrutura ou
  parar para recuperação auditada;
- nunca reutilizar o número da tentativa;
- preservar a regra de no máximo um retry quando toda falha ocorreu antes de
  atividade paga.

**Prova de aceite**

Testes em processo filho devem matar o runner antes da preparação, depois da
primeira chamada, depois de tool, antes do terminal, depois do trace derivado e
antes do append do record. O resume não pode repetir atividade paga nos casos
terminais.

## 4. Pendências P1 do fluxo oficial

### P1-01 — Implementar um compilador de casos

Hoje não existe comando suportado para "gerar casos de calibração". O autor
precisa escrever diretamente `CaseV1`, inclusive `evidenceHash`, `worldHash`,
listas de tools e `contentHash`. O materializador do piloto é uma referência,
não um gerador para copiar famílias.

Implementar `scripts/cases.mjs` ou um subcomando equivalente que:

- leia um draft semântico simples e estrito;
- execute a sequência de tools contra `World` isolado;
- derive `toolEvidence`, `evidenceHash`, `worldHash`, effects e `contentHash`;
- valide `CaseV1`, balanceamento, IDs, families e isolamento entre fases;
- produza relatório completo de issues, não somente o primeiro erro;
- grave output com exclusão mútua, sem sobrescrever;
- não faça chamadas de modelo.

O compilador não pode declarar sozinho que o texto é semanticamente neutro ou
independente. Essa continua sendo uma auditoria humana.

### P1-02 — Dividir o orquestrador em preparação e continuação

O config atual exige `confirmatoryCases` antes de iniciar. Entretanto, o
`nFinal` exato só existe depois do power, que o próprio script executa após
piloto e calibração.

Implementar dois modos imutáveis:

1. `prepare`/`--through power`: gates, piloto, calibração e power; publica o
   `nFinal` requerido e para antes do freeze;
2. `continue`: fixa o corpus confirmatório auditado, valida exatamente
   `nFinal`, cria o freeze e continua as famílias.

Receipts e inputs compartilhados precisam continuar content-addressed. Mudar
config/corpus deve exigir um novo study root, nunca editar receipts existentes.

### P1-03 — Adicionar validação standalone de casos de calibração

`--validate-config` valida somente o envelope e a existência dos arquivos. O
comando `validate --cases` trata o corpus como confirmatório. A validação de
calibração completa só ocorre no preflight da execução paga.

Adicionar `validate --calibration-cases` ou integrar a checagem ao compilador,
incluindo:

- 60 casos/families únicos;
- exatamente 15 por domínio e 10 por classe;
- `phase: "calibration"`;
- referências, hashes e execução esperada válidos;
- isolamento do piloto e, quando disponível, do confirmatório;
- todas as issues em JSON.

### P1-04 — Validar capacidades dos modelos antes do piloto

O runner não usa `validateModel()` nem verifica antecipadamente se Luna, o
candidato, o embedder e o reranker suportam o contrato congelado. Uma
incompatibilidade pode aparecer somente depois que o piloto começou.

Implementar preflight de metadata/capability e, quando metadata não bastar, um
probe mínimo, explicitamente aprovado e contabilizado. Verificar:

- IDs dos quatro modelos e disponibilidade pelo transporte escolhido;
- effort `medium` para completions;
- structured output e tools;
- embedding com 2.048 dimensões;
- rerank no formato usado;
- usage necessário para metering.

O probe não pode ser escondido em um gate declarado "sem custo".

### P1-05 — Congelar a semântica de `seed`

O scheduler gera a mesma seed por `(case, repetition)` e randomiza a ordem das
condições. A request do provider não recebe essa seed; portanto ela não controla
a amostragem do modelo principal.

Escolher e documentar uma das opções:

- adicionar seed provider-neutral e passá-la onde houver suporte, registrando
  provedores que a ignoram; ou
- declarar que a seed cobre somente schedule, ordem e hooks determinísticos,
  enquanto o pareamento estatístico é por caso/repetição.

Os nomes dos campos, D20, freeze e manual devem dizer a mesma coisa.

### P1-06 — Tornar power operacionalmente auditável

O arquivo de power aceita `baselineProbability` e `randomInterceptSd`, mas o
orquestrador não deriva nem registra a justificativa dessas escolhas a partir
do piloto. Além disso, 10.000 simulações com diversos tamanhos e GLMER podem
demorar muitas horas.

Antes do estudo:

- registrar um artefato de rationale/approval para os parâmetros; ou
- implementar uma proposta reproduzível derivada do piloto, que ainda exige
  aprovação estatística;
- estimar tempo/recursos antes de iniciar;
- adicionar checkpoint/resume do power sem alterar a seed ou a definição
  congelada do menor tamanho elegível.

### P1-07 — Criar um gate agregado de prontidão

`--validate-config` retorna `ok: true` para um envelope bem formado cujos
arquivos apenas existem. Isso não significa "study ready".

Adicionar `--check-readiness` que execute somente checagens sem custo e retorne
todas as pendências classificadas, incluindo schemas, casos, preços, imagem,
Git, hashes, capacidade verificável sem probe e espaço em disco. O output deve
distinguir `envelopeValid` de `studyReady`.

## 5. Insumos e gates humanos obrigatórios

Estes itens não são bugs e não podem ser inventados pelo runner.

### H-01 — Corpus de calibração

Depois de P0-01 e P1-01:

- autorar exatamente 60 casos em inglês;
- manter 15 por domínio e 10 por classe A–F;
- usar IDs e families novos, `phase: "calibration"` e fixtures auditáveis;
- não copiar texto, gold ou families do piloto;
- revisar equivalentes procedurais, overlaps, distratores e conflitos;
- auditar manualmente neutralidade, dificuldade e ausência de marcador de
  resposta.

### H-02 — Candidato de replicação

Luna já é o modelo primário congelado; ele não é configurado no JSON do
orquestrador. `candidate` precisa ser uma família **não OpenAI** para a
replicação. Qwen é o primeiro candidato, não uma garantia de aprovação.

O freeze só pode continuar se, em 60 casos e três repetições M1 pareadas, o IC
bootstrap de 95% da diferença para Luna estiver em `[-0,05, +0,05]`. Reprovar
esse gate é um resultado científico esperado; testar outro candidato requer
decisão documentada e, preferencialmente, novo root/config imutável.

### H-03 — Parâmetros de power

Revisar e justificar antes do freeze:

- probabilidade de sucesso do baseline;
- desvio-padrão do intercepto aleatório;
- classes adaptativas do contraste;
- efeito mínimo relevante de 10 pontos percentuais;
- alpha bilateral de 0,05, 10.000 simulações e seed;
- regra `N_final = max(240, N_power)`, arredondada ao próximo múltiplo de 120.

### H-04 — Corpus confirmatório

Somente depois do power:

- autorar exatamente `nFinal` families independentes;
- garantir `nFinal >= 240` e múltiplo de 120;
- balancear exatamente a matriz 6 classes × 4 domínios;
- usar `phase: "confirmatory"`;
- isolar de piloto e calibração por family, fingerprint e auditoria semântica;
- congelar antes de observar qualquer resultado confirmatório.

### H-05 — Snapshot de preços

Depois de P0-06/P0-07, capturar os preços vigentes dos quatro modelos,
unidades, tiers, timestamp e fontes. Não usar zeros, valores ilustrativos ou
estimativas sem provenance.

### H-06 — Autorização de custo

No menor desenho permitido (`nFinal = 240`), o orquestrador agenda:

- piloto: `60 × 6 × 5 = 1.800` runs;
- calibração: `60 × 3 × 2 = 360` runs;
- primário: `240 × 2 × 5 = 2.400` runs;
- replicação: `2.400` runs;
- sensibilidade: `2.400` runs.

Total mínimo: **9.360 runs**, fora probes, indexação, ablações, oracles e
judge auxiliar. Cada run pode conter várias chamadas. Estimar o custo depois
que P0-05–P0-07 estiverem fechados, garantir crédito/limites e obter aprovação
explícita.

## 6. Requisitos operacionais

### O-01 — Repositório congelado

- todos os P0/P1 necessários implementados e revisados;
- worktree limpo;
- commit exato registrado;
- `npx nx sync`, projetos e targets verdes;
- `git diff --check` verde.

O worktree atual precisa estar limpo antes do estudo oficial. Não descarte nem
misture mudanças de `agents/doric` para satisfazer esse gate.

### O-02 — Imagem R imutável

- construir a imagem com base fixada por digest;
- restaurar `renv.lock` sem rede durante a análise;
- executar `analysis/test-analysis.R` no sandbox do container;
- publicar ou disponibilizar a imagem como
  `repository@sha256:<64-hex>`;
- garantir que o conteúdo de `/benchmark/analysis` e `renv.lock` corresponde ao
  commit congelado.

`Rscript` no host não é necessário. Docker executa o R congelado.

### O-03 — Ambiente de execução

- Node/dependências do workspace instalados;
- Docker disponível e com CPU/memória/disco suficientes;
- root absoluto fora do repositório e com espaço para milhares de eventos;
- rede disponível somente para as chamadas pagas;
- `OPENROUTER_API_KEY` carregada em runtime pelo secret manager;
- nenhuma credencial em config, manifest, trace ou receipt.

### O-04 — Config e arquivos finais

O config oficial deve apontar para:

- imagem R por digest;
- snapshot de preços validado;
- corpus de calibração auditado;
- corpus confirmatório com o `nFinal` calculado;
- power config aprovado;
- seeds nomeadas e `frozenAt` escolhido no momento correto;
- candidate não OpenAI.

Arquivos em `tmp/` são rascunhos ignorados pelo Git, não artefatos oficiais.
O orquestrador deve copiar os bytes finais uma vez para o root externo.

## 7. Etapas P2 depois do resultado computacional

Estas lacunas não mudam o contraste primário, mas precisam ser resolvidas para
declarar o protocolo completo ou publicar:

- **P2-01:** gerar schedules dos oracles somente para failures elegíveis e
  executá-los em artifact root separado;
- **P2-02:** escolher e executar A1–A5 se a análise diagnóstica fizer parte do
  relatório; `study.mjs` não os agenda hoje;
- **P2-03:** executar os seis smokes Doric opt-in quando o composition root
  estiver estabilizado, sempre fora do estudo;
- **P2-04:** materializar 20% das entregas de M1/baseline para uma repetição
  determinística, cegar condição/modelo e preparar assignments;
- **P2-05:** realizar duas avaliações pelo pesquisador solo com intervalo mínimo
  de 14 dias e reportar estabilidade intra-avaliador;
- **P2-06:** se o judge auxiliar GPT-5.6 Sol for usado, implementar sua chamada,
  custo e artefatos cegados. Ele é opcional e não pode ser extrapolado se a
  estabilidade humana for menor que 0,80;
- **P2-07:** criar um package plan explícito, conferir secrets/PII, hashes,
  licenças e empacotar somente os arquivos auditados.

## 8. Gate final de aceite

Um estudo pode mudar de **NO-GO** para **GO** somente quando houver evidência
para todos os itens abaixo.

### Instrumento

- [ ] P0-01: oracle semântico substantivo e testes adversariais.
- [ ] P0-02: reparo estruturado equivalente e rastreado.
- [ ] P0-03: limite de turnos equivalente ou fator congelado.
- [ ] P0-04: retrieval compartilhada conforme os fatores declarados.
- [ ] P0-05: índice content-addressed com lifecycle/resume/custo.
- [ ] P0-06: usage e custo exatos para completion, embedding e rerank.
- [ ] P0-07: price contract rejeita dados inaptos e trata tiers.
- [ ] P0-08: morte abrupta não repete chamada paga nem perde record terminal.
- [ ] Matriz de condições prova que toda diferença restante é declarada.
- [ ] D01–D25 e novos testes de readiness passam.

### Insumos pré-confirmatórios

- [ ] 60 casos de calibração compilados, válidos e auditados.
- [ ] candidato não OpenAI aprovado pelo IC de calibração.
- [ ] power config justificado e power result reproduzido byte a byte.
- [ ] corpus confirmatório com exatamente `nFinal`, balanceado e independente.
- [ ] snapshot de preços completo e custo máximo aprovado.
- [ ] prompts, schemas, catálogo, tools, seeds, casos e R ligados por hash.

### Ambiente e freeze

- [ ] imagem OCI correta disponível por digest e teste R verde no sandbox.
- [ ] `OPENROUTER_API_KEY` somente no ambiente.
- [ ] root externo novo, vazio e com capacidade suficiente.
- [ ] commit limpo e todos os gates Nx verdes.
- [ ] freeze criado uma única vez e schedules apontando para seu
      `manifestHash`.
- [ ] dry/readiness check completo retorna `studyReady: true` sem chamada paga.

## 9. Comandos que só devem ser usados depois do GO

Quando todos os itens anteriores estiverem fechados, o fluxo esperado será:

```sh
node benchmarks/mosaic/scripts/study.mjs --validate-config /path/study.json

export OPENROUTER_API_KEY='carregada-do-secret-manager'
node benchmarks/mosaic/scripts/study.mjs \
  --config /path/study.json \
  --yes-paid-study
```

Repetir o segundo comando deve retomar o mesmo study root. Até P0-08 ser
fechado, essa promessa não cobre `SIGKILL`, queda do host ou falta de energia
entre uma chamada paga e o append do record.

## 10. Arquivos de autoridade relacionados

- `GROUNDING.md`: contrato de validade do repositório.
- `benchmarks/mosaic/protocol/D01-D25.md`: gates existentes do harness.
- `benchmarks/mosaic/src/schemas`: contratos V1.
- `benchmarks/mosaic/src/study/evidence.ts`: outcome determinístico atual.
- `benchmarks/mosaic/src/conditions`: implementação das condições.
- `benchmarks/mosaic/src/runtime`: traces, records e resume.
- `benchmarks/mosaic/src/cli`: provider, preços e workflow de baixo nível.
- `benchmarks/mosaic/scripts/study*.mjs`: orquestração oficial atual.
- `benchmarks/mosaic/analysis` e `container`: protocolo R congelado.
- `benchmarks/mosaic/STUDY.md`: manual operacional, aplicável depois do GO.

Sempre que uma pendência acima alterar schema, estimand, provider, runtime ou
provenance, atualize também `GROUNDING.md`, schemas gerados, testes, hashes e
este registro de prontidão.

## 11. Ordem recomendada de implementação

1. Fechar P0-01 e congelar o novo contrato do outcome antes de autorar qualquer
   corpus experimental.
2. Fechar P0-02–P0-04 e provar a paridade das condições.
3. Fechar P0-05–P0-07 em conjunto, pois índice, usage e price contract formam
   uma única cadeia de custo/provenance.
4. Fechar P0-08 e executar testes reais de morte do processo.
5. Implementar P1-01/P1-03; só então compilar e auditar os casos de calibração.
6. Implementar P1-02 e os preflights P1-04–P1-07.
7. Regenerar schemas, fixtures, cases piloto, hashes e imagem R; repetir todos
   os gates em um commit limpo.
8. Produzir os insumos humanos, executar a fase de preparação até power,
   autorar o confirmatório e somente então criar o freeze.

## 12. Evidência observada nesta auditoria

Em 2026-08-08, antes de implementar as pendências:

- `npx nx sync --check` informou que o workspace estava atualizado;
- typecheck, test e build de `agent`, `mosaic` e `mosaic-benchmark` passaram;
- passaram 50 testes de Agent, 105 de MOSAIC, 114 testes TypeScript do benchmark
  e 6 testes MJS do orquestrador;
- `mosaic-benchmark validate` e `conformance` retornaram `valid: true` e nenhuma
  issue;
- a imagem local
  `mosaic-analysis@sha256:8e3183b67c69a237c7a1c940101922762577cf9d084741bab4353c8d1fc37aa8`
  passou `analysis/test-analysis.R` sem rede;
- os hashes dos 18 arquivos em `/benchmark/analysis` nessa imagem eram
  idênticos aos arquivos locais;
- o worktree não estava limpo, portanto O-01 permanecia aberto.

Essa evidência confirma os contratos já cobertos. Ela não testa eco de
marcadores, paridade de repair/turn/retrieval, custo de embedding/rerank,
lifecycle do índice nem morte abrupta entre chamada paga e record; por isso o
status continua **NO-GO**.
