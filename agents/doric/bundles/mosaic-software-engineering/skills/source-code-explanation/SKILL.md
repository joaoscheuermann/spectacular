---
name: source-code-explanation
description: "Explica como um comportamento é implementado seguindo módulos, funções, dados e chamadas relevantes a partir de um ponto de entrada."
allowed-tools:
  - repository_tree
  - read
  - search_text
---

# Source Code Explanation

## Purpose

Produzir uma explicação técnica rastreável do fluxo de execução sem converter cada arquivo em um objetivo separado.

## Use this skill when

- o usuário pergunta como uma feature, fluxo ou módulo funciona.
- é necessário seguir chamadas e transformações entre arquivos.
- o código precisa ser explicado para manutenção ou onboarding.

## Do not use this skill when

- o objetivo é diagnosticar uma falha observável.
- o pedido exige modificar o código.
- é necessário mapear dependências externas e impacto de mudança, não explicar o fluxo atual.

## Procedure

1. Localize o ponto de entrada por estrutura, rota, símbolo ou chamada.
2. Leia a implementação e use search_text para seguir símbolos relevantes.
3. Rastreie entradas, transformações, estado, efeitos e saídas.
4. Separe comportamento garantido pelo código de suposições sobre runtime.
5. Ignore detalhes que não alteram o fluxo explicado.
6. Inclua referências de arquivo e símbolo para cada etapa material.

## Completion

A explicação descreve o fluxo de ponta a ponta, aponta arquivos e símbolos e distingue fatos do código de inferências.
