# Manual operacional do estudo empírico MOSAIC

Este manual descreve o fluxo completo para preparar, executar, pontuar,
analisar e arquivar um estudo do benchmark MOSAIC. Ele foi escrito a partir da
CLI e dos schemas atuais de `benchmarks/mosaic`; quando houver divergência, os
schemas versionados e o código da CLI são a autoridade.

> **Estado de prontidão em 2026-08-08: NO-GO.** Este manual descreve a operação
> pretendida, mas ainda existem bloqueios de validade no outcome, paridade das
> condições, metering e recuperação de crashes. Consulte
> [READINESS.md](./READINESS.md) e não inicie chamadas pagas enquanto qualquer
> item P0 estiver aberto. `--validate-config` valida o envelope, não certifica
> que o estudo está pronto.

O fluxo computacional pode ser executado por um único orquestrador retomável;
as etapas humanas continuam explícitas. Um estudo completo passa por estas
fases:

1. validar o instrumento TypeScript;
2. construir e testar a imagem R;
3. executar e pontuar o piloto;
4. calibrar a segunda família de modelos;
5. calcular o tamanho amostral;
6. autorar e validar os casos confirmatórios;
7. congelar todas as decisões no `freeze.json`;
8. executar e pontuar os estudos primário, de replicação e de sensibilidade;
9. executar a revisão semântica cega;
10. analisar os CSVs no container R;
11. empacotar os dados e a proveniência.

## 1. O que cada etapa produz

| Etapa           | Entrada principal                 | Saída principal                     | Faz chamadas pagas?            |
| --------------- | --------------------------------- | ----------------------------------- | ------------------------------ |
| Build/test      | código do repositório             | binários, testes e schemas          | não                            |
| Container R     | `Dockerfile` e `renv.lock`        | imagem OCI imutável                 | não                            |
| Pilot           | schedule de 1.800 runs            | records e traces                    | sim                            |
| Score do pilot  | records, traces e casos           | `scores.json` e `scores.csv`        | não                            |
| Calibração      | dois conjuntos de 180 score rows  | `calibration.json`                  | não, além dos runs anteriores  |
| Power           | configuração estatística          | `power-result.json`                 | não                            |
| Freeze          | todos os artefatos preparatórios  | `freeze.json` imutável              | não                            |
| Estudo primário | schedule congelado                | records e traces                    | sim                            |
| Score primário  | records e traces                  | `scores.json` e `scores.csv`        | não                            |
| Replicação      | schedule da segunda família       | records, traces e scores            | sim                            |
| Sensibilidade   | schedule com limite p95           | records, traces e scores            | sim                            |
| Revisão cega    | entregas selecionadas             | reviews e estabilidade              | judge auxiliar é opcional/pago |
| Analyze         | scores CSV, freeze e configuração | resultado JSON e relatório Markdown | não                            |
| Package         | lista explícita de arquivos       | pacote reproduzível e hashes        | não                            |

Os dados brutos do estudo são os records e traces gerados por `run`. Os dados
tabulares auditados são produzidos por `score`. O resultado inferencial é
produzido por `analyze`. Não use o JSON que `run` imprime em stdout como
substituto de `scores.json` ou `scores.csv`.

## 2. O que significam “freeze” e “estudo”

### Freeze

O freeze é o registro imutável de todas as escolhas tomadas antes de observar
os resultados confirmatórios. Ele fixa, entre outros itens:

- o commit Git e a exigência de worktree limpo;
- o modelo primário, reranker e embedder;
- o baseline escolhido pelo piloto;
- o limite p95 usado na sensibilidade;
- o candidato aprovado para replicação;
- `N_power` e `N_final`;
- casos confirmatórios, schedule e seeds;
- preços, prompts, schemas, catálogo, tools e protocolo;
- implementação R, `renv.lock` e digest da imagem OCI.

O arquivo `freeze.json` é criado com escrita exclusiva e não pode ser
sobrescrito pela CLI. Depois que ele existe, os schedules do estudo referenciam
seu `manifestHash`. Alterar casos, seeds, condições, modelos, preços, código ou
imagem depois disso invalida o estudo congelado.

### Estudo

O estudo é a execução dos schedules congelados e a derivação dos resultados.
Para cada run, o benchmark grava uma tentativa imutável, um trace encadeado por
hash e um record terminal. Depois, `score` verifica esses dados contra o caso e
gera uma linha `ScoreRowV1`. Por fim, `analyze` consome o CSV completo e ajusta
o modelo estatístico congelado.

O contraste primário é M1 versus o baseline escolhido no piloto, nos casos de
classes E/F ou marcados como adaptativos. A replicação é analisada
separadamente, sem pooling com a família OpenAI.

## 3. Orquestrador e limites de automação

`scripts/study.mjs` automatiza a parte computacional depois que os quatro
inputs científicos e a imagem OCI por digest estão prontos. Gere o envelope de
configuração, substitua todos os placeholders e faça a checagem sem custo:

```sh
node benchmarks/mosaic/scripts/study.mjs --print-config > /tmp/mosaic-study.json
node benchmarks/mosaic/scripts/study.mjs \
  --validate-config /tmp/mosaic-study.json
```

O arquivo referencia preços, 60 casos de calibração, casos confirmatórios e a
configuração de power. Os casos confirmatórios precisam ter o `nFinal`
determinístico dessa configuração; calcule-o antes com o comando `power` de
baixo nível quando ainda estiver autorando o corpus. A execução oficial exige
worktree limpo, a imagem já disponível localmente e confirmação explícita das
chamadas pagas:

```sh
export OPENROUTER_API_KEY='carregada-do-seu-secret-manager'
node benchmarks/mosaic/scripts/study.mjs \
  --config /tmp/mosaic-study.json \
  --yes-paid-study
```

O script fixa uma cópia byte a byte dos inputs em `<root>/inputs`, gera os
schedules de calibração e das três famílias, executa gates, piloto, calibração,
power, freeze, runs, scoring e análises R. Para retomar uma interrupção, repita
exatamente o mesmo comando. Uma etapa só é pulada quando seu receipt imutável e
todos os outputs esperados existem; outputs órfãos interrompem o fluxo para
auditoria.

A CLI de baixo nível continua disponível para executar cada fase deste manual.
Ainda são deliberadamente humanos ou separados:

- autoria e auditoria dos casos de calibração e confirmatórios;
- revisão semântica cega em duas passagens separadas por 14 dias;
- extração/materialização das entregas cegadas;
- oracles opcionais somente sobre falhas;
- plano explícito do pacote de publicação.

Não edite schedules, cases, receipts ou scores depois de gerar seus hashes.

## 4. Pré-requisitos

### Repositório e ferramentas

Você precisa de:

- Node.js e o gerenciador de pacotes já usado pelo workspace;
- dependências do monorepo instaladas;
- Git;
- Docker compatível com imagens OCI;
- acesso à API do OpenRouter para os comandos `run`;
- espaço em disco suficiente para milhares de pequenos arquivos de trace;
- um diretório externo ao repositório para os dados do estudo.

R e `Rscript` não precisam existir no host se você executar os testes e as
análises pelo container. O target `mosaic-benchmark:analysis-test` só serve para
um host que já possua o ambiente R congelado.

### Recursos do Docker

Confira os recursos antes de construir a imagem:

```sh
docker info --format 'memory={{.MemTotal}} cpus={{.NCPU}}'
```

Na experiência local deste projeto, 2 GiB não foram suficientes para compilar
`RcppEigen`. Use 6 GiB e 4 CPUs como ponto de partida operacional. Isso não é
um parâmetro estatístico do protocolo, apenas uma recomendação para o build.

Com Colima, primeiro confira se há workloads importantes:

```sh
docker ps
```

Reiniciar o Colima interrompe containers em execução. Se for seguro fazê-lo:

```sh
colima stop
colima start --memory 6 --cpu 4
```

No Docker Desktop, ajuste a memória em Resources antes do build.

### Credencial

O runner lê apenas `OPENROUTER_API_KEY` do ambiente. Não coloque a chave em
JSON, freeze, schedule, trace, histórico de shell ou pacote de reprodução.
Prefira um secret manager. Para uma sessão interativa, uma alternativa é:

```sh
read -s OPENROUTER_API_KEY
export OPENROUTER_API_KEY
```

Remova a variável ao terminar os runs pagos:

```sh
unset OPENROUTER_API_KEY
```

### Controle de custo

Antes de executar qualquer `run`, confira o tamanho do schedule e o preço
congelado. Os principais volumes são:

