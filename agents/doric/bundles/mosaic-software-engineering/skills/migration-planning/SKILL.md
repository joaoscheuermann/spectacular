---
name: migration-planning
description: "Planeja migração entre versões, bibliotecas ou arquiteturas identificando breaking changes, compatibilidade, etapas e estratégia de rollback."
allowed-tools:
  - package_info
  - dependency_graph
  - read
  - search_text
---

# Migration Planning

## Purpose

Transformar uma atualização potencialmente ampla em transição verificável e ordenada, sem misturar migração com refatorações oportunistas.

## Use this skill when

- o pedido envolve atualização de framework, biblioteca, runtime ou arquitetura.
- há consumidores e dependências que precisam migrar em sequência.
- compatibilidade temporária ou rollback são relevantes.

## Do not use this skill when

- a mudança é uma correção local sem alteração de versão ou contrato.
- o objetivo é aplicar diretamente um codemod já validado.
- não há informação sobre estado atual e destino.

## Procedure

1. Confirme versão atual, versão alvo e restrições de ambiente.
2. Use package_info e documentação local para identificar mudanças incompatíveis.
3. Mapeie consumidores com dependency_graph e search_text.
4. Separe preparação, compatibilidade, migração de uso, limpeza e validação.
5. Defina checkpoints, critérios de rollback e período de coexistência quando necessário.
6. Inclua testes de contrato e dados afetados.
7. Evite incluir refatorações que não reduzem risco ou desbloqueiam a migração.

## Completion

O plano apresenta estado atual e alvo, breaking changes, sequência de adoção, verificações e estratégia de rollback ou coexistência.
