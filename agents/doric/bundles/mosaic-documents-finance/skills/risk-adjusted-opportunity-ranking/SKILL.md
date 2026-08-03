---
name: risk-adjusted-opportunity-ranking
description: "Ordena oportunidades considerando retorno, probabilidade, downside, incerteza e capacidade de recuperação, em vez de maximizar apenas o upside."
allowed-tools:
  - calculator
  - sort_records
---

# Risk Adjusted Opportunity Ranking

## Purpose

Produzir um ranking ajustado a risco quando o usuário precisa escolher entre alternativas com perfis de incerteza diferentes.

## Use this skill when

- o pedido menciona risco, volatilidade, confiança, downside ou perfil conservador.
- há estimativas de retorno e evidências sobre incerteza.
- a melhor alternativa bruta pode não ser a melhor ajustada ao risco.

## Do not use this skill when

- o usuário pediu somente retorno potencial sem ajuste de risco.
- não há qualquer informação para distinguir probabilidade ou downside.
- o objetivo é elaborar uma tese persuasiva para uma alternativa já escolhida.

## Procedure

1. Defina dimensões de risco relevantes ao contexto.
2. Registre retorno esperado, downside, reversibilidade, horizonte e confiança para cada alternativa.
3. Use calculator para valor esperado ou medidas derivadas somente quando as premissas forem explícitas.
4. Aplique uma regra de ranking consistente e use sort_records se necessário.
5. Realize análise de sensibilidade para premissas que mudam a ordem.
6. Explique por que uma alternativa com maior upside pode ficar abaixo de outra.

## Completion

O ranking usa uma regra explícita de ajuste a risco, preserva incertezas e indica premissas capazes de alterar a ordem.