- piloto: `60 × 6 × 5 = 1.800` runs;
- calibração: `60 × 3 = 180` runs para Luna e outros 180 para o candidato;
- primário: `N_final × 2 × 5` runs;
- replicação: `N_final × 2 × 5` runs;
- sensibilidade: `N_final × 2 × 5` runs adicionais, se executada;
- oracles: somente sobre falhas e fora da família primária.

O runner atual executa o schedule sequencialmente. Reserve tempo de parede e
janela de quota compatíveis.

## 5. Diretório recomendado para o estudo

Use um caminho absoluto fora do repositório. Isso evita que resultados pagos
deixem o worktree sujo no momento do freeze.

Uma estrutura recomendada é:

```text
mosaic-study-001/
  inputs/
    prices.json
    calibration-cases.json
    calibration-input.json
    power-config.json
    confirmatory-cases.json
    confirmatory-schedule.prefreeze.json
    confirmatory-schedule.json
    replication-schedule.json
    sensitivity-schedule.json
    analysis-config.primary.json
    analysis-config.replication.json
    analysis-config.sensitivity.json
  pilot/
    schedule.json
    command.json
    run-summary.json
    artifacts/
      records/
      traces/
    scores.json
    scores.csv
  calibration/
    luna/
      schedule.json
      artifacts/
      scores.json
      scores.csv
    candidate/
      schedule.json
      artifacts/
      scores.json
      scores.csv
    calibration.json
  power/
    power-result.json
    verification/
  freeze/
    validate-before-freeze.json
    freeze-input.json
    freeze.json
    command.json
  primary/
    artifacts/
    run-summary.json
    scores.json
    scores.csv
    analysis-result.json
    report.md
    verification/
  replication/
    artifacts/
    run-summary.json
    scores.json
    scores.csv
    analysis-result.json
    report.md
    verification/
  sensitivity/
    artifacts/
    run-summary.json
    scores.json
    scores.csv
    analysis-result.json
    report.md
    verification/
  review/
    review-plan.json
    assignments.json
    private-key.json
    reviews.json
    status.json
    artifacts/review/
  package/
```

Defina variáveis específicas para a sessão. Substitua o caminho do estudo por
um caminho absoluto real:

```sh
MOSAIC_REPO_ROOT="$(git rev-parse --show-toplevel)"
MOSAIC_STUDY_ROOT="/absolute/path/to/mosaic-study-001"
MOSAIC_STUDY_ID="mosaic-study-001"
MOSAIC_CLI="$MOSAIC_REPO_ROOT/benchmarks/mosaic/dist/src/cli.js"

mkdir -p \
  "$MOSAIC_STUDY_ROOT/inputs" \
  "$MOSAIC_STUDY_ROOT/pilot" \
  "$MOSAIC_STUDY_ROOT/calibration/luna" \
  "$MOSAIC_STUDY_ROOT/calibration/candidate" \
  "$MOSAIC_STUDY_ROOT/power/verification" \
  "$MOSAIC_STUDY_ROOT/freeze" \
  "$MOSAIC_STUDY_ROOT/primary/verification" \
  "$MOSAIC_STUDY_ROOT/replication/verification" \
  "$MOSAIC_STUDY_ROOT/sensitivity/verification" \
  "$MOSAIC_STUDY_ROOT/review/artifacts/review" \
  "$MOSAIC_STUDY_ROOT/package"
```

Todos os comandos da CLI imprimem exatamente um documento JSON em stdout e
progresso em stderr. Quando redirecionar stdout, abra o JSON resultante e
confirme `"ok": true`. Os arquivos passados em `--output`, `--csv`, `--result`,
`--report` ou `--path` são criados com exclusividade; escolha um caminho novo
para repetir um comando.

## 6. Gate do instrumento TypeScript

Execute os gates a partir da raiz do repositório. A ordem recomendada evita
corridas em diretórios `dist` compartilhados:

```sh
cd "$MOSAIC_REPO_ROOT"

npx nx sync
npx nx show projects
npx nx run-many -t typecheck -p agent,mosaic,mosaic-benchmark --parallel=2
npx nx run-many -t build -p agent,mosaic,mosaic-benchmark --parallel=1
npx nx run-many -t test -p agent,mosaic,mosaic-benchmark --parallel=1
npx nx run mosaic-benchmark:schemas
node "$MOSAIC_CLI" validate
node "$MOSAIC_CLI" conformance
git diff --check
```

Resultados esperados:

- `validate` termina com `ok: true`, `result.valid: true` e `issues: []`;
- `conformance` termina com `ok: true`, `result.valid: true` e `issues: []`;
- os schemas ficam em `benchmarks/mosaic/schemas/v1`;
- `CaseV1.schema.json`, `RunSpecV1.schema.json`,
  `FreezeManifestV1.schema.json`, `ScoreRowV1.schema.json` e os demais schemas
  são as referências para arquivos autorados externamente.

Não execute um estudo pago se esses gates falharem.

## 7. Construir, testar e publicar a imagem R

### 7.1 Fixar a imagem base

O `Dockerfile` exige `R_IMAGE` imutável. Use a referência
`rocker/r-ver@sha256:<digest>` que foi verificada para a plataforma do estudo;
não congele uma tag mutável.

```sh
MOSAIC_R_BASE_IMAGE='rocker/r-ver@sha256:SUBSTITUA_PELO_DIGEST_VERIFICADO'
docker pull "$MOSAIC_R_BASE_IMAGE"
```

### 7.2 Build

O contexto do build precisa ser `benchmarks/mosaic`, pois o Dockerfile copia
`analysis/`:

```sh
cd "$MOSAIC_REPO_ROOT"

docker build \
  --pull=false \
  --build-arg R_IMAGE="$MOSAIC_R_BASE_IMAGE" \
  -f benchmarks/mosaic/container/Dockerfile \
  -t mosaic-analysis:local \
  benchmarks/mosaic
```

O build pode acessar a rede para restaurar os pacotes exatos do `renv.lock`.
Power e análise posteriores são executados sem rede.

Capture o ID imutável da imagem local:

```sh
MOSAIC_ANALYSIS_IMAGE="$(docker image inspect mosaic-analysis:local --format '{{.Id}}')"
echo "$MOSAIC_ANALYSIS_IMAGE"
```

O valor deve ter a forma `sha256:<64 hexadecimais>`.

### 7.3 Smoke do ambiente R no container

```sh
docker run --rm \
  --pull never \
  --network none \
  --read-only \
  --cap-drop ALL \
  --security-opt no-new-privileges \
  --tmpfs /tmp:rw,noexec,nosuid,size=256m \
  --env TMPDIR=/tmp \
  --user "$(id -u):$(id -g)" \
  "$MOSAIC_ANALYSIS_IMAGE" \
  /benchmark/analysis/test-analysis.R
```

Esse teste substitui a necessidade de `Rscript` no host. Se o host possuir o
R congelado, o equivalente opcional é:

```sh
npx nx run mosaic-benchmark:analysis-test
```

### 7.4 Digest publicável

Para uma execução oficial, publique a imagem em um registry e use a referência
`registry/repository@sha256:<digest>` retornada pelo registry. Verifique-a
localmente antes do freeze:

```sh
MOSAIC_ANALYSIS_IMAGE='registry.example/mosaic-analysis@sha256:SUBSTITUA'
docker pull "$MOSAIC_ANALYSIS_IMAGE"
docker image inspect "$MOSAIC_ANALYSIS_IMAGE"
```

Usar somente o ID local funciona tecnicamente, mas dificulta a reprodução em
outra máquina. O digest gravado no freeze deve ser exatamente a parte
`sha256:...` da referência usada nos comandos `freeze`, `power` e `analyze`.

## 8. Preparar os preços

Crie `$MOSAIC_STUDY_ROOT/inputs/prices.json`. Registre os preços vigentes no
momento do freeze, a URL da fonte e todos os modelos que possam aparecer em
requests. Não use valores estimados ou zeros fictícios.

