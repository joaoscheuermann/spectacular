---
name: append-only-file-update
description: "Atualiza um arquivo existente acrescentando conteúdo sem substituir ou reordenar o material anterior."
allowed-tools:
  - read
  - append
---

# Append Only File Update

## Purpose

Aplicar atualização cumulativa quando a preservação byte a byte ou estrutural do conteúdo existente é requisito.

## Use this skill when

- o usuário pede adicionar, anexar ou registrar nova entrada.
- o arquivo funciona como log, changelog, diário ou coleção incremental.
- a substituição do conteúdo existente seria incorreta.

## Do not use this skill when

- o usuário pede reescrever, reorganizar ou substituir o arquivo.
- não há caminho de destino definido.
- a atualização precisa modificar trechos existentes.

## Procedure

1. Leia o final ou a estrutura necessária do arquivo.
2. Determine separador, cabeçalho e convenção de nova entrada.
3. Prepare apenas o bloco novo.
4. Use append sem enviar novamente o conteúdo anterior.
5. Evite duplicar entrada já presente quando isso puder ser verificado.
6. Confirme que o conteúdo anterior permanece e que o novo bloco foi acrescentado.

## Completion

O arquivo preserva o conteúdo anterior e contém exatamente a nova entrada no local permitido pela semântica de append.
