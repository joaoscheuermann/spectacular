---
name: exact-document-reading
description: "Lê um documento explicitamente identificado por caminho, nome ou ID e retorna o conteúdo solicitado sem procurar alternativas."
allowed-tools:
  - read
---

# Exact Document Reading

## Purpose

Executar leitura direta quando a identidade do artefato já está resolvida, evitando descoberta ou substituição não solicitada.

## Use this skill when

- o pedido fornece caminho, ID, URI interna ou nome único.
- o estado anterior já resolveu a referência exata.
- o objetivo pede conteúdo ou uma seção de um artefato conhecido.

## Do not use this skill when

- o usuário pede o documento mais recente ou mais apropriado.
- o identificador é ambíguo entre várias versões.
- é necessário validar se o arquivo é final ou vigente antes da leitura.

## Procedure

1. Preserve o identificador exatamente como recebido.
2. Use read no artefato indicado, com intervalo quando o objetivo delimitar uma seção.
3. Não substitua o documento por outro de nome semelhante.
4. Se a leitura falhar, reporte a referência e a falha observável; não adivinhe um caminho alternativo.
5. Retorne somente o conteúdo necessário e a referência do artefato.

## Completion

O conteúdo solicitado do documento identificado foi lido, ou a impossibilidade foi registrada sem trocar silenciosamente de artefato.
