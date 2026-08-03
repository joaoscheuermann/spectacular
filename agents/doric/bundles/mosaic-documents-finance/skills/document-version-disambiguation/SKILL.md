---
name: document-version-disambiguation
description: "Distingue versões, rascunhos e documentos substituídos para selecionar a edição correta segundo vigência, status e finalidade."
allowed-tools:
  - list
  - inspect_metadata
  - read
---

# Document Version Disambiguation

## Purpose

Resolver ambiguidade entre documentos semanticamente semelhantes quando recência isolada não determina a versão correta.

## Use this skill when

- há arquivos com o mesmo título, período ou conteúdo aproximado.
- existem marcadores de draft, final, amended, approved ou superseded.
- o pedido depende da versão vigente ou formalmente aprovada.

## Do not use this skill when

- há um único documento com identificador exato.
- o usuário pediu deliberadamente comparar todas as versões.
- o objetivo é apenas obter o mais recente e os status são equivalentes.

## Procedure

1. Liste os candidatos que compartilham identidade documental.
2. Colete versão, data, status, autor ou aprovador e relação de substituição.
3. Leia trechos de identificação quando os metadados forem insuficientes.
4. Aplique a precedência exigida pelo pedido: vigente, aprovada, assinada, publicada ou outra.
5. Não trate rascunho mais novo como substituto automático de versão final.
6. Registre os candidatos rejeitados e a razão objetiva da escolha.

## Completion

A versão escolhida satisfaz a regra de vigência ou finalidade, e a decisão pode ser auditada pelos metadados e trechos lidos.
