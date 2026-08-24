# Análise das falhas do MOSAIC no SkillsBench

## Escopo

Este documento registra a análise forense do braço MOSAIC da campanha:

```text
skillsbench-run-58c79cd7-08f9-4dac-90b0-01293b6b5542-pvfsC4
```

O job analisado é `2026-08-13__00-48-16`, executado com
`openrouter/openai/gpt-5.6-luna`, esforço `low`, 87 tarefas, 34 sucessos, 52
falhas pontuadas e um timeout do verifier. O score agregado foi 39,1%.

A campanha contém somente o braço `mosaic`. Portanto, as classificações abaixo
são atribuições causais baseadas nas trajetórias, planos, revisões, decisões e
verifiers, não uma estimativa experimental pareada contra o braço Direct.

O diretório de evidências local é:

```text
results/skillsbench-run-58c79cd7-08f9-4dac-90b0-01293b6b5542-pvfsC4/
  mosaic/jobs/2026-08-13__00-48-16/
```

O agregado do job está em
[`summary.json`](../../results/skillsbench-run-58c79cd7-08f9-4dac-90b0-01293b6b5542-pvfsC4/mosaic/jobs/2026-08-13__00-48-16/summary.json).
A fonte oficial das tarefas está fixada no
[`benchflow-ai/skillsbench@b63b7b2`](https://github.com/benchflow-ai/skillsbench/tree/b63b7b2850226b6aa4fb5929a8c1ac7bc4d9a6af/tasks).

## Régua de classificação

| Classe | Definição                                                                                                | Resultado no run |
| ------ | -------------------------------------------------------------------------------------------------------- | ---------------: |
| M3     | Defeito demonstrado do runtime ou de seu contrato: scheduler, ACP, IDs, schemas ou máquina de estados    |                0 |
| M2     | Decisão de planejamento, hints, revisão ou topologia do MOSAIC que contribuiu materialmente para a falha |               13 |
| M1     | Erro imediato do executor que um guardrail semântico melhor poderia ter detectado ou recuperado          |               31 |
| M0     | Falha sem contribuição material do MOSAIC, como algoritmo, ambiente ou verifier                          |                8 |

Esta análise chama de “falha do MOSAIC” somente os 13 casos M2. Doze têm
causalidade forte ou moderada; `lake-warming-attribution` permanece M2
limítrofe, pois a mutação do P1 contribuiu, mas o erro do executor foi dominante.
A mera
aceitação de uma conclusão errada do executor não basta: é necessário que o
MOSAIC tenha alterado o contrato, direcionado o objetivo incorretamente ou
impedido uma recuperação plausível.

## Conclusão executiva

Os 13 casos M2 formam quatro grupos causais:

| Grupo | Mecanismo primário                                                      | Casos |
| ----- | ----------------------------------------------------------------------- | ----: |
| A     | Erosão do contrato de aceitação                                         |     5 |
| B     | Hint ou skill promovido indevidamente a hard gate ou método obrigatório |     3 |
| C     | Corrupção semântica ou expansão indevida do pedido                      |     3 |
| D     | Amplificação de uma falha local pela revisão ou topologia do grafo      |     2 |

Os grupos indicam o primeiro ponto causal dominante, não classes mutuamente
exclusivas. Por exemplo, `fix-build-agentops` combina erosão do gate com captura
por uma skill de `uv`; `setup-fuzzing-py` combina erosão na revisão com
invalidação de artefato upstream; e `multilingual-video-dubbing` combina
hard-gate inflation com prioridade invertida.

Há duas leituras complementares:

- Em 10 casos, o erro entrou no P0/P1; em três, entrou ou se consolidou numa
  revisão localizada.
- Nove casos produziram falso sucesso (`completed` para um resultado
  incompatível); quatro produziram falso bloqueio ou impediram a entrega.

Quanto à força do contrafactual, dez casos são M2 fortes, dois são M2 de
confiança média (`drone-planning-control` e `setup-fuzzing-py`) e um é M2
limítrofe (`lake-warming-attribution`).

A cadeia causal recorrente foi:

```text
pedido original
  -> P0/P1 ou revisão altera o contrato
  -> doneWhen e dependências tornam a alteração normativa
  -> executor otimiza ou valida o objetivo alterado
  -> runtime verifica a forma, mas não a equivalência semântica com o pedido
  -> completed ou blocked incorreto
  -> verifier expõe a divergência
```

## Grupo A — Erosão do contrato de aceitação

### Definição

O plano ou uma revisão substitui uma condição que precisava ser verdadeira por
uma condição mais fraca: registrar o resultado, documentar a falha, executar
uma validação “equivalente” ou fornecer um campo semanticamente parecido. A
tarefa então pode terminar com sucesso sem cumprir o contrato original.

O problema não é apenas o executor mentir sobre um critério. Nesses casos, o
próprio `doneWhen` tornou a conclusão incorreta admissível.

### `adaptive-cruise-control`

**Onde entrou:** P1.

**Mutação do contrato:** os critérios finais passaram a exigir que a evidência
registrasse se cada meta havia passado e que o relatório declarasse
conformidade ou não conformidade. Isso é mais fraco que exigir que as metas de
controle fossem realmente atendidas. O critério de tuning também favoreceu a
inclusão de uma chave adicional `evaluation` no YAML, embora o verifier
exigisse exatamente `pid_speed` e `pid_distance`: o plano exigiu evidência sem
deixar claro que ela pertencia ao relatório, não ao artefato de schema fechado.

**Efeito observado:** os nós foram marcados `completed` mesmo com overshoot e
distance SSE registrados como `false`; o artefato ainda violava o schema
fechado.

**Fator concomitante:** o executor aceitou conscientemente métricas reprovadas
e materializou a chave adicional. O MOSAIC não causou os valores ruins, mas
transformou “medir e relatar” em substituto para “passar”.

**Guardrail mínimo:** critérios quantitativos devem permanecer monotônicos:
“atingir X” não pode virar “registrar se X foi atingido”. Schemas literais do
pedido devem ser preservados como conjuntos fechados de campos, e todo critério
de evidência deve declarar o destino em que será materializado.

### `fix-build-agentops`

**Onde entrou:** P0/P1.

**Mutação do contrato:** a aceitação final permitia “build ou validação
equivalente” e aceitava que uma falha remanescente fosse apenas distinguida.
Isso permitiu usar `uv build`, que valida packaging, como substituto do teste de
reprodução BugSwarm que exercitava o defeito real.

O hint de `uv-package-manager` também deixou de ser uma hipótese investigativa:
P1 introduziu `pyproject.toml`, `uv.lock`, `.python-version`, `uv sync` e
`uv lock` antes de haver evidência de que o gerenciador de pacotes era a causa
do bug.

**Efeito observado:** o nó final terminou `completed` após o build do pacote,
mas o comportamento `init_timestamp == end_timestamp` continuou incorreto.

**Fator concomitante:** o executor concentrou os patches em packaging e não
executou o teste oficial completo.

**Guardrail mínimo:** quando o pedido possui um comando ou reprodução canônica,
ela deve permanecer um gate imutável. “Validação equivalente” só pode ser
aceita se provar a mesma propriedade observável, não apenas outra camada do
projeto. Hints de tecnologia devem permanecer `investigate_if_present` até
serem confirmados pelos arquivos e pela falha real do repositório.

### `fix-build-google-auto`

**Onde entrou:** revisão localizada.

**Mutação do contrato:** depois de uma execução Maven vermelha, a revisão
alterou a aceitação do teste de `auto-common` para “corrigido ou explicitamente
documentado”. A revisão converteu evidência de falha em uma forma admissível de
sucesso.

**Efeito observado:** o nó terminou `completed 6/6` apesar de Maven sair com
código 1, `VisibilityTest` continuar falhando e `auto-factory` não compilar.

**Fator concomitante:** o patch escolhido pelo executor para JDK 11 estava
incorreto. A análise também foi feita sob JDK 11, embora o job BugSwarm e a
configuração histórica usassem JDK 7; o executor acabou corrigindo parcialmente
uma incompatibilidade do ambiente de diagnóstico, não o build canônico.

**Guardrail mínimo:** uma revisão não pode enfraquecer o predicado que originou
`needs_revision`. Um teste vermelho pode mudar a estratégia ou a decomposição,
mas não pode ser reclassificado como sucesso por ter sido documentado.

### `hvac-control`

**Onde entrou:** P0/P1.

**Mutação do contrato:** o pedido exigia as chaves literais `Kp`, `Ki`, `Kd` e
`lambda`; o plano aceitou `lambda` “ou um conjunto completo de ganhos
equivalente”.

**Efeito observado:** a decisão final citou `lambda_seconds` como evidência de
sucesso, mas o verifier rejeitou a ausência da chave literal `lambda`.

**Fator concomitante:** o executor escolheu e serializou o nome alternativo.

**Guardrail mínimo:** nomes de campos, paths, arquivos, comandos e formatos
declarados literalmente devem ser invariantes não parafraseáveis.

### `setup-fuzzing-py`

**Onde entrou:** revisão localizada.

**Mutação do contrato:** o P1 inicial exigia um fuzzing run completo de 10
segundos e evidência de conclusão. Depois do `ImportError` de `minisgl`, a
revisão passou a exigir runs apenas “intended to last 10 seconds”, logs com
`INITED` e ausência de erro antes da inicialização.

**Efeito observado:** o nó foi marcado `completed` com execuções que apenas
iniciaram e terminaram por timeout. O verifier rejeitou os runs incompletos.

Há uma segunda patologia do grafo: `n03` já estava protegido como `completed`,
mas o executor revisado de `n04` modificou os `fuzz.py` que sustentavam essa
conclusão, inclusive movendo imports para fora de `atheris.instrument_imports`.
O MOSAIC não rastreou que a prova upstream havia ficado obsoleta.

**Fator concomitante:** havia também problemas do executor, incluindo path
absoluto em `libraries.txt`, instrumentação e dependências. Portanto, preservar
o gate original impediria o falso sucesso, mas não garantiria score 1. O
ambiente local ainda mascarou a ausência de `torch`, e o verifier exigia
basenames em `libraries.txt` apesar de o pedido usar a palavra “path”.

**Guardrail mínimo:** aplicar um diff semântico de revisão e rejeitar qualquer
mudança de “concluído” para “tentado”, “iniciado”, “documentado” ou
“best-effort”, salvo autorização explícita do pedido.

### Causa sistêmica do grupo A

O prompt de revisão manda preservar intenção, constraints e deliverables, mas
essa invariância é apenas instrucional. O runtime valida que todos os critérios
de um `completed` estejam marcados `true`; ele não verifica se os critérios
continuam semanticamente equivalentes ao pedido nem se a evidência prova sua
verdade.

## Grupo B — Hints e skills promovidos a hard gates ou métodos

### Definição

Uma skill fornece conhecimento útil, mas o P1 transforma esse conhecimento em
uma obrigação normativa não sustentada pelo pedido. O efeito pode ser:

- escolher um método específico entre várias interpretações válidas;
- tornar uma métrica auxiliar mais importante que o resultado do usuário;
- exigir uma capacidade ambiental indisponível como condição de progresso.

### `lake-warming-attribution` — M2 limítrofe

**Onde entrou:** P1 body-aware, após hints das skills.

**Mutação do contrato:** o plano passou de qualquer medida documentada e
suportada pelos dados para uma receita fechada: PCA global, varimax, factor
scores, mapeamento por loadings e decomposição leave-one-out de R². P1 ainda
acrescentou a conversão para uma porcentagem do “total definido”, formulação
ambígua que favoreceu renormalizar as contribuições para somarem 100%.

**Efeito observado:** o executor produziu Wind perto de 68%, enquanto o verifier
esperava Heat entre 40% e 60%. O teste de tendência passou.

**Fator concomitante dominante:** a família metodológica das skills estava, em
linhas gerais, alinhada ao oracle, mas o executor a implementou de forma
infiel. Manteve Longwave e Shortwave separados em vez de formar
`NetRadiation`, substituiu `FactorAnalyzer` por `sklearn.PCA` com rotação
artesanal, inventou um matching bijetivo fator-categoria, clipou deltas negativos
e renormalizou tudo para 100%.

**Por que ainda é M2:** o MOSAIC fez specification laundering: converteu
exemplos e defaults de skill em contrato normativo e introduziu uma
interpretação ambígua de porcentagem que aparece literalmente na execução.
Porém, a causa dominante do score foi M1/executor; por isso este caso tem
confiança menor que os outros 12.

**Guardrail mínimo:** novos métodos e pós-processamentos precisam registrar
`source`, `normativeStrength` e a frase que os sustenta. Defaults e exemplos não
podem virar `doneWhen`; normalização, threshold ou transformação ausentes do
pedido e de uma regra explicitamente obrigatória devem permanecer decisões
locais sujeitas a cross-check.

### `multilingual-video-dubbing`

**Onde entrou:** P1 e duas revisões localizadas.

**Mutação do contrato:** true peak próximo de -1,5 dBTP virou hard gate, enquanto
a exigência principal de naturalidade equivalente a voz humana permaneceu sem
um critério operacional comparável.

**Efeito observado:** as revisões concentraram esforço em `loudnorm`, limiter e
até injeção de seno. O nó terminou `blocked` exclusivamente porque o peak
permaneceu em aproximadamente -4,5 dBFS. O verifier não penalizou esse peak;
reprovou a naturalidade, com UTMOS de aproximadamente 1,29.

**Fator concomitante:** o executor escolheu eSpeak e não substituiu o sintetizador
por uma opção de maior naturalidade, embora a própria skill descrevesse eSpeak
como opção de protótipo e alternativas neurais como superiores em qualidade.

**Guardrail mínimo:** distinguir métrica de resultado de métrica diagnóstica.
Um hint só pode virar hard gate quando deriva diretamente do pedido e mede o
resultado prioritário; metas auxiliares não devem consumir a recuperação nem
impedir entrega válida.

P1 também deve executar um coverage check: não pode adicionar um gate técnico
secundário enquanto “human-level/high quality”, o principal requisito do
usuário, permanece sem critério observável.

### `react-performance-debugging`

**Onde entrou:** P1 body-aware, a partir da skill de browser testing.

**Mutação do contrato:** waterfall, heap, layouts, script time e CLS completos
viraram gate rígido, embora Chromium não estivesse disponível.

**Efeito observado:** o executor gastou tentativas procurando uma medição
impossível e terminou `blocked` por esse critério, sem concluir correções
estáticas já identificáveis em bundle e cache. O verifier reprovou justamente
checkout e bundle.

**Fator concomitante:** o executor poderia ter aplicado as correções de código
mesmo sem telemetria completa. Também não tentou a recuperação sugerida pelo
erro (`npx playwright install`) e interpretou uma métrica de build diferente da
medida pelo verifier como prova de redução do bundle.

**Guardrail mínimo:** todo gate dependente de capacidade precisa de preflight.
Se a capacidade não existe e não é requisito explícito do usuário, a medição
deve virar evidência best-effort, não impossibilidade terminal.

### Causa sistêmica do grupo B

O extrator de hints reconhece `vocabulary`, `gap`, `division` e `dependency`, e
o prompt chama os hints de advisory. Entretanto, o produto final do P1 é um
novo grafo normativo; não há controle mecânico de provenance, prioridade ou
suporte no pedido para o novo `doneWhen`.

No benchmark, todas as skills da tarefa foram configuradas como `required` e
portanto injetadas como universais. A seleção opcional ficou desativada com
`maxSkills: 0`. Isso não significa que as skills estavam ausentes; significa
que o tratamento testado expôs todos os corpos obrigatórios e seus hints ao
planejamento.

Essa configuração dá dupla autoridade ao mesmo corpo: ele participa da
extração P0→P1 e reaparece como Universal Skill na execução. O braço Direct
também receberia os corpos num experimento pareado; o tratamento adicional do
MOSAIC é promover defaults do corpo a goals e `doneWhen` normativos.

## Grupo C — Corrupção semântica ou expansão indevida do pedido

### Definição

O planejamento preserva a aparência geral da tarefa, mas altera um quantificador,
um nome literal, um conjunto de outputs ou uma regra de agregação. O executor
então implementa consistentemente a tarefa errada.

### `shock-analysis-demand`

**Onde entrou:** P0 e P1.

**Mutação do contrato:** o pedido mandava trabalhar somente em Excel, sem
Python, e copiar as sheets 38-38 mantendo seus nomes originais. O plano omitiu
a proibição de Python e passou a exigir sheets chamadas exatamente `SUPPLY` e
`USE`. Também cristalizou a formulação incoerente “2026-2043 eight-year”,
misturando horizonte de projeção com duração do investimento.

**Efeito observado:** o executor usou `openpyxl`, removeu sheets existentes e
criou `SUPPLY` e `USE` sintéticas; o verifier exigia o nome original
`SUPPLY (38-38)-2024` e reprovou a estrutura.

**Fator concomitante:** o workbook continha vários outros erros de fórmula,
links, escala, cenários e bell shape. A correção do nome impediria uma falha
diretamente induzida pelo plano, mas não resolveria tudo. O executor também
fabricou dados em vez de coletar e preservar a proveniência solicitada.

**Guardrail mínimo:** preservar tokens literais do pedido e detectar
contradições entre P0/P1 e frases como “original name unchanged”. Paths,
filenames, sheet names e intervalos temporais devem ter tratamento de contrato,
não de paráfrase.

### `threejs-structure-parser`

**Onde entrou:** P1 body-aware.

**Mutação do contrato:** o P1 passou a exigir que todo `THREE.Group` nomeado,
incluindo grupos aninhados, fosse representado como part/link e que cada mesh
fosse associado ao ancestral nomeado mais próximo. Essa expansão incluiu o
grupo contêiner raiz `ferris_wheel` como se fosse uma peça.

**Efeito observado:** foram criados exatamente os dois extras que o verifier
rejeitou: o diretório `part_meshes/ferris_wheel` e o arquivo
`links/ferris_wheel.obj`.

**Fator concomitante:** o executor seguiu literalmente o conjunto ampliado pelo
P1. Sua própria validação abriu uma exceção para o `ferris_wheel.obj` vazio;
essa ausência de geometria era evidência de que o root era contêiner, não peça.

**Guardrail mínimo:** toda expansão do conjunto de outputs precisa demonstrar
que os novos elementos satisfazem a definição do usuário, não apenas uma
propriedade sintática ampla. Containers estruturais e parts entregáveis devem
permanecer conceitos distintos.

### `tictoc-unnecessary-abort-detection`

**Onde entrou:** P1 body-aware.

**Mutação do contrato:** o P1 introduziu a regra existencial “incluir toda
transação com pelo menos um abort comprovadamente desnecessário”. A política
correta precisava agregar todos os aborts da transação e excluí-la se qualquer
registro demonstrasse um hard abort ou soft abort necessário.

As skills também introduziram intervalos WTS/RTS, proof objects e candidate
serialization order, apesar de o pedido dizer que mudanças de read timestamp
não estavam no trace e destacar o contador ATS compartilhado.

**Efeito observado:** o executor classificou registros individualmente e
incluiu milhares de IDs ao encontrar pelo menos uma condição local favorável.
O policy ladder deu reward 0,1 e diagnosticou que os abort records foram
tratados de forma ampla demais como desnecessários.

**Fator concomitante:** a implementação do executor também simplificou a regra
para comparações de timestamps, descartou `ats_at_write` e não usou
`ats_at_abort`, ignorando justamente a janela de writes/access counter.

**Guardrail mínimo:** preservar quantificadores e cardinalidade durante P1.
Termos como “cada”, “qualquer”, “todos”, “pelo menos um” e “nenhum” precisam de
um diff semântico, sobretudo quando determinam agregação por entidade. P1 só
pode adicionar predicados deriváveis dos campos declarados; estado
explicitamente ausente não pode virar requisito.

### Causa sistêmica do grupo C

O planejamento usa texto livre para reexpressar o pedido. O schema garante um
grafo válido, mas não garante fidelidade semântica. Campos aparentemente
pequenos — um nome, um quantificador ou o universo dos outputs — podem alterar
completamente a função avaliada pelo verifier.

## Grupo D — Amplificação pela revisão ou topologia

### Definição

O problema inicial poderia ser local, recuperável ou parcialmente contornável,
mas a política de revisão, o orçamento ou as dependências o transformam em
falha terminal da tarefa.

### `drone-planning-control`

**Onde entrou:** uso do loop de revisão localizada.

**Mutação operacional:** um nó grande de produção e validação emitiu quatro
`needs_revision` para problemas de implementação, como frames, yaw, sinais de
thrust e métricas. As três primeiras revisões mantiveram essencialmente a mesma
estrutura e consumiram todo `revision.max = 3` sem uma reparação estrutural
material.

Em cada revisão, o target reiniciou com observations, artifacts e outcome
vazios. O próximo prompt de execução recebeu o goal e os critérios revisados,
mas não recebeu um handoff explícito com `invalidatedAssumption`,
`requestedEffect` e o ledger operacional anterior. Isso favoreceu redescoberta
e repetição.

**Efeito observado:** o quarto pedido não recebeu uma nova recuperação. O run
terminou por volta de 300 segundos; se a próxima transição de revisão fosse
executada, encontraria o limite já esgotado. O score parcial foi 0,766667.

**Fator concomitante:** o executor não conseguiu corrigir os erros físicos e de
referencial nas tentativas disponíveis. O timeout externo também participou do
encerramento.

**Guardrail mínimo:** `needs_revision` deve ser reservado a mudanças estruturais.
Uma revisão que mantém o mesmo grafo e apenas reexecuta implementação deve ser
tratada como retry do nó, não consumir orçamento estrutural. O runtime também
deve detectar estagnação e exigir um efeito material antes de anexar um novo
snapshot. Quando houver revisão real, o próximo executor precisa receber um
handoff imutável dos critérios falsos e da evidência que motivou a mudança.

### `fix-visual-stability`

**Onde entrou:** topologia P0/P1 e propagação de bloqueio.

**Mutação operacional:** o plano criou `assessment -> fix` e tornou um baseline
quantitativo de browser condição rígida do primeiro nó. O assessment já havia
identificado em fonte tema pós-hidratação, fontes, imagens sem dimensões e
blocos sem espaço reservado, mas não conseguiu medir o baseline via
API/Playwright.

**Efeito observado:** o primeiro nó terminou `blocked 3/4`; como o nó de fix
dependia integralmente dele, o scheduler bloqueou o descendente e nenhuma
correção foi aplicada.

**Fator concomitante:** a capacidade de browser estava indisponível. Isso
impedia a medição, mas não as correções estáticas já fundamentadas.

**Guardrail mínimo:** uma observação diagnóstica opcional não deve ser uma
dependência all-or-nothing da implementação. O plano deveria combinar
diagnóstico e correção no mesmo nó, marcar a medição como best-effort ou
permitir projeção de evidência parcial suficiente para o descendente.

### Causa sistêmica do grupo D

O runtime age corretamente de acordo com a política atual:

- cada snapshot localizado aceito conta para o limite, independentemente de
  progresso material;
- ao esgotar o limite, o target é bloqueado;
- uma dependência terminal `blocked` ou `failed` bloqueia automaticamente todos
  os descendentes pendentes.

Portanto, esses casos não são bugs M3 da máquina de estados. São falhas M2 da
política e da topologia produzida pelo MOSAIC.

## Mapa transversal dos 13 casos

| Tarefa                               | Grupo primário          | Entrada              | Efeito terminal       | Força causal |
| ------------------------------------ | ----------------------- | -------------------- | --------------------- | ------------ |
| `adaptive-cruise-control`            | A — erosão              | P1                   | Falso `completed`     | Forte        |
| `fix-build-agentops`                 | A — erosão              | P0/P1                | Falso `completed`     | Forte        |
| `fix-build-google-auto`              | A — erosão              | Revisão localizada   | Falso `completed`     | Forte        |
| `hvac-control`                       | A — erosão              | P0/P1                | Falso `completed`     | Forte        |
| `setup-fuzzing-py`                   | A — erosão              | Revisão localizada   | Falso `completed`     | Média        |
| `lake-warming-attribution`           | B — gate/método         | P1/hints             | Falso `completed`     | Limítrofe    |
| `multilingual-video-dubbing`         | B — gate/método         | P1 e revisões        | Falso `blocked`       | Forte        |
| `react-performance-debugging`        | B — gate/método         | P1/hints             | Falso `blocked`       | Forte        |
| `shock-analysis-demand`              | C — corrupção semântica | P0/P1                | Falso `completed`     | Forte        |
| `threejs-structure-parser`           | C — corrupção semântica | P1/hints             | Falso `completed`     | Forte        |
| `tictoc-unnecessary-abort-detection` | C — corrupção semântica | P1/hints             | Falso `completed`     | Forte        |
| `drone-planning-control`             | D — revisão/topologia   | Revisões localizadas | Recuperação esgotada  | Média        |
| `fix-visual-stability`               | D — revisão/topologia   | P0/P1 e scheduler    | Descendente bloqueado | Forte        |

## Causas-raiz compartilhadas

### 1. Fidelidade ao pedido é um requisito de prompt, não uma invariante

Os prompts dizem para preservar intenção, constraints e deliverables. O schema
do grafo, porém, só valida forma, referências e aciclicidade. Não existe um
contrato mecânico que impeça:

- enfraquecimento de um threshold;
- troca de nome literal;
- mudança de quantificador;
- expansão do conjunto de outputs;
- promoção de tentativa ou documentação a sucesso.

### 2. Hints advisory produzem um grafo normativo

O extrator pode sugerir apenas quatro tipos de efeito, mas o P1 reescreve
livremente goals e `doneWhen`. Depois da reescrita, o runtime não distingue um
requisito vindo do usuário de um requisito introduzido por uma skill.

### 3. O mesmo modelo executa e certifica

O executor produz o artefato, escolhe a evidência e marca cada critério como
`satisfied`. A validação estruturada exige que um `completed` tenha todos os
booleans verdadeiros, mas não recompõe métricas, abre artefatos segundo um
contrato específico nem verifica equivalência semântica.

### 4. Revisão é validada por forma, não por progresso

Uma revisão localizada pode preservar a mesma estrutura, enfraquecer critérios
ou simplesmente provocar nova execução. Se o grafo continua válido, ela conta
contra o orçamento mesmo sem resolver a premissa invalidada.

Além disso, o target revisado perde seu ledger local, e o prompt seguinte não
carrega explicitamente a solicitação de revisão. O conhecimento precisa ser
reconstruído a partir do novo goal, dos arquivos e de novas observações.

### 5. Dependências são binárias

O scheduler conhece `completed`, `blocked` e `failed`, mas não uma dependência
parcialmente satisfeita ou diagnóstica. Um bloqueio upstream se propaga para
todos os descendentes, mesmo quando a evidência já obtida permitiria trabalho
útil.

### 6. Propriedade e invalidação de artefatos não são rastreadas

Um descendente pode modificar um arquivo que sustentava a conclusão de um nó
protegido. O grafo preserva o nó como `completed`, mas não sabe que sua prova
ficou obsoleta. Em `setup-fuzzing-py`, o nó de validação modificou `fuzz.py`
depois de o nó produtor ter sido congelado, invalidando critérios anteriores
sem reabri-los.

## Guardrails recomendados por prioridade

### P0 — Contract Atom Ledger e preservação monotônica

Antes de P0, extrair um ledger mínimo de átomos tipados: obrigações duras,
thresholds, identificadores literais, schemas fechados, proibições de
ferramenta, proveniência, cardinalidade, topologia e campos observáveis. Antes
de aceitar P1 ou uma revisão, comparar o novo plano com esse ledger e com o
snapshot anterior. Rejeitar automaticamente:

- threshold convertido em simples relatório;
- sucesso convertido em tentativa, documentação ou best-effort;
- alteração de nomes, paths, comandos, chaves e formatos literais;
- mudança de quantificadores;
- expansão ou redução não justificada do conjunto de entregáveis.

Esse controle cobre diretamente os grupos A e C.

### P0 — Provenance e admissão de hard gates

Cada `doneWhen` novo deveria indicar se deriva do pedido ou de um hint. Um
critério vindo apenas de skill deve ser advisory por padrão. Para virar hard
gate, precisa demonstrar:

1. suporte direto no pedido;
2. capacidade disponível no ambiente;
3. relação necessária com o resultado do usuário;
4. ausência de conflito com outro requisito prioritário.

Esse controle cobre o grupo B.

### P0 — Revisão não regressiva

Uma revisão deve preservar todos os predicados de aceitação ainda válidos e
mostrar qual mudança estrutural responde à evidência. Revisões semanticamente
idênticas não devem consumir `revision.max`; pedidos não estruturais devem
retornar ao mesmo nó como retries de execução.

O próximo executor deve receber um handoff delimitado da revisão: premissa
invalidada, efeito solicitado, critérios ainda falsos e observações citadas.

### P1 — Invalidação por mutação de artefato

Se um nó posterior alterar um artefato que sustentava um ancestral concluído,
o ancestral deve ser reaberto ou revalidado end-to-end antes da entrega. Esse
controle exige rastrear somente os paths materialmente citados, não ownership
genérico de todo o filesystem.

### P1 — Gate de `blocked`

Antes de aceitar `blocked`, verificar se:

- a impossibilidade é externa, e não um erro corrigível do executor;
- alternativas razoáveis foram tentadas;
- critérios indisponíveis são realmente obrigatórios;
- trabalho útil independente ainda pode ser realizado.

### P1 — Validação independente por tipo de artefato

Adicionar validadores mecânicos somente onde já existe um contrato claro:
schema fechado, arquivo/path literal, comando de teste canônico, fórmula,
parser de formato ou métrica recomputável. Isso reduz falsos `completed` sem
pretender resolver validação semântica geral.

### P2 — Dependências com semântica de evidência

Avaliar uma distinção mínima entre pré-requisito obrigatório e evidência
diagnóstica. A mudança só se justifica se os controles anteriores não forem
suficientes, pois amplia a complexidade do scheduler.

## Casos M1 e M0 preservados para referência

Os 31 casos M1 foram erros do executor com oportunidade de guardrail, mas sem
causalidade M2 suficiente:

- `ada-bathroom-plan-repair`
- `azure-bgp-oscillation-route-leak`
- `bike-rebalance`
- `crystallographic-wyckoff-position-analysis`
- `data-to-d3`
- `debug-trl-grpo`
- `edit-pdf`
- `energy-ac-optimal-power-flow`
- `energy-unit-commitment`
- `enterprise-information-search`
- `exam-block-sequencing`
- `financial-modeling-qa`
- `flood-risk-analysis`
- `invoice-fraud-detection`
- `jpg-ocr-stat`
- `manufacturing-fjsp-optimization`
- `organize-messy-files`
- `paper-anonymizer`
- `paratransit-routing`
- `pddl-tpp-planning`
- `pptx-reference-formatting`
- `protein-expression-analysis`
- `python-scala-translation`
- `quantum-numerical-simulation`
- `reserves-at-risk-calc`
- `seismic-phase-picking`
- `shock-analysis-supply`
- `suricata-custom-exfil`
- `syzkaller-ppdev-syzlang`
- `video-silence-remover`
- `xlsx-recover-data`

Os oito casos M0 não tiveram contribuição material do MOSAIC:

- `civ6-adjacency-optimizer`
- `dynamic-object-aware-egomotion`
- `exoplanet-detection-period`
- `fix-druid-loophole-cve`
- `manufacturing-codebook-normalization`
- `r2r-mpc-control`
- `simpo-code-reproduction`
- `software-dependency-audit`

`flink-query` não pertence às 52 falhas pontuadas: foi o único erro do verifier.

## Índice de evidências

Para cada rollout abaixo, os arquivos principais são
`trajectory/llm_trajectory.jsonl`, `result.json` e o conteúdo de `verifier/`.

| Tarefa                               | Rollout                                        |
| ------------------------------------ | ---------------------------------------------- |
| `adaptive-cruise-control`            | `adaptive-cruise-control__588d69d8`            |
| `drone-planning-control`             | `drone-planning-control__5eae7f62`             |
| `fix-build-agentops`                 | `fix-build-agentops__3f8d859b`                 |
| `fix-build-google-auto`              | `fix-build-google-auto__c17a242d`              |
| `fix-visual-stability`               | `fix-visual-stability__c6855460`               |
| `hvac-control`                       | `hvac-control__978f93bd`                       |
| `lake-warming-attribution`           | `lake-warming-attribution__8b338180`           |
| `multilingual-video-dubbing`         | `multilingual-video-dubbing__e883f62a`         |
| `react-performance-debugging`        | `react-performance-debugging__acb03431`        |
| `setup-fuzzing-py`                   | `setup-fuzzing-py__23862d44`                   |
| `shock-analysis-demand`              | `shock-analysis-demand__ad829e58`              |
| `threejs-structure-parser`           | `threejs-structure-parser__9a40f121`           |
| `tictoc-unnecessary-abort-detection` | `tictoc-unnecessary-abort-detection__0706bd7d` |

## Implementação do runtime relacionada

- A configuração usada pelo benchmark está em
  [`src/mosaic.ts`](../../src/mosaic.ts): todas as skills da tarefa são `required`,
  `maxSkills` é zero, há no máximo três revisões localizadas e 32 turns por nó.
- A contagem e o bloqueio por limite de revisão estão em
  [`packages/mosaic/src/lib/states/revision/index.ts`](../../../../packages/mosaic/src/lib/states/revision/index.ts).
- O reset do target revisado, incluindo seu ledger de observações, está em
  [`packages/mosaic/src/lib/states/revision/localized.ts`](../../../../packages/mosaic/src/lib/states/revision/localized.ts).
- A propagação automática de bloqueio para descendentes está em
  [`packages/mosaic/src/lib/states/schedule/index.ts`](../../../../packages/mosaic/src/lib/states/schedule/index.ts).
- O contrato de decisão e a validação estrutural de `completed` estão em
  [`packages/mosaic/src/lib/schemas/outcome.ts`](../../../../packages/mosaic/src/lib/schemas/outcome.ts).
- A extração advisory de hints e os prompts de P1/revisão estão em
  [`packages/mosaic/src/lib/prompts/hints.ts`](../../../../packages/mosaic/src/lib/prompts/hints.ts)
  e
  [`packages/mosaic/src/lib/prompts/revision.ts`](../../../../packages/mosaic/src/lib/prompts/revision.ts).

## Limites da conclusão

- Sem braço Direct, não é possível medir quantas dessas tarefas passariam sem
  MOSAIC nem calcular regressão pareada.
- Em vários M2 também houve erro independente do executor. M2 significa que a
  orquestração contribuiu materialmente, não que foi a única causa.
- Não foi encontrado defeito M3: scheduler, schemas, ACP e máquina de estados
  se comportaram conforme a implementação observada.
- A prioridade recomendada é reforçar invariantes existentes antes de criar
  um verificador semântico geral ou ampliar o modelo de dependências.
