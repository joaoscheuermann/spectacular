---
name: readme-installation-writing
description: "Cria uma seção ou arquivo README de instalação com requisitos, instalação, configuração, execução e verificação inicial."
allowed-tools:
  - package_info
  - read
---

# README Installation Writing

## Purpose

Transformar informações reais do projeto em instruções de instalação reproduzíveis para um ambiente limpo.

## Use this skill when

- o usuário pede README, setup, getting started ou instruções de instalação.
- há manifestos, scripts ou configuração local a consultar.
- o leitor precisa chegar do clone ao primeiro resultado verificável.

## Do not use this skill when

- o pedido é documentação conceitual sem instalação.
- o conteúdo do README já está pronto e falta somente salvar.
- não existe informação suficiente para afirmar comandos ou requisitos.

## Procedure

1. Use package_info para identificar runtime, gerenciador, scripts e versões declaradas.
2. Leia arquivos de configuração e exemplos necessários.
3. Liste pré-requisitos com versões somente quando verificáveis.
4. Apresente instalação de dependências, configuração de ambiente e comando de execução.
5. Inclua um teste simples de verificação e o resultado esperado.
6. Separe passos obrigatórios de opções locais.
7. Não invente credenciais, URLs, nomes de scripts ou variáveis.

## Completion

O README contém um caminho reproduzível de instalação e execução, com pré-requisitos e verificação sustentados pelo projeto.
