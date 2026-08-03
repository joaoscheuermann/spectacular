---
name: test-failure-diagnosis
description: "Diagnostica testes falhos distinguindo defeito de produto, expectativa incorreta, flakiness, ambiente e configuração."
allowed-tools:
  - read
  - search_text
  - run_tests
  - execute_command
---

# Test Failure Diagnosis

## Purpose

Classificar e explicar falhas de teste antes de alterar código ou assertions.

## Use this skill when

- há saída de teste, stack trace ou caso falho.
- a falha pode depender de ambiente, ordem ou dados.
- o usuário precisa entender por que a suíte falhou.

## Do not use this skill when

- não há teste falho observável.
- o objetivo é desenhar novos testes para um bug conhecido.
- a causa já foi isolada e o pedido é aplicar a correção.

## Procedure

1. Execute o menor conjunto de testes que reproduz a falha.
2. Leia assertion, fixture, setup e código sob teste.
3. Compare esperado, recebido e pré-condições.
4. Use execute_command somente para inspecionar ambiente ou reproduzir comando necessário.
5. Classifique a falha como produto, teste, fixture, ambiente, ordem ou flakiness.
6. Repita sob variação controlada quando a falha não for determinística.
7. Indique a evidência mínima que sustenta a classificação.

## Completion

A falha foi reproduzida ou caracterizada, sua classe foi determinada e a causa provável está ligada a evidência de teste e código.
