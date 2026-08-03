---
name: implementation-planning
description: "Transforma um resultado técnico em etapas de implementação orientadas a resultados, com dependências, decisões e critérios de verificação."
allowed-tools:
  - repository_tree
  - read
  - search_text
  - dependency_graph
---

# Implementation Planning

## Purpose

Planejar uma mudança sem reduzir o plano a uma lista de chamadas ou arquivos e sem começar a editar prematuramente.

## Use this skill when

- o usuário pede plano de implementação, abordagem ou sequência de mudança.
- a solução envolve múltiplos resultados intermediários.
- é necessário entender arquitetura e dependências antes de codificar.

## Do not use this skill when

- o pedido exige aplicar imediatamente uma alteração pequena e inequívoca.
- não há contexto suficiente para localizar o sistema afetado.
- o objetivo é revisar um patch já produzido.

## Procedure

1. Defina o comportamento final e os critérios observáveis.
2. Mapeie componentes e dependências relevantes.
3. Identifique decisões de contrato, dados e migração que precisam ocorrer antes da edição.
4. Divida o trabalho por resultados verificáveis, não por tools ou arquivos isolados.
5. Inclua estratégia de teste e compatibilidade.
6. Registre riscos técnicos somente quando alterarem a sequência ou os critérios.

## Completion

O plano contém resultados intermediários, dependências, decisões e verificações suficientes para orientar a implementação sem ser uma trajetória mecânica.
