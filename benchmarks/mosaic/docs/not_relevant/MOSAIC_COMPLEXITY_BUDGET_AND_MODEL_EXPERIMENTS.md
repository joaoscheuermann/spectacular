# Orçamento de complexidade e experimentos de modelo do MOSAIC

## Objetivo

Este documento registra a decisão de evitar que reparos pontuais transformem o
MOSAIC em um segundo planner ou verifier genérico. Ele complementa a
[análise forense do run](SKILLSBENCH_MOSAIC_FAILURE_ANALYSIS_58C79CD7.md), que
classificou 13 das 52 falhas pontuadas como casos em que planejamento, hints,
revisão ou topologia do MOSAIC contribuíram materialmente.

A meta não é fazer o runtime resolver qualquer tarefa. A meta é impedir que a
orquestração:

- enfraqueça o pedido original;
- esconda uma falha observável;
- desperdice recuperação com revisões sem progresso;
- amplifique um bloqueio local desnecessariamente.

Produzir o artefato correto, escolher o algoritmo apropriado e interpretar
semântica de domínio continuam sendo responsabilidades do modelo executor.

## Princípio de projeto

> O MOSAIC deve impedir que sua própria orquestração piore a execução, não
> tentar entender e validar qualquer tarefa do mundo.

Há três resultados diferentes que não devem ser confundidos:

1. **Evitar regressão causada pelo MOSAIC:** em grande parte controlável pelo
   runtime.
2. **Detectar um resultado incorreto:** determinístico apenas quando o pedido
   oferece um contrato observável.
3. **Produzir a correção correta:** ainda depende do modelo em todos os 13
   casos analisados.

Uma mudança de runtime não deve ser justificada apenas porque poderia ter
ajudado uma tarefa. Ela deve corrigir uma invariante geral, ocorrer em mais de
um caso independente e apresentar baixo risco de impedir execuções válidas.

## Orçamento de complexidade

O primeiro ciclo de reparos deve obedecer aos seguintes limites:

- nenhuma mudança de API ou schema público;
- nenhum novo provider call;
- nenhum novo estado da máquina de estados;
- nenhuma nova dependência;
- nenhuma autoridade sobre o verifier privado;
- lógica local, removível e coberta por testes unitários;
- promoção ao core somente após replay e experimento controlado.

Se uma solução ultrapassar esses limites, ela começa como experimento do
harness ou análise offline. A exceção exige evidência de benefício recorrente e
uma decisão explícita de ampliar o contrato descrito em `GROUNDING.md`.

## Mudanças mínimas candidatas ao core

Somente três defesas estão inicialmente aprovadas para exploração como
comportamento de produto.

### 1. Revisão exatamente no-op não consome orçamento

Comparar os campos de planejamento antes de anexar um snapshot localizado:
`id`, `goal`, `doneWhen`, `dependsOn` e `deliver`. Uma resposta que não altera
nenhum campo da região revisável é rejeitada antes de incrementar a revisão.

Isso elimina desperdício mecânico observado em `drone-planning-control` sem
pretender decidir se duas paráfrases são semanticamente equivalentes.

### 2. Handoff curto após revisão

O executor seguinte deve receber, a partir do snapshot histórico já existente:

- a premissa invalidada;
- o efeito solicitado;
- os critérios que permaneciam falsos;
- as observações relevantes, marcadas como contexto não citável.

O novo executor ainda precisa produzir observações atuais para comprovar a
correção. O handoff reduz redescoberta e não reautoriza IDs de snapshots
anteriores.

### 3. Coerência mínima de `blocked`

Uma decisão `blocked` com todos os critérios marcados como satisfeitos é
internamente contraditória e deve ser rejeitada. Pelo menos um critério precisa
estar falso.

Não se deve exigir uma observação local em todo bloqueio: impossibilidades
lógicas podem ser demonstradas sem uma ferramenta.

Essas três mudanças reforçam contratos já existentes. Elas não verificam a
qualidade do artefato nem encontram a correção.

## Experimentos fora do core

Os mecanismos abaixo devem começar em replay offline ou em **shadow mode**:
eles registram o que teriam rejeitado, mas não alteram a execução.

| Experimento             | Sinal procurado                                                                            | Motivo para não bloquear ainda                                 |
| ----------------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------- |
| Drift de contrato       | Threshold, nome literal ou sucesso obrigatório enfraquecido entre pedido, P0, P1 e revisão | Equivalência semântica produz falsos positivos                 |
| Provenance de hard gate | Critério obrigatório introduzido apenas por uma skill ou hint                              | Nem todo requisito legítimo aparece literalmente no pedido     |
| Capability preflight    | Browser, executável ou permissão assumidos, mas indisponíveis                              | Disponibilidade não determina se a capacidade é obrigatória    |
| Validador observável    | Exit code, schema fechado, nome de sheet ou término de processo contradiz `completed`      | Associação entre observação e requisito ainda pode ser ambígua |

O resultado do shadow mode deve registrar somente classificação estrutural e
contagens seguras. Não deve expor verifier, prompts, payloads sensíveis ou
diagnósticos que o runtime atual não autoriza.

## Soluções adiadas

As soluções abaixo não entram no primeiro ciclo:

- Contract Atom Ledger semântico completo;
- segundo modelo crítico como hard gate;
- framework genérico de validadores;
- tracking de ownership e digest de artefatos;
- dependências tipadas como `required` e `advisory`;
- novo estado de salvage revision;
- interpretação de efeitos de comandos shell;
- validação genérica de algoritmos, planilhas, áudio, cenas ou políticas de
  domínio.

