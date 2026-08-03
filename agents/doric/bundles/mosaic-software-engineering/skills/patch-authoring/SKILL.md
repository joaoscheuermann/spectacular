---
name: patch-authoring
description: "Produz uma alteração de código mínima e coerente com o repositório, revisando o diff e verificando o comportamento afetado."
allowed-tools:
  - read
  - apply_patch
  - diff
  - run_tests
---

# Patch Authoring

## Purpose

Aplicar uma mudança já compreendida com o menor escopo necessário e evidência de que o resultado foi atingido.

## Use this skill when

- o objetivo e a abordagem estão definidos.
- o usuário solicita modificar código.
- há arquivos e critérios de conclusão identificados.

## Do not use this skill when

- a causa ou requisito ainda está ambíguo.
- o pedido é apenas explicar ou planejar.
- a mudança exige migração ampla ainda não decomposta.

## Procedure

1. Leia a implementação e os testes diretamente relacionados.
2. Defina a menor alteração que satisfaz o comportamento.
3. Use apply_patch preservando estilo e contratos existentes salvo quando a mudança exigir o contrário.
4. Revise o diff para remover alterações acidentais.
5. Execute testes focados e, quando viável, a suíte relevante.
6. Não introduza refatorações não necessárias ao objetivo.

## Completion

O diff contém apenas mudanças necessárias, os critérios foram verificados e qualquer limitação de teste está registrada.
