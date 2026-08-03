---
name: file-writing
description: "Persiste conteúdo aprovado em um caminho solicitado, preservando exatamente o artefato produzido e confirmando o resultado observável."
allowed-tools:
  - write
---

# File Writing

## Purpose

Separar a criação cognitiva do conteúdo de sua persistência em arquivo.

## Use this skill when

- o usuário pede salvar, criar ou materializar um arquivo.
- o conteúdo final e o caminho estão definidos.
- a conclusão exige que o artefato exista fora do chat.

## Do not use this skill when

- o usuário pediu apenas o conteúdo na resposta.
- o conteúdo ainda precisa de análise ou revisão substantiva.
- o pedido exige acrescentar sem substituir um arquivo existente.

## Procedure

1. Confirme o caminho e o conteúdo que devem ser persistidos.
2. Não altere o conteúdo durante a etapa de escrita.
3. Use write com o modo coerente com a solicitação explícita.
4. Preserve codificação e formato textual esperado.
5. Registre o caminho retornado e qualquer metadado observável.
6. Não criar arquivos auxiliares não solicitados.

## Completion

O arquivo existe no caminho solicitado, contém o conteúdo aprovado e sua referência está disponível para entrega.