```json
{
  "schemaVersion": 1,
  "currency": "USD",
  "models": {
    "openai/gpt-5.6-luna": {
      "kind": "completion",
      "capturedAt": "2026-08-09T00:00:00.000Z",
      "source": "https://provider.example/pricing/luna",
      "charges": [
        { "unit": "input-token", "quantity": 1000000, "priceUsd": 1.25 },
        {
          "unit": "cached-input-token",
          "quantity": 1000000,
          "priceUsd": 0.125
        },
        { "unit": "output-token", "quantity": 1000000, "priceUsd": 10 }
      ]
    },
    "qwen/qwen3.7-flash": {
      "kind": "completion",
      "capturedAt": "2026-08-09T00:00:00.000Z",
      "source": "https://provider.example/pricing/configured-candidate",
      "charges": [
        { "unit": "input-token", "quantity": 1000000, "priceUsd": 0.4 },
        { "unit": "output-token", "quantity": 1000000, "priceUsd": 1.2 }
      ]
    },
    "voyageai/voyage-4-large": {
      "kind": "embedding",
      "capturedAt": "2026-08-09T00:00:00.000Z",
      "source": "https://provider.example/pricing/voyage-4-large",
      "charges": [
        {
          "unit": "embedding-input-token",
          "quantity": 1000000,
          "priceUsd": 0.12
        }
      ]
    },
    "voyageai/rerank-2.5-lite": {
      "kind": "rerank",
      "capturedAt": "2026-08-09T00:00:00.000Z",
      "source": "https://provider.example/pricing/rerank-2.5-lite",
      "charges": [
        { "unit": "rerank-input-token", "quantity": 1000000, "priceUsd": 0.05 }
      ]
    }
  }
}
```

O exemplo acima mostra o shape e valores ilustrativos, não preços válidos.
Cada modelo possui fonte e timestamp próprios. Use somente unidades realmente
cobradas; tiers devem cobrir uma faixa contígua de zero até infinito. Preço
zero, unidade incompatível, moeda diferente de USD e fonte incompleta falham.

Valide o arquivo:

```sh
node "$MOSAIC_CLI" validate \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json"
```

### 8.1 Preflight pago e índice reutilizável

Depois que as checagens gratuitas e a autorização de custo passarem, construa
uma única vez o índice de setup. Os dois acknowledgements separam os probes
pagos da mera validação de metadata:

```sh
node "$MOSAIC_CLI" index \
  --artifacts "$MOSAIC_STUDY_ROOT/setup" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --candidate-model 'qwen/qwen3.7-flash' \
  --yes-paid-probes \
  --yes-paid-setup \
  > "$MOSAIC_STUDY_ROOT/setup/index-command.json"
```

Preserve `result.path`, `result.indexHash` e `result.setupUsage`. O hash liga
catálogo, views, bytes indexados, embedder, dimensão, algoritmo e vetores. Uma
reexecução reutiliza o artefato sem novas embeddings. Uma chamada paga
reservada sem resultado durável interrompe o fluxo para recuperação auditada;
ela nunca é repetida silenciosamente. Passe o mesmo `result.path` como
`--index` a todo `run`, `validate` e `freeze`.

```sh
export MOSAIC_INDEX_PATH='/caminho/absoluto/exato/de/result.path'
```

## 9. Piloto

O piloto usa os 60 casos internos e as seis condições B0, B1, B2, B3, M0 e M1,
com cinco repetições pareadas: 1.800 runs.

Controles não experimentais são comuns e fazem parte de
`ConditionFactorsV1`: 16 turnos por execução e dois retries de reparo
estruturado em todas as seis condições. B1–B3, M0 e M1 usam a mesma fronteira,
corpus, embedder, reranker e `maxCandidates: 5`; somente a regra declarada de
bundle reduz o ranking para uma skill, top-3 fixo ou seleção seletiva.

### 9.1 Gerar o schedule

```sh
MOSAIC_PILOT_SEED='pilot-seed-001'

node "$MOSAIC_CLI" pilot \
  --study-id "$MOSAIC_STUDY_ID" \
  --seed "$MOSAIC_PILOT_SEED" \
  --output "$MOSAIC_STUDY_ROOT/pilot/schedule.json" \
  > "$MOSAIC_STUDY_ROOT/pilot/command.json"
```

Confirme no schedule:

- exatamente 1.800 elementos;
- `phase: "pilot"`;
- `freezeHash: null`;
- modelo `openai/gpt-5.6-luna`, esforço `medium`;
- cinco repetições por caso e condição.

### 9.2 Executar

Confirme a credencial e o orçamento antes deste comando:

```sh
node "$MOSAIC_CLI" run \
  --schedule "$MOSAIC_STUDY_ROOT/pilot/schedule.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/pilot/artifacts" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  > "$MOSAIC_STUDY_ROOT/pilot/run-summary.json"
```

O piloto usa os casos internos; por isso `--cases` é omitido.

Se o processo cair, execute novamente com os mesmos caminhos e `--resume`:

```sh
node "$MOSAIC_CLI" run \
  --schedule "$MOSAIC_STUDY_ROOT/pilot/schedule.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/pilot/artifacts" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  --resume \
  > "$MOSAIC_STUDY_ROOT/pilot/run-resume-summary.json"
```

`--resume` só permite uma nova tentativa técnica quando a falha anterior
ocorreu antes da primeira chamada de modelo. Depois que uma chamada de modelo
começa, interrupção ou falha é terminal e permanece no dataset. Isso é
intencional; não apague a tentativa nem execute o run novamente em outra pasta
para substituir o resultado.

### 9.3 Pontuar

```sh
node "$MOSAIC_CLI" score \
  --schedule "$MOSAIC_STUDY_ROOT/pilot/schedule.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/pilot/artifacts" \
  --family exploratory \
  --output "$MOSAIC_STUDY_ROOT/pilot/scores.json" \
  --csv "$MOSAIC_STUDY_ROOT/pilot/scores.csv" \
  > "$MOSAIC_STUDY_ROOT/pilot/score-command.json"
```

O scorer valida cada record, resolve a tentativa terminal, verifica a cadeia de
hashes, deriva o trace e avalia o world/tool/delivery oracle. Assertions não
podem ser injetadas pelo chamador.

### 9.4 Selecionar baseline e calcular o p95

O baseline é escolhido somente entre B0–B3:

1. maior taxa de sucesso;
2. em empate, menor custo médio incluindo falhas;
3. em novo empate, menor ID lexical.

O limite de sensibilidade é o p95 nearest-rank de `modelCalls` em todas as 300
linhas do baseline escolhido, incluindo falhas.

A CLI ainda não possui um comando de resumo do piloto. Calcule esses valores
com um script versionado e auditável usando `selectBaseline` e
`modelCallBudgetAtP95`, exportados pelo pacote, ou reproduza exatamente as
regras acima. O comando `freeze` recalcula os dois valores a partir das 1.800
linhas e rejeita qualquer divergência; portanto, ele é o gate definitivo.

Este comando read-only reproduz as duas regras diretamente sobre
`pilot/scores.json` e grava um resumo operacional:

```sh
MOSAIC_PILOT_SCORE_FILE="$MOSAIC_STUDY_ROOT/pilot/scores.json" \
  node --input-type=module > "$MOSAIC_STUDY_ROOT/pilot/selection.json" <<'NODE'
import { readFile } from 'node:fs/promises';

const path = process.env.MOSAIC_PILOT_SCORE_FILE;
if (!path) throw new Error('MOSAIC_PILOT_SCORE_FILE is required');
const rows = JSON.parse(await readFile(path, 'utf8'));
const ids = ['B0', 'B1', 'B2', 'B3'];
const summaries = ids.map((conditionId) => {
  const selected = rows.filter((row) => row.conditionId === conditionId);
  if (selected.length === 0) throw new Error(`missing ${conditionId}`);
  return {
    conditionId,
    successRate:
      selected.reduce((sum, row) => sum + row.success, 0) / selected.length,
    meanCostUsd:
      selected.reduce((sum, row) => sum + row.costUsd, 0) / selected.length,
    runs: selected.length,
  };
});
summaries.sort(
  (left, right) =>
    right.successRate - left.successRate ||
    left.meanCostUsd - right.meanCostUsd ||
    left.conditionId.localeCompare(right.conditionId),
);
const selectedBaseline = summaries[0].conditionId;
const calls = rows
  .filter(
    (row) =>
      row.phase === 'pilot' && row.conditionId === selectedBaseline,
  )
  .map((row) => row.modelCalls)
  .sort((left, right) => left - right);
const modelCallBudgetP95 = calls[Math.ceil(calls.length * 0.95) - 1];
process.stdout.write(
  `${JSON.stringify(
    { summaries, selectedBaseline, modelCallBudgetP95 },
    null,
    2,
  )}\n`,
);
NODE
```

Confira que cada baseline tem 300 runs. Esse resumo ajuda a preencher o freeze,
mas não substitui a validação do comando `freeze`.

Preserve:

- o ID do baseline escolhido;
- a taxa de sucesso e custo médio de B0–B3;
- o `modelCallBudgetP95`;
- `scores.json` e `scores.csv` completos, inclusive falhas.

## 10. Calibração da segunda família

O primeiro candidato configurado é:

```json
{
  "provider": "openrouter",
  "model": "qwen/qwen3.7-flash",
  "effort": "medium"
}
```

### 10.1 Autorar os casos

