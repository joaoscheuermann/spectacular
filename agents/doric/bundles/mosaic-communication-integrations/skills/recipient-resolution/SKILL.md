---
name: recipient-resolution
description: "Resolve a pessoa ou contato correto a partir de nome, função, empresa ou contexto, tratando ambiguidade antes de qualquer envio."
allowed-tools:
  - list_contacts
  - get_contact
---

# Recipient Resolution

## Purpose

Transformar uma referência humana parcial em um identificador de destinatário confiável.

## Use this skill when

- o pedido menciona uma pessoa por nome parcial, função ou organização.
- uma mensagem ou convite depende de um contactId.
- há possibilidade de homônimos ou múltiplos endereços.

## Do not use this skill when

- o destinatário já foi fornecido por ID ou endereço inequívoco.
- o objetivo é escolher um canal público, não uma pessoa.
- não existe critério suficiente para diferenciar candidatos e não é permitido pedir esclarecimento.

## Procedure

1. Extraia nome, função, empresa, domínio e relação contextual do pedido.
2. Use list_contacts com os termos mais discriminantes.
3. Use get_contact para confirmar detalhes dos melhores candidatos.
4. Elimine candidatos que contrariem empresa, função ou contexto.
5. Quando restar ambiguidade material, não escolha por proximidade textual; retorne os candidatos e a informação faltante.
6. Preserve o identificador resolvido para o objetivo de entrega.

## Completion

Um destinatário inequívoco foi resolvido e referenciado, ou a ambiguidade foi explicitada sem envio ao contato errado.
