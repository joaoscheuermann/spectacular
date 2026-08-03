---
name: action-item-extraction
description: "Extrai ações de conteúdo disponível e representa cada item com ação, responsável, prazo, dependências e evidência de origem."
---

# Action Item Extraction

## Purpose

Transformar compromissos e pedidos em itens acionáveis sem inventar responsáveis ou datas.

## Use this skill when

- há reunião, mensagem, relatório ou plano com compromissos.
- o usuário pede tarefas, pendências ou próximos passos.
- a saída será acompanhada por pessoas ou sistema.

## Do not use this skill when

- o conteúdo contém apenas ideias sem compromisso ou ação.
- o objetivo é resumir a discussão completa.
- não existe evidência suficiente para distinguir proposta de atribuição.

## Procedure

1. Localize verbos de ação, compromissos e pedidos explícitos.
2. Para cada item, registre resultado esperado, responsável, prazo e dependências.
3. Use null ou “não definido” quando responsável ou prazo não estiverem presentes.
4. Diferencie decisão, ação, questão aberta e sugestão.
5. Preserve referência ao trecho de origem.
6. Consolide duplicatas somente quando representarem o mesmo resultado.

## Completion

Cada item possui ação verificável e campos rastreáveis, sem responsáveis ou prazos inventados.