Escreva exatamente 60 drafts semânticos independentes e compile-os em
`inputs/calibration-cases.json` com `scripts/cases.mjs compile`. Todos devem
usar `phase: "calibration"` e o resultado deve ser válido contra
`schemas/v1/CaseV1.schema.json`. Eles não podem reutilizar famílias nem
conteúdo do piloto. A validação também exige a distribuição do instrumento:

- 15 casos por domínio;
- 10 casos por classe A–F;
- 60 IDs e 60 `familyId` únicos;
- `contentHash`, evidência de tools e world hash reproduzíveis;
- documento JSON canônico e fields de delivery ligados a cada critério por
  `evidenceRefs`.

Valide o arquivo compilado sem chamadas pagas:

```sh
node benchmarks/mosaic/scripts/cases.mjs validate \
  --calibration-cases inputs/calibration-cases.json
```

Faça auditoria humana antes de observar os resultados para verificar
neutralidade entre famílias de modelo e ausência de duplicatas semânticas. A
validação automática detecta violações estruturais e clones exatos, não
certifica neutralidade, dificuldade ou independência semântica.

### 10.2 Gerar dois schedules pareados

Não há gerador de calibração na CLI. Use a API pública `study.createSchedule`
com a condição `conditions.M1`, três repetições, captura `structure` e os 60
casos. O schedule Luna usa o modelo primário. Gere o schedule candidato como
transformação determinística do mesmo schedule, preservando `caseId`,
`conditionId`, `repetition`, `seed`, `pairedBlock` e `order`, mas usando:

- outro `id` determinístico por run;
- o model config do candidato;
- `phase: "calibration"`;
- `freezeHash: null`;
- nenhum `modelCallBudget`.

Valide os dois arrays contra `schemas/v1/RunSpecV1.schema.json`. Cada um deve
ter 180 runs M1 e os pares Luna/candidato devem compartilhar
`<caseId>.rep.<1..3>`.

Essa preparação deve ser feita por um script preservado com o estudo. Não
duplique ou edite seeds manualmente.

No contrato V1, `ScheduleInput.seed` determina somente a ordem das condições
em cada bloco e deriva o `RunSpec.seed` consumido por hooks determinísticos
(por exemplo, o shuffle de A2). Nenhuma dessas seeds é enviada ao provider ou
controla sampling do modelo. O pareamento estatístico é definido por
`caseId`, `repetition` e `pairedBlock`; não pressuponha respostas modelares
idênticas entre condições ou reruns.

### 10.3 Executar e pontuar Luna

```sh
node "$MOSAIC_CLI" run \
  --schedule "$MOSAIC_STUDY_ROOT/calibration/luna/schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/calibration-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/calibration/luna/artifacts" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  > "$MOSAIC_STUDY_ROOT/calibration/luna/run-summary.json"

node "$MOSAIC_CLI" score \
  --schedule "$MOSAIC_STUDY_ROOT/calibration/luna/schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/calibration-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/calibration/luna/artifacts" \
  --family exploratory \
  --output "$MOSAIC_STUDY_ROOT/calibration/luna/scores.json" \
  --csv "$MOSAIC_STUDY_ROOT/calibration/luna/scores.csv" \
  > "$MOSAIC_STUDY_ROOT/calibration/luna/score-command.json"
```

Use `--resume` nas mesmas condições descritas para o piloto, se necessário.

### 10.4 Executar e pontuar o candidato

```sh
node "$MOSAIC_CLI" run \
  --schedule "$MOSAIC_STUDY_ROOT/calibration/candidate/schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/calibration-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/calibration/candidate/artifacts" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  > "$MOSAIC_STUDY_ROOT/calibration/candidate/run-summary.json"

node "$MOSAIC_CLI" score \
  --schedule "$MOSAIC_STUDY_ROOT/calibration/candidate/schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/calibration-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/calibration/candidate/artifacts" \
  --family exploratory \
  --output "$MOSAIC_STUDY_ROOT/calibration/candidate/scores.json" \
  --csv "$MOSAIC_STUDY_ROOT/calibration/candidate/scores.csv" \
  > "$MOSAIC_STUDY_ROOT/calibration/candidate/score-command.json"
```

### 10.5 Criar o artefato de calibração

Crie `inputs/calibration-input.json` com os arrays completos de scores, não com
observações autoradas manualmente:

```json
{
  "studyId": "mosaic-study-001",
  "candidate": {
    "provider": "openrouter",
    "model": "qwen/qwen3.7-flash",
    "effort": "medium"
  },
  "seed": "calibration-bootstrap-seed-001",
  "cases": [],
  "lunaRows": [],
  "candidateRows": []
}
```

Substitua os arrays vazios pelos conteúdos de `calibration-cases.json` e dos
dois `scores.json`. Então execute:

```sh
node "$MOSAIC_CLI" calibrate-models \
  --input "$MOSAIC_STUDY_ROOT/inputs/calibration-input.json" \
  --output "$MOSAIC_STUDY_ROOT/calibration/calibration.json" \
  > "$MOSAIC_STUDY_ROOT/calibration/command.json"
```

O artefato contém 180 observações pareadas, diferença de sucesso e IC bootstrap
de 95% com 10.000 amostras. O candidato é aprovado somente quando todo o IC
fica em `[-0.05, +0.05]`. Se `approved` for falso, o freeze é bloqueado. Teste
outro candidato com o mesmo protocolo; não force `approved: true`.

## 11. Análise de poder

Crie `inputs/power-config.json`. O arquivo de fixture do repositório é um
ponto de partida estrutural:

```json
{
  "simulations": 10000,
  "seed": "L'Ecuyer-CMRG:104729",
  "numericSeed": 104729,
  "minimumEffect": 0.1,
  "minimumCases": 24,
  "maximumCases": 960,
  "baselineProbability": 0.45,
  "randomInterceptSd": 0.7,
  "adaptiveClasses": ["E", "F"]
}
```

Defina `baselineProbability` e `randomInterceptSd` de forma justificada e
registre essa decisão antes dos resultados confirmatórios. `seed` deve ser
exatamente `L'Ecuyer-CMRG:<numericSeed>`. O simulador avalia múltiplos de 24,
faz 10.000 simulações por candidato e escolhe o primeiro com poder de pelo
menos 80%.

Execute o power por último entre as tarefas preparatórias longas. Dependendo
de `maximumCases` e da máquina, ele pode levar muitas horas:

```sh
node "$MOSAIC_CLI" power \
  --config "$MOSAIC_STUDY_ROOT/inputs/power-config.json" \
  --image "$MOSAIC_ANALYSIS_IMAGE" \
  --work-dir "$MOSAIC_STUDY_ROOT/power/verification" \
  --result "$MOSAIC_STUDY_ROOT/power/power-result.json" \
  > "$MOSAIC_STUDY_ROOT/power/command.json"
```

O comando executa a simulação duas vezes, sem rede, e só publica o resultado se
os bytes forem idênticos. Preserve a pasta de verificação e `SHA256SUMS`.

O resultado contém:

- `nPower`: primeiro múltiplo de 24 com poder >= 0,80;
- `nFinal`: `max(240, nPower)` arredondado para o próximo múltiplo de 120;
- poder estimado;
- seed, parâmetros e hash dos bytes de `power-config.json`.

## 12. Casos confirmatórios

### 12.1 Autoria

Depois de conhecer `nFinal`, autorar exatamente esse número de casos em
`inputs/confirmatory-cases.json`. Todos devem usar `phase: "confirmatory"` e
passar `CaseV1`.

Requisitos principais:

- `nFinal >= 240` e múltiplo de 120;
- exatamente `nFinal / 24` casos em cada célula classe A–F × quatro domínios;
- IDs e `familyId` únicos;
- nenhuma família ou clone estrutural do piloto;
- tool evidence e `worldHash` determinísticos;
- delivery determinístico e não vazio;
- referências válidas ao catálogo e às 24 tools;
- autoria independente dos resultados do piloto, além das escolhas permitidas
  de baseline e tamanho amostral.

Audite humanamente vazamento semântico, ambiguidade do gold e neutralidade. O
validator não substitui essa auditoria.

### 12.2 Validar o corpus

Substitua `N_FINAL` pelo valor do power result:

```sh
MOSAIC_N_FINAL='SUBSTITUA'

node "$MOSAIC_CLI" validate \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --n-final "$MOSAIC_N_FINAL" \
  > "$MOSAIC_STUDY_ROOT/inputs/confirmatory-validation.json"
```

Exija `result.valid: true` e `result.issues: []`.

### 12.3 Gerar o schedule pré-freeze

