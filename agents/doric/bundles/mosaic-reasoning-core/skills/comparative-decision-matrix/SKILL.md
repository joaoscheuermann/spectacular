---
name: comparative-decision-matrix
description: "Compara alternativas sob critérios explícitos, aplica escalas consistentes e torna visíveis pesos, trade-offs e dados ausentes."
allowed-tools:
  - calculator
  - sort_records
---

# Comparative Decision Matrix

## Purpose

Estruturar uma decisão comparativa de modo auditável, evitando preferências implícitas ou critérios que mudam entre alternativas.

## Use this skill when

- existem duas ou mais alternativas.
- o usuário forneceu critérios ou espera uma comparação justificável.
- há atributos quantitativos e qualitativos a combinar.

## Do not use this skill when

- há uma única alternativa sem decisão comparativa.
- o pedido exige somente descrição ou resumo.
- os critérios não podem ser definidos sem inventar preferências do usuário.

## Procedure

1. Liste alternativas e elimine duplicatas semânticas.
2. Defina critérios observáveis e indique sua origem: pedido, domínio ou premissa.
3. Normalize unidades e escalas antes de comparar.
4. Atribua pesos somente quando forem fornecidos ou claramente justificados.
5. Use calculator para métricas derivadas e sort_records para ordenar resultados quando necessário.
6. Apresente pontuações junto com evidências e trade-offs; não esconda empates ou lacunas.

## Completion

Todas as alternativas foram avaliadas pelos mesmos critérios, os pesos são explícitos e a comparação permite reconstruir a ordenação.
