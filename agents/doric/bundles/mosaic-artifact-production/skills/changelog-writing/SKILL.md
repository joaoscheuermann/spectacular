---
name: changelog-writing
description: "Produz changelog baseado em alterações observáveis, agrupando impacto para o usuário e evitando inferir mudanças que o histórico não sustenta."
allowed-tools:
  - git_history
  - diff
---

# Changelog Writing

## Purpose

Converter commits ou diferenças em notas de versão claras, sem copiar mensagens internas nem inventar benefícios.

## Use this skill when

- o usuário pede changelog, release notes ou resumo de mudanças entre refs.
- há histórico ou diff disponível.
- as alterações precisam ser classificadas por impacto.

## Do not use this skill when

- o pedido é explicar a implementação atual sem intervalo de versões.
- não existem refs, commits ou mudanças observáveis.
- o objetivo é escrever anúncio promocional da versão.

## Procedure

1. Defina o intervalo de versão, data ou refs.
2. Use git_history e diff para coletar alterações.
3. Agrupe por adicionados, alterados, corrigidos, removidos ou outra taxonomia solicitada.
4. Reescreva mensagens internas em impacto observável para o usuário ou mantenedor.
5. Destaque breaking changes, migrações e ações necessárias.
6. Remova commits sem efeito entregável, como merge mecânico, quando apropriado.
7. Não atribua causa, benefício ou compatibilidade sem evidência.

## Completion

O changelog cobre o intervalo correto, descreve mudanças observáveis por categoria e sinaliza incompatibilidades ou ações necessárias.
