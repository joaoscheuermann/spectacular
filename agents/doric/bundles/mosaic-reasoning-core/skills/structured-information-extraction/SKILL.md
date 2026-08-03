---
name: structured-information-extraction
description: "Extrai campos, entidades e registros para uma estrutura definida, preservando valores ausentes, unidades e referências de origem."
allowed-tools:
  - read
  - parse_table
  - filter_records
---

# Structured Information Extraction

## Purpose

Converter conteúdo textual ou tabular em dados estruturados sem preencher lacunas por suposição.

## Use this skill when

- o objetivo pede JSON, tabela, lista de registros ou campos definidos.
- há dados em texto, tabela ou documento que precisam ser normalizados.
- a saída será consumida por outro objetivo ou verificador.

## Do not use this skill when

- o objetivo é interpretar significado ou recomendar ação.
- não existe um esquema ou conjunto mínimo de campos a produzir.
- a tarefa é somente resumir o documento.

## Procedure

1. Defina o esquema de saída e os tipos esperados.
2. Leia somente as partes necessárias da fonte.
3. Use parse_table quando a estrutura tabular for relevante.
4. Normalize unidades, datas e identificadores sem alterar o valor semântico.
5. Mantenha null ou marcador equivalente para informação ausente; não invente.
6. Use filter_records apenas para aplicar critérios explícitos e preserve referência da origem.

## Completion

A saída respeita o esquema, contém valores rastreáveis, distingue ausência de zero e não inclui campos inventados.