Não existe comando confirmatório pronto. Use um gerador determinístico que
importe:

- `study.createSchedule`;
- `conditions.conditionById` para o baseline selecionado;
- `conditions.M1`;
- `CaseV1` para validar os casos;
- `RunSpecV1` para validar a saída.

Parâmetros obrigatórios:

```text
studyId       = o mesmo ID usado no piloto
cases         = confirmatory-cases.json
conditions    = [baseline selecionado, M1]
seed          = uma seed confirmatória registrada
repetitions   = 5
capture       = structure
freezeHash    = null
```

O resultado deve ter `nFinal × 2 × 5` runs, sempre com modelo
`openai/gpt-5.6-luna` e esforço `medium`. Preserve o script gerador e sua seed.

Salve a saída como:

```text
$MOSAIC_STUDY_ROOT/inputs/confirmatory-schedule.prefreeze.json
```

Valide o schema e obtenha o hash do ledger de seeds:

```sh
node "$MOSAIC_CLI" validate \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/confirmatory-schedule.prefreeze.json" \
  > "$MOSAIC_STUDY_ROOT/inputs/schedule-validation.json"
```

`validate --schedule` calcula o hash, mas o gate completo de cardinalidade,
pareamento, condições, modelo e ordem ocorre no comando `freeze`.

## 13. Preparar e gravar o freeze

### 13.1 Coletar os hashes

Execute uma validação agregada:

```sh
node "$MOSAIC_CLI" validate \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --n-final "$MOSAIC_N_FINAL" \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/confirmatory-schedule.prefreeze.json" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  --pilot-scores "$MOSAIC_STUDY_ROOT/pilot/scores.json" \
  --calibration "$MOSAIC_STUDY_ROOT/calibration/calibration.json" \
  --calibration-audit "$MOSAIC_STUDY_ROOT/inputs/calibration-audit.json" \
  --confirmatory-audit "$MOSAIC_STUDY_ROOT/inputs/confirmatory-audit.json" \
  --cost-approval "$MOSAIC_STUDY_ROOT/inputs/cost-approval.json" \
  --power-config "$MOSAIC_STUDY_ROOT/inputs/power-config.json" \
  --power-approval "$MOSAIC_STUDY_ROOT/inputs/power-approval.json" \
  --power-result "$MOSAIC_STUDY_ROOT/power/power-result.json" \
  > "$MOSAIC_STUDY_ROOT/freeze/validate-before-freeze.json"
```

Copie os hashes de `result.artifactHashes` para o freeze input. Não recalcule
esses valores com ferramentas diferentes: alguns são hashes de JSON canônico,
outros são hashes dos bytes exatos do arquivo.

### 13.2 Deixar o repositório limpo

Todos os arquivos de implementação que participarão do estudo precisam estar
commitados antes do freeze:

```sh
cd "$MOSAIC_REPO_ROOT"
git status --short
git rev-parse HEAD
```

`git status --short` precisa estar vazio. Registre o hash exato de HEAD no
freeze input. Não use `git stash`, reset ou exclusões ad hoc para esconder
mudanças relevantes; resolva e registre o estado intencionalmente.

### 13.3 Freeze input

Crie `freeze/freeze-input.json`. O template abaixo não é executável até todos
os placeholders serem substituídos. Ele não deve conter `manifestHash`, pois a
CLI o calcula:

```json
{
  "schemaVersion": 1,
  "studyId": "mosaic-study-001",
  "frozenAt": "2026-08-08T00:00:00.000Z",
  "gitCommit": "SUBSTITUA_PELO_HEAD_LIMPO",
  "cleanWorktree": true,
  "primaryModel": {
    "provider": "openai",
    "model": "openai/gpt-5.6-luna",
    "effort": "medium"
  },
  "reranker": "voyageai/rerank-2.5-lite",
  "embedder": {
    "model": "voyageai/voyage-4-large",
    "dimensions": 2048
  },
  "selectedBaseline": "B2",
  "repetitions": 5,
  "alpha": 0.05,
  "minimumEffect": 0.1,
  "targetPower": 0.8,
  "nPower": 264,
  "nFinal": 360,
  "modelCallBudgetP95": 7,
  "semanticReviewFraction": 0.2,
  "artifactHashes": {
    "protocol": "sha256:SUBSTITUA",
    "schemas": "sha256:SUBSTITUA",
    "catalog": "sha256:SUBSTITUA",
    "tools": "sha256:SUBSTITUA",
    "pilotCases": "sha256:SUBSTITUA",
    "pilotScores": "sha256:SUBSTITUA",
    "confirmatoryCases": "sha256:SUBSTITUA",
    "conditions": "sha256:SUBSTITUA",
    "prompts": "sha256:SUBSTITUA",
    "prices": "sha256:SUBSTITUA",
    "seeds": "sha256:SUBSTITUA",
    "calibration": "sha256:SUBSTITUA",
    "calibrationAudit": "sha256:SUBSTITUA",
    "confirmatoryAudit": "sha256:SUBSTITUA",
    "costApproval": "sha256:SUBSTITUA",
    "powerConfig": "sha256:SUBSTITUA",
    "powerApproval": "sha256:SUBSTITUA",
    "powerResult": "sha256:SUBSTITUA",
    "retrievalIndex": "sha256:SUBSTITUA",
    "analysis": "sha256:SUBSTITUA",
    "renvLock": "sha256:SUBSTITUA"
  },
  "replication": {
    "candidate": {
      "provider": "openrouter",
      "model": "qwen/qwen3.7-flash",
      "effort": "medium"
    },
    "neutralCases": 60,
    "repetitions": 3,
    "difference": 0,
    "confidence95": {
      "lower": -0.04,
      "upper": 0.04
    },
    "approved": true
  },
  "containerDigest": "sha256:SUBSTITUA"
}
```

Substitua `selectedBaseline`, `modelCallBudgetP95`, `nPower`, `nFinal` e todo o
objeto `replication` pelos valores realmente produzidos. A CLI recomputa o
piloto, a calibração, o power, os casos, o schedule, os preços e os hashes
locais; placeholders ou valores forjados serão rejeitados.

### 13.4 Criar o freeze uma única vez

```sh
node "$MOSAIC_CLI" freeze \
  --input "$MOSAIC_STUDY_ROOT/freeze/freeze-input.json" \
  --path "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  --pilot-scores "$MOSAIC_STUDY_ROOT/pilot/scores.json" \
  --calibration "$MOSAIC_STUDY_ROOT/calibration/calibration.json" \
  --calibration-audit "$MOSAIC_STUDY_ROOT/inputs/calibration-audit.json" \
  --confirmatory-audit "$MOSAIC_STUDY_ROOT/inputs/confirmatory-audit.json" \
  --cost-approval "$MOSAIC_STUDY_ROOT/inputs/cost-approval.json" \
  --power-config "$MOSAIC_STUDY_ROOT/inputs/power-config.json" \
  --power-approval "$MOSAIC_STUDY_ROOT/inputs/power-approval.json" \
  --power-result "$MOSAIC_STUDY_ROOT/power/power-result.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/confirmatory-schedule.prefreeze.json" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  --image "$MOSAIC_ANALYSIS_IMAGE" \
  > "$MOSAIC_STUDY_ROOT/freeze/command.json"
```

O freeze falhará se:

- o worktree não estiver limpo ou HEAD diferir de `gitCommit`;
- o baseline/p95 não corresponder ao piloto completo;
- a calibração não estiver aprovada e reproduzível;
- power config/result não corresponderem;
- `nFinal` ou os casos estiverem incorretos;
- o schedule não for a grade pareada completa;
- preços ou hashes locais divergirem;
- a imagem por digest não estiver disponível localmente.

Faça cópia segura de `freeze.json`. Não o regenere com outro timestamp para o
mesmo estudo.

## 14. Materializar os schedules congelados

Leia `manifestHash` de `freeze.json` e crie três schedules distintos por uma
transformação determinística validada com `RunSpecV1`.

### Schedule primário

Parta do schedule pré-freeze e altere somente:

```text
freezeHash = freeze.manifestHash
```

Preserve model, phase, conditions, seeds, blocos e ordem. Salve como
`inputs/confirmatory-schedule.json`.

### Schedule de replicação

Use a mesma grade pareada e preserve os campos que compõem o seed ledger:
`studyId`, `caseId`, `conditionId`, `repetition`, `seed`, `pairedBlock` e
`order`. Nesse ledger, `seed` significa exclusivamente a seed de hooks
determinísticos; sampling do provider permanece não semeado. Defina:

```text
phase          = replication
model          = freeze.replication.candidate
capture        = structure
freezeHash     = freeze.manifestHash
modelCallBudget = ausente
```

