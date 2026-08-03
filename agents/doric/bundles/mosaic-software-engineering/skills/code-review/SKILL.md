---
name: code-review
description: "Revisa um diff quanto a correção, regressões, contratos, testes e manutenção, priorizando achados acionáveis por severidade."
allowed-tools:
  - diff
  - read
  - run_tests
---

# Code Review

## Purpose

Avaliar uma mudança concreta sem reescrever o código por preferência pessoal ou produzir comentários genéricos.

## Use this skill when

- há patch, branch ou diff para revisar.
- o usuário pede peer review ou análise de qualidade.
- é necessário identificar riscos e lacunas de teste.

## Do not use this skill when

- não existe alteração concreta.
- o pedido é elaborar a implementação.
- o objetivo é apenas explicar o código atual.

## Procedure

1. Leia o objetivo declarado e obtenha o diff.
2. Reconstrua o comportamento alterado e seus contratos.
3. Verifique correção, tratamento de estados, compatibilidade, dados e concorrência quando aplicável.
4. Leia contexto suficiente ao redor das mudanças.
5. Execute testes focados quando isso puder confirmar um risco.
6. Registre somente achados com mecanismo de falha ou custo de manutenção claro.
7. Ordene por severidade e indique localização e correção esperada.

## Completion

A revisão contém achados específicos, priorizados e reproduzíveis, além de lacunas de teste e uma conclusão sobre o risco residual.
