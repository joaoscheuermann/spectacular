---
name: change-impact-analysis
description: "Avalia quais módulos, contratos, consumidores, testes e comportamentos podem ser afetados por uma mudança proposta ou observada."
allowed-tools:
  - dependency_graph
  - diff
  - search_text
  - read
---

# Change Impact Analysis

## Purpose

Delimitar impacto antes de implementar ou aprovar uma mudança, cobrindo efeitos transitivos sem listar o repositório inteiro.

## Use this skill when

- há diff, proposta ou componente-alvo.
- o usuário pergunta o que pode quebrar ou precisar mudar.
- existem consumidores, contratos ou dados compartilhados.

## Do not use this skill when

- o objetivo é somente explicar o comportamento atual.
- a mudança é puramente textual e comprovadamente sem efeito semântico.
- não existe definição suficiente da mudança.

## Procedure

1. Defina a unidade modificada e o comportamento pretendido.
2. Use diff quando houver refs ou alteração concreta.
3. Use dependency_graph para localizar dependências e consumidores.
4. Confirme usos dinâmicos com search_text e read.
5. Classifique impacto em contrato, dados, runtime, testes, build e documentação.
6. Priorize efeitos plausíveis e marque os que exigem verificação adicional.
7. Não trate toda dependência transitiva como impacto real sem mecanismo de propagação.

## Completion

Os impactos materiais e seus mecanismos estão identificados, com consumidores, testes e contratos prioritários para verificação.