Gere IDs determinísticos distintos dos IDs primários para evitar colisões
quando os datasets forem arquivados. Salve em `inputs/replication-schedule.json`.
O arquivo de casos continua sendo o corpus confirmatório congelado; não crie
uma cópia com `phase: replication`, pois isso mudaria seu hash.

### Schedule de sensibilidade

Parta do schedule primário e defina:

```text
phase           = confirmatory
model           = freeze.primaryModel
modelCallBudget = freeze.modelCallBudgetP95
```

Use IDs determinísticos distintos e salve em
`inputs/sensitivity-schedule.json`.

Mantenha cada família em seu próprio artifact root. Nunca misture schedules
primário, replicação e sensibilidade na mesma chamada `run`.

Antes de gastar, confira em cada schedule:

- comprimento igual a `nFinal × 10`;
- exatamente baseline + M1;
- cinco repetições por caso/condição;
- um `pairedBlock` e seed compartilhados pelo par de cada repetição;
- `order` cobrindo `0..length-1` sem repetição;
- captura `structure`;
- `freezeHash` exato.

## 15. Executar o estudo primário

### 15.1 Run pago

O repositório ainda precisa estar no commit limpo do freeze:

```sh
cd "$MOSAIC_REPO_ROOT"
git status --short
git rev-parse HEAD
```

Então execute:

```sh
node "$MOSAIC_CLI" run \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/confirmatory-schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/primary/artifacts" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  --freeze "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  > "$MOSAIC_STUDY_ROOT/primary/run-summary.json"
```

Se houver interrupção, repita o mesmo comando com `--resume` e outro arquivo de
stdout. Não remova records ou traces existentes.

### 15.2 Score primário

```sh
node "$MOSAIC_CLI" score \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/confirmatory-schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/primary/artifacts" \
  --freeze "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  --family primary \
  --output "$MOSAIC_STUDY_ROOT/primary/scores.json" \
  --csv "$MOSAIC_STUDY_ROOT/primary/scores.csv" \
  > "$MOSAIC_STUDY_ROOT/primary/score-command.json"
```

O resultado precisa ter exatamente `nFinal × 10` linhas. Outcomes de
infraestrutura permanecem no arquivo com `success = 0`; não os filtre.

## 16. Executar a replicação

Execute e pontue em uma pasta separada:

```sh
node "$MOSAIC_CLI" run \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/replication-schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/replication/artifacts" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  --freeze "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  > "$MOSAIC_STUDY_ROOT/replication/run-summary.json"

node "$MOSAIC_CLI" score \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/replication-schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/replication/artifacts" \
  --freeze "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  --family replication \
  --output "$MOSAIC_STUDY_ROOT/replication/scores.json" \
  --csv "$MOSAIC_STUDY_ROOT/replication/scores.csv" \
  > "$MOSAIC_STUDY_ROOT/replication/score-command.json"
```

Não combine o CSV de replicação com o CSV primário para ajustar um único
modelo.

## 17. Executar a sensibilidade de orçamento

Esta execução aplica o limite p95 do baseline piloto e não substitui o
resultado primário sem cap:

```sh
node "$MOSAIC_CLI" run \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/sensitivity-schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/sensitivity/artifacts" \
  --prices "$MOSAIC_STUDY_ROOT/inputs/prices.json" \
  --index "$MOSAIC_INDEX_PATH" \
  --freeze "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  > "$MOSAIC_STUDY_ROOT/sensitivity/run-summary.json"

node "$MOSAIC_CLI" score \
  --schedule "$MOSAIC_STUDY_ROOT/inputs/sensitivity-schedule.json" \
  --cases "$MOSAIC_STUDY_ROOT/inputs/confirmatory-cases.json" \
  --artifacts "$MOSAIC_STUDY_ROOT/sensitivity/artifacts" \
  --freeze "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  --family sensitivity \
  --output "$MOSAIC_STUDY_ROOT/sensitivity/scores.json" \
  --csv "$MOSAIC_STUDY_ROOT/sensitivity/scores.csv" \
  > "$MOSAIC_STUDY_ROOT/sensitivity/score-command.json"
```

## 18. Oracles diagnósticos opcionais

Oracles são diagnósticos e não entram na análise confirmatória. Gere-os apenas
para parents com `status: "failed"`, usando `study.createOracleSchedule`. Cada
run oracle precisa carregar `oracleParentRunId` e deve ser executado em artifact
root separado. Não gere oracles para sucessos ou para outcomes de
infraestrutura.

Condições disponíveis:

- `O_PLAN`;
- `O_RETRIEVAL`;
- `O_BUNDLE`;
- `O_MENU`;
- `O_STATE`;
- `O_REVISION`.

Não acrescente suas linhas ao CSV primário, de replicação ou de sensibilidade.

## 19. Onde os dados ficam e como extraí-los

### Records

Cada tentativa terminal é gravada em:

```text
<artifact-root>/records/<run-id>/001-<record-hash>.json
```

Uma segunda tentativa técnica válida usa prefixo `002-`. O record contém:

- a especificação completa do run;
- status `succeeded`, `failed` ou `infrastructure`;
- timestamps e duração;
- indicação de primeira chamada de modelo;
- referência e hashes do trace;
- outcome final;
- world hash;
- uso de tokens, chamadas, tools e custo;
- falha de infraestrutura sanitizada, quando aplicável.

### Traces

Os eventos append-only ficam em:

```text
<artifact-root>/traces/<run-id>/attempt-001/events/
<artifact-root>/traces/<run-id>/attempt-001/derived/
```

Cada evento possui `sequence`, `previousHash`, `payloadHash` e `eventHash`. O
arquivo derivado é uma visão imutável da tentativa. Não edite, renomeie ou
reordene esses arquivos.

### Score JSON

`scores.json` é o dataset canônico estruturado, um array de `ScoreRowV1`. Use-o
para integração programática, auditoria e preparação de outros artefatos.

### Score CSV

`scores.csv` é a tabela pronta para análise. Suas colunas são:

```text
schemaVersion, runId, attempt, studyId, phase, caseId, familyId,
conditionId, repetition, pairedBlock, provider, model, effort, freezeHash,
traceRootHash, traceDerivedHash, evidenceHash, worldHash, modelCallBudget,
domain, compositionClass, adaptive, success, primaryEligible,
infrastructure, failureCode, retrieval, bundleExact, menuExact,
observationExact, inputTokens, outputTokens, modelCalls, toolCalls, costUsd,
durationMs
```

Use:

- `success` como outcome primário;
- `primaryEligible` para confirmar elegibilidade congelada;
- `infrastructure` e `failureCode` para descrever falhas, sem excluí-las;
- `inputTokens`, `outputTokens`, `modelCalls`, `toolCalls`, `costUsd` e
  `durationMs` apenas como outcomes descritivos;
- `retrieval`, `bundleExact`, `menuExact` e `observationExact` como métricas
  diagnósticas secundárias;
- hashes e `pairedBlock` para provenance e pareamento.

Para uma inspeção descritiva rápida, sem substituir a análise oficial em R:

```sh
MOSAIC_SCORE_FILE="$MOSAIC_STUDY_ROOT/primary/scores.json" \
  node --input-type=module <<'NODE'
import { readFile } from 'node:fs/promises';

const path = process.env.MOSAIC_SCORE_FILE;
if (!path) throw new Error('MOSAIC_SCORE_FILE is required');
const rows = JSON.parse(await readFile(path, 'utf8'));
const conditions = [...new Set(rows.map((row) => row.conditionId))].sort();
const summary = conditions.map((conditionId) => {
  const selected = rows.filter((row) => row.conditionId === conditionId);
  return {
    conditionId,
    runs: selected.length,
    successRate:
      selected.reduce((sum, row) => sum + row.success, 0) / selected.length,
    infrastructure: selected.filter((row) => row.infrastructure).length,
    totalCostUsd: selected.reduce((sum, row) => sum + row.costUsd, 0),
    meanDurationMs:
      selected.reduce((sum, row) => sum + row.durationMs, 0) /
      selected.length,
  };
});
console.log(
  JSON.stringify(
    {
      rows: rows.length,
      cases: new Set(rows.map((row) => row.caseId)).size,
      summary,
    },
    null,
    2,
  ),
);
NODE
```

### Resultado e relatório R

`analysis-result.json` contém:

- hash do dataset e do freeze;
- fórmula e estimador realmente usado;
- taxa de fits válidos;
- número de observações e casos;
- OR e IC 95%;
- diferença absoluta e IC 95%;
- p-value e p-value Holm;
- resultado de power para as famílias primária/replicação;
- warnings e hash final.

