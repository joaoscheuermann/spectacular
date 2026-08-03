---
name: api-contract-analysis
description: "Analisa contratos de API, incluindo entradas, saídas, erros, invariantes, versionamento e expectativas dos consumidores."
allowed-tools:
  - read
  - search_text
---

# API Contract Analysis

## Purpose

Tornar explícito o contrato observável de uma interface antes de explicar, testar ou alterar sua implementação.

## Use this skill when

- o pedido envolve endpoint, função pública, evento, schema ou interface.
- é necessário verificar compatibilidade entre produtor e consumidor.
- uma mudança pode alterar payload, erros ou semântica.

## Do not use this skill when

- o objetivo é entender lógica interna sem interface pública.
- o pedido se limita a documentação geral do repositório.
- não há definição ou uso observável do contrato.

## Procedure

1. Localize definição, tipos, validações e documentação do contrato.
2. Identifique campos obrigatórios, opcionais, defaults e restrições.
3. Registre respostas, erros, efeitos e garantias de idempotência quando presentes.
4. Use search_text para localizar consumidores e exemplos reais.
5. Compare contrato declarado com comportamento implementado.
6. Destaque mudanças incompatíveis, ambiguidades e dependências de versão.

## Completion

Entradas, saídas, erros, invariantes e consumidores relevantes estão descritos, com incompatibilidades ou lacunas explicitadas.