Essas ideias podem voltar à avaliação se os experimentos mostrarem um padrão
persistente que as três defesas mínimas não cobrem. Rastreamento de artefatos,
em especial, exige nova autoridade do host: hoje o core preserva referências
opacas e deliberadamente não resolve paths, existência ou conteúdo.

Também não são soluções adequadas:

- aumentar indiscriminadamente `maxTurns` ou `revision.max`;
- entregar o verifier privado ao agente;
- tratar um segundo judge como fonte infalível;
- fazer hash de todo o workspace;
- inferir arquivos modificados analisando texto de shell;
- tornar todas as skills meramente informativas.

## Experimento com modelos mais potentes

O primeiro teste deve alterar o modelo sem alterar o MOSAIC. Isso mede quanto
das 13 falhas desaparece apenas com maior capacidade de planejamento e
execução.

### Hipóteses

- **H1 — capacidade de execução:** um executor mais forte reduz casos M1 e os
  erros concomitantes presentes nos M2.
- **H2 — capacidade de planejamento:** um planner/revisor mais forte reduz
  erosão de contrato, promoção incorreta de hints e revisões sem progresso.
- **H3 — falha estrutural:** mecanismos M2 que persistem em modelos mais fortes
  são candidatos melhores a uma invariante do runtime.

### Variáveis controladas

Em cada comparação, manter constantes:

- versão do bundle e do harness;
- tasks, assets e verifier;
- prompt, skills e terminal disponíveis;
- limites de turns e revisões;
- reasoning effort, quando suportado de forma equivalente;
- ordem das tarefas e política de retry;
- braço Direct e braço MOSAIC com a mesma configuração de modelo.

Cada campanha deve ter um ID novo. Não reutilizar ou retomar uma campanha cuja
configuração de modelo, bundle ou runtime seja diferente.

### Etapas

#### Etapa A — modelo, sem reparos de runtime

Executar primeiro uma amostra diagnóstica:

- os 13 casos M2;
- controles entre os 34 sucessos;
- controles representativos entre M1 e M0.

Comparar o baseline atual, `openrouter/openai/gpt-5.6-luna` com esforço `low`,
com cada modelo mais potente. O verifier continua exclusivamente pós-hoc.

#### Etapa B — ablação por função

Como o MOSAIC possui perfis separados, testar somente quando a Etapa A mostrar
ganho material:

| Tratamento         | Planning/revision | Execution    | Pergunta respondida                          |
| ------------------ | ----------------- | ------------ | -------------------------------------------- |
| Baseline           | baseline          | baseline     | Referência atual                             |
| Planejamento forte | modelo forte      | baseline     | O contrato e a topologia melhoram?           |
| Execução forte     | baseline          | modelo forte | O artefato e a recuperação melhoram?         |
| Tudo forte         | modelo forte      | modelo forte | Qual é o teto combinado?                     |
| Direct forte       | não aplicável     | modelo forte | O MOSAIC agrega valor sobre execução direta? |

Não misturar essa ablação com os três guardrails candidatos; caso contrário,
não será possível atribuir o efeito observado.

#### Etapa C — guardrails mínimos

Somente depois de estabelecer o efeito do modelo, repetir a amostra com cada
defesa mínima isolada e, por último, com as três combinadas.

Ampliar para as 87 tarefas apenas se a amostra não mostrar regressão relevante,
explosão de retries ou crescimento desproporcional de custo.

### Métricas

Registrar por tarefa e por tratamento:

- score e status do verifier;
- classificação pós-hoc M0/M1/M2/M3;
- falso `completed` e falso `blocked`;
- quantidade de revisões, incluindo no-ops;
- turns, tokens, duração e custo;
- diferença pareada MOSAIC versus Direct;
- mudança de trajetória mesmo quando o score final não muda.

O score isolado não distingue um artefato melhor de uma orquestração mais
honesta. Por isso, revisão consumida, estado terminal e classe causal continuam
necessários.

## Regras de decisão

Preferir aumento de capacidade do modelo quando ele elimina o padrão sem
introduzir custo ou regressão inaceitáveis. Preferir mudança de runtime quando o
mesmo mecanismo persiste em mais de um modelo forte e a defesa pode ser expressa
como invariante simples.

Uma defesa só deve sair de shadow mode quando:

- cobre pelo menos dois casos independentes;
- não apresenta falsos positivos nos controles bem-sucedidos da amostra;
- não depende de informação do verifier privado;
- não cria novo loop, provider call ou autoridade implícita;
- possui testes unitários de aceitação, rejeição e compatibilidade;
- seu custo e sua complexidade são menores que o problema recorrente evitado.

Se modelos mais potentes resolverem a maior parte dos M2, a decisão correta pode
ser não ampliar o core. Se apenas os erros de execução desaparecerem e a mesma
erosão de contrato continuar, isso fortalece o caso para os guardrails mínimos.

## Estado desta decisão

Este documento registra uma estratégia experimental, não autorização para
implementar todas as soluções enumeradas. O default atual é:

1. testar modelos mais potentes sem mudanças de runtime;
2. executar replay e shadow mode;
3. considerar somente as três defesas mínimas;
4. reavaliar soluções maiores apenas com evidência nova.
