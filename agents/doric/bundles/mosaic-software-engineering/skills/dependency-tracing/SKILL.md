---
name: dependency-tracing
description: "Rastreia dependências diretas e transitivas entre módulos, pacotes ou serviços, distinguindo quem consome e quem é consumido."
allowed-tools:
  - dependency_graph
  - search_text
  - read
---

# Dependency Tracing

## Purpose

Determinar relações de dependência relevantes para compreensão, impacto ou ordem de execução sem confundir proximidade textual com acoplamento.

## Use this skill when

- o usuário pergunta de que um módulo depende ou quem depende dele.
- uma mudança pode afetar consumidores transitivos.
- há imports, injeção, eventos ou contratos compartilhados a seguir.

## Do not use this skill when

- o objetivo é explicar apenas o fluxo interno de uma função.
- a dependência já está fornecida em estrutura confiável.
- o pedido é escrever um patch sem necessidade de análise de impacto.

## Procedure

1. Defina o nó inicial e a direção: upstream, downstream ou ambas.
2. Use dependency_graph para obter relações estruturais.
3. Use search_text e read para confirmar dependências dinâmicas, eventos ou registries que o grafo não capture.
4. Diferencie dependência de build, runtime, teste e documentação.
5. Limite a profundidade ao que pode alterar o objetivo.
6. Marque relações incertas e ciclos relevantes.

## Completion

As dependências materiais, sua direção, tipo e profundidade estão mapeadas e confirmadas por evidência de código.
