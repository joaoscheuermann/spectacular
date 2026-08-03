---
name: latest-document-reading
description: "Localiza e lê o documento válido mais recente de um tipo solicitado, confirmando período, status e conteúdo antes de disponibilizá-lo."
allowed-tools:
  - list
  - inspect_metadata
  - read
---

# Latest Document Reading

## Purpose

Ensinar uma trajetória de descoberta e leitura quando o usuário identifica o tipo de documento, mas não fornece um arquivo exato.

## Use this skill when

- o pedido menciona o último relatório, memo, extrato, especificação ou documento equivalente.
- há múltiplos candidatos com datas ou versões diferentes.
- o conteúdo correto precisa ser disponibilizado a um objetivo posterior.

## Do not use this skill when

- o usuário forneceu caminho, ID ou nome exato e não pediu validação de versão.
- o objetivo é comparar versões, e não escolher apenas a mais recente.
- a data mais recente não representa necessariamente a versão válida e não há metadados para confirmar status.

## Procedure

1. Identifique o tipo documental, período e escopo solicitados.
2. Use list para obter candidatos plausíveis sem ler o catálogo inteiro.
3. Use inspect_metadata para comparar data, versão, status e marcadores como draft, final ou superseded.
4. Leia cabeçalhos ou trechos mínimos dos melhores candidatos para confirmar identidade e período.
5. Prefira a versão válida mais recente, não apenas o arquivo com timestamp maior.
6. Leia o conteúdo necessário e preserve uma referência inequívoca ao documento escolhido.

## Completion

Um único documento foi selecionado com justificativa de recência e validade, e o conteúdo necessário está disponível ao próximo objetivo.
