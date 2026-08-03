---
name: channel-message-delivery
description: "Seleciona um canal de comunicação compatível com o contexto, prepara a mensagem final e executa o envio solicitado."
allowed-tools:
  - list_channels
  - send_message
---

# Channel Message Delivery

## Purpose

Combinar resolução de canal e efeito externo em um único resultado quando o usuário pede entrega em canal.

## Use this skill when

- o usuário pede enviar ou publicar em Slack, chat de equipe ou canal equivalente.
- o canal é nomeado parcialmente ou precisa ser resolvido.
- o conteúdo da mensagem já existe ou pode ser preparado no mesmo objetivo.

## Do not use this skill when

- o usuário pede somente redigir sem enviar.
- o destinatário é uma pessoa e a entrega deve ser privada.
- o pedido exige anúncio público formal para múltiplas audiências.

## Procedure

1. Identifique organização, workspace, canal e propósito da mensagem.
2. Use list_channels para resolver o canal; não escolha apenas por nome semelhante quando houver ambiguidade.
3. Ajuste formato e extensão às convenções do canal sem mudar fatos.
4. Confirme que o pedido inclui envio e que o texto está completo.
5. Use send_message com destinationType channel.
6. Preserve o retorno da tool como evidência do efeito.

## Completion

A mensagem correta foi enviada ao canal resolvido, e o identificador ou retorno de entrega está registrado.
