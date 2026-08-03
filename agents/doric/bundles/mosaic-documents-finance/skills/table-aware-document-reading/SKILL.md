---
name: table-aware-document-reading
description: "Interpreta tabelas em documentos preservando cabeçalhos, unidades, períodos, notas e relações entre linhas e colunas."
allowed-tools:
  - read
  - parse_table
---

# Table Aware Document Reading

## Purpose

Evitar erros comuns de leitura tabular, como deslocar colunas, perder unidades ou ignorar notas que alteram o significado.

## Use this skill when

- os dados relevantes estão em uma tabela ou quadro.
- há comparações entre períodos, categorias ou métricas.
- a saída depende de valores que precisam ser associados corretamente a rótulos.

## Do not use this skill when

- o conteúdo é puramente narrativo.
- a tabela já foi extraída e validada em estrutura confiável.
- o objetivo requer somente localizar o documento.

## Procedure

1. Leia o título, cabeçalhos, unidades, período e notas da tabela.
2. Use parse_table para obter uma estrutura de linhas e colunas.
3. Verifique células mescladas, totais, subtotais, sinais negativos e escalas como milhares ou milhões.
4. Associe cada valor aos cabeçalhos corretos antes de calcular ou resumir.
5. Preserve notas de rodapé que alterem escopo, definição ou comparabilidade.
6. Sinalize células ilegíveis ou relações ambíguas em vez de completar por padrão.

## Completion

Os valores relevantes estão associados aos rótulos, períodos e unidades corretos, com notas materiais preservadas.
