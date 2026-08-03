---
name: bug-root-cause-analysis
description: "Investiga uma falha observável por hipóteses testáveis, histórico e evidência de execução até identificar a causa mais específica sustentada."
allowed-tools:
  - read
  - search_text
  - git_history
  - run_tests
---

# Bug Root Cause Analysis

## Purpose

Evitar correções por palpite e distinguir sintoma, mecanismo e causa raiz.

## Use this skill when

- há erro reproduzível, comportamento incorreto ou regressão.
- o usuário pede causa, diagnóstico ou explicação do defeito.
- testes e histórico podem reduzir o espaço de hipóteses.

## Do not use this skill when

- o pedido é apenas implementar uma feature.
- não existe sintoma observável nem condição de falha.
- a causa já foi demonstrada e o objetivo atual é produzir o patch.

## Procedure

1. Descreva o sintoma, condições e resultado esperado.
2. Formule um conjunto pequeno de hipóteses falsificáveis.
3. Use run_tests para reproduzir ou criar o menor sinal observável disponível.
4. Leia o caminho de código e use search_text para rastrear dados e condições.
5. Use git_history quando uma regressão ou mudança recente for plausível.
6. Elimine hipóteses por evidência e escolha a causa mais próxima que explica todos os sintomas.
7. Separe causa raiz, fatores contribuintes e dano observado.

## Completion

A causa proposta explica o sintoma e as condições, possui evidência rastreável e foi diferenciada de sintomas e fatores contribuintes.
