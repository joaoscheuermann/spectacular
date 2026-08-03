---
name: scope-preserving-response
description: "Mantém a execução dentro do resultado, formato e restrições pedidos, excluindo análises, previsões ou efeitos externos não solicitados."
---

# Scope Preserving Response

## Purpose

Evitar expansão de escopo causada por skills ou informações disponíveis, preservando a intenção original do usuário.

## Use this skill when

- o pedido contém limites claros de formato, período, domínio ou efeito.
- o catálogo possui skills próximas que acrescentariam trabalho não solicitado.
- a resposta precisa ser objetiva ou contratualmente restrita.

## Do not use this skill when

- o usuário autorizou exploração aberta ou alternativas adicionais.
- uma observação torna impossível cumprir o escopo sem revisão do plano.
- o objetivo atual é justamente identificar lacunas no pedido.

## Procedure

1. Reescreva internamente o resultado requerido e suas restrições.
2. Classifique cada passo proposto como necessário, opcional ou fora do escopo.
3. Exclua previsões, recomendações, persistência ou envio quando não solicitados.
4. Não transforme a disponibilidade de uma tool em razão para usá-la.
5. Quando faltar informação necessária, peça revisão ou declare a limitação em vez de ampliar o objetivo.
6. Verifique o formato e a extensão antes de concluir.

## Completion

A saída satisfaz o resultado solicitado sem incluir trabalho, formato ou efeito externo que o usuário não pediu.