`report.md` é a apresentação humana dos mesmos resultados e inclui custos
médios e totais por condição, contando falhas.

### Checagens mínimas após score

Antes de analisar, confirme:

- número de linhas igual a `nFinal × 10`;
- exatamente dois `conditionId`: baseline congelado e M1;
- cinco repetições por caso e condição;
- `nFinal` casos únicos;
- um único modelo, esforço `medium` e freeze hash;
- todas as linhas elegíveis para a família esperada;
- failures e infraestrutura preservados;
- pares com mesmo `pairedBlock` e seed provenance.

O script R repete essas checagens e falha fechado, mas a inspeção operacional
evita descobrir arquivos trocados depois de uma análise longa.

## 20. Análise estatística no container

### 20.1 Configuração primária

Coloque uma cópia física de `power-result.json` no mesmo diretório de
`analysis-config.primary.json`. No JSON, `powerResultPath` deve ser o caminho
interno do container, exatamente `/input/config/power-result.json`.

```sh
cp \
  "$MOSAIC_STUDY_ROOT/power/power-result.json" \
  "$MOSAIC_STUDY_ROOT/inputs/power-result.json"
```

Não reformate essa cópia: o hash validado é o SHA-256 de seus bytes exatos.

Template:

```json
{
  "studyId": "mosaic-study-001",
  "family": "primary",
  "baselineCondition": "B2",
  "provider": "openai",
  "model": "openai/gpt-5.6-luna",
  "effort": "medium",
  "implementationHash": "sha256:SUBSTITUA_PELO_HASH_ANALYSIS",
  "powerResultPath": "/input/config/power-result.json",
  "powerParameters": {
    "seed": "L'Ecuyer-CMRG:104729",
    "numericSeed": 104729,
    "baselineProbability": 0.45,
    "randomInterceptSd": 0.7,
    "adaptiveClasses": ["E", "F"]
  },
  "freeze": {
    "manifestHash": "sha256:SUBSTITUA",
    "nPower": 264,
    "nFinal": 360,
    "repetitions": 5,
    "minimumEffect": 0.1,
    "modelCallBudgetP95": 7,
    "artifactHashes": {
      "analysis": "sha256:SUBSTITUA",
      "powerConfig": "sha256:SUBSTITUA",
      "powerResult": "sha256:SUBSTITUA"
    }
  },
  "bootstrapRepetitions": 10000,
  "bootstrapSeed": 104729
}
```

Todos os campos devem ser copiados do freeze, do power config e do power
result; não use os valores ilustrativos.

Execute:

```sh
node "$MOSAIC_CLI" analyze \
  --scores "$MOSAIC_STUDY_ROOT/primary/scores.csv" \
  --config "$MOSAIC_STUDY_ROOT/inputs/analysis-config.primary.json" \
  --freeze "$MOSAIC_STUDY_ROOT/freeze/freeze.json" \
  --image "$MOSAIC_ANALYSIS_IMAGE" \
  --work-dir "$MOSAIC_STUDY_ROOT/primary/verification" \
  --result "$MOSAIC_STUDY_ROOT/primary/analysis-result.json" \
  --report "$MOSAIC_STUDY_ROOT/primary/report.md" \
  > "$MOSAIC_STUDY_ROOT/primary/analyze-command.json"
```

O comando roda análise e relatório duas vezes em containers sem rede e compara
os bytes. Preserve o diretório `determinism.*` e `SHA256SUMS` retornados.

### 20.2 Replicação

Crie uma configuração separada com:

```text
family   = replication
provider = freeze.replication.candidate.provider
model    = freeze.replication.candidate.model
effort   = medium
```

Os demais campos de freeze/power permanecem idênticos. Então execute `analyze`
contra `replication/scores.csv`, com result, report e work-dir próprios.

### 20.3 Sensibilidade

Crie uma configuração com:

```text
family              = sensitivity
provider/model       = freeze.primaryModel
modelCallBudgetP95  = freeze.modelCallBudgetP95
```

Essa é a única família que declara o campo top-level `modelCallBudgetP95`.
Execute `analyze` contra `sensitivity/scores.csv`. O power result é validado,
mas aparece como `null` no resultado de sensibilidade.

### 20.4 Estimador e fallbacks

O modelo primário é:

```r
glmer(
  success ~ condition * composition_class + (1 | case_id),
  family = binomial(),
  control = glmerControl(
    optimizer = "bobyqa",
    optCtrl = list(maxfun = 200000)
  )
)
```

Fallbacks, nesta ordem:

1. `glmer-bobyqa`;
2. `glmer-nloptwrap`;
3. `glm-hc2-cluster`;
4. `case-bootstrap` com 10.000 amostras;
5. `not-estimable` se menos de 95% dos fits forem válidos.

Não escolha manualmente um estimador diferente depois de ver os resultados.

## 21. Revisão semântica cega

A revisão secundária usa exatamente 20% de cada célula confirmatória, M1 e o
baseline, uma repetição escolhida deterministicamente e duas passagens do mesmo
pesquisador com intervalo mínimo de 14 dias.

### 21.1 Preparar as entregas

Para cada caso, localize no record/trace a entrega terminal da repetição
selecionável para M1 e baseline. Materialize uma representação cega, estável e
sem IDs de condição. Calcule o `artifactHash` segundo o JSON canônico do
benchmark; não use um SHA de bytes arbitrário como substituto.

A CLI ainda não extrai essas entregas. Preserve o script de extração e confirme
que cada arquivo em `review/artifacts/review/<hash-sem-prefixo>` corresponde ao
hash declarado.

### 21.2 Review plan

Crie `review/review-plan.json` com todas as células. Shape:

```json
{
  "studyId": "mosaic-study-001",
  "cases": [
    {
      "caseId": "case.example",
      "cell": "E:software",
      "repetitions": [1, 2, 3, 4, 5],
      "artifacts": {
        "M1": {
          "hash": "sha256:SUBSTITUA"
        },
        "B2": {
          "hash": "sha256:SUBSTITUA"
        }
      }
    }
  ],
  "conditions": ["B2", "M1"],
  "seed": "review-seed-001",
  "rounds": ["2026-09-01T12:00:00.000Z", "2026-09-15T12:00:00.000Z"]
}
```

Cada célula precisa conter um múltiplo positivo de cinco casos. As datas devem
ser instantes ISO canônicos separados por pelo menos 14 dias completos.

### 21.3 Gerar assignments e chave privada

```sh
node "$MOSAIC_CLI" review prepare \
  --input "$MOSAIC_STUDY_ROOT/review/review-plan.json" \
  --assignments "$MOSAIC_STUDY_ROOT/review/assignments.json" \
  --key "$MOSAIC_STUDY_ROOT/review/private-key.json" \
  > "$MOSAIC_STUDY_ROOT/review/prepare-command.json"
```

Mantenha `private-key.json` fora do alcance do reviewer durante as duas
passagens e fora do pacote público cego.

### 21.4 Registrar reviews

Cada review segue `ReviewV1`:

```json
{
  "schemaVersion": 1,
  "assignmentId": "SUBSTITUA",
  "reviewerCode": "solo-reviewer-001",
  "submittedAt": "2026-09-01T13:00:00.000Z",
  "acceptable": true,
  "criteria": [
    {
      "criterionId": "criterion.example",
      "satisfied": true
    }
  ],
  "confidence": 4,
  "notes": "",
  "auxiliaryJudge": null
}
```

O input de `review ingest` exige `assignments` e `key` recombinados em um único
objeto privado. A CLI `prepare` os grava separados, portanto monte localmente:

```json
{
  "prepared": {
    "schemaVersion": 1,
    "assignments": [],
    "key": []
  },
  "existing": [],
  "incoming": {}
}
```

Preencha `assignments` com o array do arquivo público, `key` com o array da
chave privada, `existing` com reviews já ingeridos e `incoming` com exatamente
um novo `ReviewV1`. Execute:

```sh
node "$MOSAIC_CLI" review ingest \
  --input "$MOSAIC_STUDY_ROOT/review/review-ingest.json" \
  --output "$MOSAIC_STUDY_ROOT/review/reviews.next.json" \
  > "$MOSAIC_STUDY_ROOT/review/ingest-command.json"
```

Como a escrita é exclusiva, use um novo arquivo a cada ingestão e promova a
versão final de forma auditável; não sobrescreva versões anteriores.

### 21.5 Status e estabilidade

O input de status tem o mesmo objeto `prepared` combinado e o array final de
reviews:

```json
{
  "prepared": {
    "schemaVersion": 1,
    "assignments": [],
    "key": []
  },
  "reviews": []
}
```

