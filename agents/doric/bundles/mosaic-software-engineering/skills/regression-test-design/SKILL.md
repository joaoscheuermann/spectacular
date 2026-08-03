---
name: regression-test-design
description: "Projeta testes que reproduzem um defeito ou protegem um comportamento alterado, incluindo limites e assertions estáveis."
allowed-tools:
  - read
  - apply_patch
  - run_tests
---

# Regression Test Design

## Purpose

Converter um risco ou bug conhecido em verificação automatizada que falha antes da correção e passa depois.

## Use this skill when

- há comportamento esperado e condição de regressão.
- uma mudança precisa de cobertura específica.
- o usuário pede teste de regressão ou desenho de casos.

## Do not use this skill when

- não há critério observável para o comportamento.
- o pedido é diagnosticar uma falha de teste existente.
- a verificação adequada é manual ou pertence a outro nível não disponível.

## Procedure

1. Defina o comportamento e o menor cenário que o torna observável.
2. Escolha o nível de teste mais próximo do contrato afetado.
3. Crie caso principal, limite material e erro relevante.
4. Evite assertions sobre detalhes internos que não fazem parte do contrato.
5. Use apply_patch para adicionar ou ajustar testes quando solicitado.
6. Execute o teste antes e depois da correção quando o estado permitir.
7. Verifique determinismo e isolamento de fixtures.

## Completion

Os testes cobrem o comportamento e seus limites, falham pelo motivo correto sem a correção e evitam acoplamento desnecessário à implementação.
