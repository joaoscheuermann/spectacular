---
name: repository-orientation
description: "Mapeia a estrutura de um repositório, identifica pontos de entrada, configuração, pacotes e convenções antes de qualquer análise ou mudança."
allowed-tools:
  - repository_tree
  - read
  - package_info
---

# Repository Orientation

## Purpose

Criar uma visão inicial do sistema suficiente para orientar objetivos posteriores sem ler arquivos indiscriminadamente.

## Use this skill when

- o pedido envolve um repositório desconhecido.
- é necessário localizar módulos, entry points, configurações ou pacotes.
- uma análise ou mudança depende de entender a organização do código.

## Do not use this skill when

- o caminho e o arquivo relevantes já estão resolvidos.
- o objetivo é revisar um diff pequeno e autocontido.
- o usuário pediu apenas executar uma operação conhecida.

## Procedure

1. Use repository_tree para obter uma visão limitada por profundidade.
2. Identifique manifestos, entry points, pastas de domínio, testes e configuração.
3. Use package_info para confirmar linguagem, gerenciador e scripts disponíveis.
4. Leia somente arquivos de orientação, como README, manifestos e configurações centrais.
5. Registre convenções de estrutura e os caminhos mais prováveis para o objetivo.
6. Não conclua comportamento do sistema apenas pelos nomes dos arquivos.

## Completion

A estrutura relevante, os pontos de entrada, as configurações e os próximos arquivos a inspecionar estão identificados.