```sh
node "$MOSAIC_CLI" review status \
  --input "$MOSAIC_STUDY_ROOT/review/review-status-input.json" \
  > "$MOSAIC_STUDY_ROOT/review/status.json"
```

O status informa assignments, passagens completas, Cohen's kappa e
`judgeExtrapolationAllowed`. O judge auxiliar `openai/gpt-5.6-sol` nunca
substitui o humano. Sua extrapolação só é permitida quando todas as avaliações
estão completas e a estabilidade humana é estimável e >= 0,80.

## 22. Empacotar e arquivar

O comando `package` copia apenas arquivos regulares explicitamente listados,
recusa symlinks, preserva a árvore relativa e não sobrescreve destino.

Crie um plano:

```json
{
  "root": "/absolute/path/to/mosaic-study-001",
  "destination": "/absolute/path/to/mosaic-study-001/package/publication",
  "relativePaths": [
    "freeze/freeze.json",
    "inputs/prices.json",
    "inputs/power-config.json",
    "inputs/confirmatory-cases.json",
    "inputs/confirmatory-schedule.json",
    "power/power-result.json",
    "pilot/scores.json",
    "pilot/scores.csv",
    "primary/scores.json",
    "primary/scores.csv",
    "primary/analysis-result.json",
    "primary/report.md",
    "replication/scores.json",
    "replication/scores.csv",
    "replication/analysis-result.json",
    "replication/report.md",
    "review/assignments.json",
    "review/reviews.json",
    "review/status.json"
  ]
}
```

Inclua também, conforme o protocolo de publicação:

- schedules de calibração, replicação e sensibilidade;
- calibration cases/input/result;
- análise configs e diretórios `SHA256SUMS`;
- records e derived traces;
- eventos brutos, se o pacote completo os exigir;
- scripts de autoria, schedule, extração e materialização de review;
- hashes ou cópias dos schemas, catálogo, prompts e protocolo congelados;
- referência completa da imagem OCI por digest.

Não inclua:

- `OPENROUTER_API_KEY` ou qualquer segredo;
- `review/private-key.json` no pacote público antes de concluir e abrir a
  análise cega;
- arquivos temporários ou uma versão editada dos traces.

Execute:

```sh
node "$MOSAIC_CLI" package \
  --input "$MOSAIC_STUDY_ROOT/package/package-plan.json" \
  > "$MOSAIC_STUDY_ROOT/package/package-result.json"
```

Guarde `packageHash` e o mapa de hashes por arquivo. Como `relativePaths` é
explícito, enumere e audite records/traces antes de considerar o pacote
completo.

## 23. Checklist final

### Antes de qualquer run pago

- [ ] `nx sync`, typecheck, build e testes passaram.
- [ ] Schemas foram regenerados e estão sem diff inesperado.
- [ ] `validate` e `conformance` estão verdes.
- [ ] Container R foi construído e `test-analysis.R` passou sem rede.
- [ ] Imagem está disponível por digest imutável.
- [ ] Preços reais de todos os modelos estão registrados.
- [ ] API key está apenas no ambiente.
- [ ] Schedule foi contado e custo máximo revisado.
- [ ] Artifact root é novo ou a retomada usa `--resume` corretamente.

### Antes do freeze

- [ ] Piloto tem 1.800 score rows completos.
- [ ] Baseline e p95 foram derivados pela regra congelada.
- [ ] Calibração tem 60 casos × 3 repetições × 2 modelos e está aprovada.
- [ ] Power executou duas vezes com bytes idênticos.
- [ ] Casos confirmatórios têm `nFinal` e matriz 6 × 4 balanceada.
- [ ] Schedule pré-freeze tem `nFinal × 10` runs pareados.
- [ ] Auditoria humana de casos/catálogo foi concluída.
- [ ] Todos os hashes vieram de `validate` ou dos comandos oficiais.
- [ ] Imagem por digest está disponível localmente.
- [ ] Worktree está limpo e `gitCommit` corresponde a HEAD.

### Depois de cada run

- [ ] stdout registrou `ok: true`.
- [ ] Não existem runs sem tentativa terminal antes de `score`.
- [ ] `score` produziu JSON e CSV com a cardinalidade esperada.
- [ ] Falhas e infraestrutura permanecem no dataset.
- [ ] Records e traces não foram alterados.
- [ ] Hashes de freeze, trace e evidence estão presentes.

### Antes de publicar

- [ ] Primário e replicação foram analisados separadamente.
- [ ] Duas execuções R produziram bytes idênticos.
- [ ] Resultados, reports e `SHA256SUMS` foram preservados.
- [ ] Revisão cega respeitou 20%, uma repetição e intervalo de 14 dias.
- [ ] Chave privada não vazou durante a avaliação.
- [ ] Pacote contém inputs, outputs, provenance e scripts necessários.
- [ ] Pacote não contém credenciais.

## 24. Troubleshooting

### `Rscript is unavailable`

Isso só bloqueia `mosaic-benchmark:analysis-test` no host. Construa a imagem e
execute `test-analysis.R` pelo container. Os comandos `power` e `analyze`
também usam Docker, não o `Rscript` do host.

### Build morre durante `RcppEigen`

Verifique a memória do daemon Docker. Em ambiente com 2 GiB, aumente para pelo
menos 6 GiB e reconstrua. Não altere versões do `renv.lock` para contornar falta
de memória.

### Arquivo de saída já existe

É uma proteção de imutabilidade. Não apague um resultado material para repetir
silenciosamente. Use um novo caminho e documente a razão; para runs
interrompidos, use o mesmo artifact root com `--resume`.

### `existing attempts require --resume`

O artifact root já contém uma tentativa do schedule. Repita com `--resume`.
Somente falha técnica anterior à primeira chamada pode ser tentada novamente.

### Freeze diz que o worktree está sujo

O freeze exige `git status --porcelain --untracked-files=all` vazio e HEAD
igual ao commit declarado. Commit as mudanças intencionais e remova do
repositório apenas artefatos realmente externos/gerados segundo a política do
projeto. Não esconda mudanças relevantes.

### Freeze rejeita baseline ou p95

Recalcule sobre todas as linhas B0–B3 do piloto, incluindo falhas e custos. Não
filtre por sucesso nem por infraestrutura. O p95 usa todas as 300 linhas do
baseline selecionado.

### Freeze rejeita schedule

Confira cardinalidade, baseline + M1, cinco repetições, pareamento de seed e
`pairedBlock`, ordens contíguas, modelo primário, captura `structure`, ausência
de cap e `freezeHash: null` no schedule pré-freeze.

### Run congelado rejeita o schedule

Confira:

- worktree e commit do freeze;
- `freezeHash` em cada run;
- hash dos casos e preços;
- model/phase corretos para primário ou replicação;
- cap ausente no primário/replicação e exato na sensibilidade;
- separação entre artifact roots.

### `score` informa `run has no terminal attempt`

O schedule não terminou ou ainda permite uma tentativa técnica. Retome o run
com `--resume`. Não fabrique um record nem remova a tentativa parcial.

### `powerResultPath` é rejeitado

No analysis config use `/input/config/power-result.json`, mas mantenha o arquivo
físico `power-result.json` no mesmo diretório do config no host. A CLI monta
esse diretório como `/input/config`.

### A análise é `not-estimable`

Esse é um resultado protocolar possível. Se menos de 95% dos fits forem
válidos após todos os fallbacks congelados, reporte “não estimável”. Não mude o
modelo, remova casos ou escolha outro fallback pós-hoc.

## 25. Referências locais

- Visão geral e CLI: `benchmarks/mosaic/README.md`.
- Decisões D01–D25: `benchmarks/mosaic/protocol/D01-D25.md`.
- Protocolo estatístico: `benchmarks/mosaic/analysis/PROTOCOL.md`.
- Protocolo de review: `benchmarks/mosaic/analysis/REVIEW.md`.
- Ambiente R: `benchmarks/mosaic/analysis/renv.lock`.
- Container: `benchmarks/mosaic/container/Dockerfile`.
- Schemas JSON: `benchmarks/mosaic/schemas/v1/`.
- Contratos TypeScript: `benchmarks/mosaic/src/schemas/`.
- CLI: `benchmarks/mosaic/src/cli/`.
- Store de records/traces: `benchmarks/mosaic/src/runtime/store.ts`.
- Scheduler: `benchmarks/mosaic/src/study/scheduler.ts`.
- Freeze e validação: `benchmarks/mosaic/src/cli/freeze.ts`.
- Scoring: `benchmarks/mosaic/src/cli/score.ts`.
- Análise no container: `benchmarks/mosaic/src/cli/analysis.ts`.
