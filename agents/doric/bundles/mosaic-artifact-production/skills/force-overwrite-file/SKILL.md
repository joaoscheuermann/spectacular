---
name: force-overwrite-file
description: "Substitui integralmente um arquivo existente pelo conteúdo fornecido quando o usuário solicita explicitamente sobrescrita total."
allowed-tools:
  - write
---

# Force Overwrite File

## Purpose

Executar substituição deliberada sem tentar mesclar, preservar ou inferir partes do arquivo anterior.

## Use this skill when

- o usuário usa instruções inequívocas como substituir, sobrescrever ou recriar do zero.
- o conteúdo completo de destino está pronto.
- a existência anterior do arquivo não deve afetar o resultado.

## Do not use this skill when

- o pedido é acrescentar ou preservar trechos existentes.
- o usuário não autorizou substituição total.
- o conteúdo fornecido é apenas um fragmento.

## Procedure

1. Confirme que a solicitação exige substituição integral e que o conteúdo é completo.
2. Não leia ou mescle o arquivo anterior salvo se isso for necessário para confirmar o caminho.
3. Use write em modo de sobrescrita.
4. Verifique que o conteúdo final corresponde integralmente ao novo artefato.
5. Registre o caminho e o fato de que houve substituição.
6. Não combine esta skill com procedimentos append-only para o mesmo objetivo e arquivo.

## Completion

O arquivo foi substituído integralmente pelo conteúdo completo autorizado, sem preservação ou mescla não solicitada.
