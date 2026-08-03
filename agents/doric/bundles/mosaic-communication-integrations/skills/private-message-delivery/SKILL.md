---
name: private-message-delivery
description: "Prepara e entrega uma comunicação privada a destinatários resolvidos, distinguindo rascunho de envio efetivo."
allowed-tools:
  - list_contacts
  - get_contact
  - create_draft
  - send_message
---

# Private Message Delivery

## Purpose

Conduzir comunicação individual ou restrita sem convertê-la em anúncio público e sem confundir preparar com enviar.

## Use this skill when

- o pedido envolve email, mensagem direta ou comunicação privada.
- destinatários precisam ser resolvidos ou confirmados.
- o usuário pode pedir rascunho ou envio.

## Do not use this skill when

- o destino é um canal público ou de equipe.
- o usuário pede anúncio amplo.
- não há autorização textual para envio e o pedido se limita a escrever.

## Procedure

1. Resolva cada destinatário por contato e confirme identidade.
2. Adapte saudação, contexto e chamada para ação à relação.
3. Quando o pedido disser rascunhar, use create_draft e não execute envio.
4. Quando o pedido disser enviar, use send_message com destinationType contact.
5. Não exponha destinatários privados em canal público.
6. Registre draftId ou messageId e destinatários efetivos.

## Completion

O rascunho ou envio corresponde exatamente à ação pedida, alcança os contatos corretos e mantém a comunicação privada.
