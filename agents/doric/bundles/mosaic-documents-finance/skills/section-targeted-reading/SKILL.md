---
name: section-targeted-reading
description: "Encontra e lê apenas seções relevantes de um documento, preservando contexto local suficiente para interpretar o trecho corretamente."
allowed-tools:
  - search_text
  - read
---

# Section Targeted Reading

## Purpose

Reduzir leitura desnecessária sem perder definições, qualificadores ou referências que cercam a seção solicitada.

## Use this skill when

- o usuário nomeia seções, tópicos ou palavras-chave.
- o documento é longo e somente partes específicas alimentam o objetivo.
- há necessidade de localizar ocorrências antes de ler o contexto.

## Do not use this skill when

- a tarefa exige compreender o documento integralmente.
- a seção já foi entregue completa no estado.
- a estrutura do documento não permite localizar o conteúdo de forma confiável.

## Procedure

1. Converta a solicitação em títulos, sinônimos e termos de busca.
2. Use search_text para localizar ocorrências e cabeçalhos candidatos.
3. Leia o intervalo que inclui o título, o conteúdo e notas ou definições imediatamente relacionadas.
4. Expanda o intervalo quando uma frase depender de tabela, rodapé ou seção anterior.
5. Evite incluir seções apenas porque compartilham palavras.
6. Retorne referências de posição junto com o conteúdo relevante.

## Completion

As seções pedidas foram localizadas e lidas com contexto suficiente, sem carregar partes irrelevantes do documento.
