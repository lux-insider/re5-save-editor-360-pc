# Migração da interface para Slint

A interface em HTML/CSS/JS (tao + wry) foi trocada por Slint. O backend,
`src/save.rs`, não mudou.

## Estrutura

```
src/
  main.rs              só inicia: cria a janela e liga o controlador
  save.rs              BACKEND (inalterado): Xbox 360 (STFS, assinatura) e PC (XOR)
  itens.rs             tabela de itens (re5_items.json) e classes
  sistema/
    pastas.rs          keyvault, pasta de backups, pasta inicial dos diálogos
    dialogos.rs        diálogos de arquivo nativos (rfd)
  interface/
    mod.rs             Controlador: estado da edição e callbacks da UI
    modelos.rs         conversão backend -> estruturas da UI (e volta)
ui/
  app.slint            janela principal, menus, abas
  tema.slint           cores e medidas
  componentes/         campo, botão, abas, slot, lista de opções, avisos
  telas/               início, informações, desbloqueios, personagens, inventário
```

A UI não conhece o backend: ela só mostra propriedades e chama callbacks.
O controlador (Rust) é o único que fala com `save.rs`.

## Funcionalidades que precisam continuar (checklist)

Abrir
- [x] Botão "Escolher arquivo" e menu Arquivo > Abrir (diálogo nativo)
- [x] Caminho do save pela linha de comando
- [~] Arrastar o arquivo para a janela (implementado; não dá para simular arrastar no teste automático)
- [x] Detecta sozinho Xbox 360 (pacote CON) ou PC (XOR + checksum)
- [x] Mensagem de erro se o arquivo não for reconhecido

Informações
- [x] Caminho do arquivo, plataforma (Xbox 360 · Title ID / PC · Steam)
- [x] Xbox: perfil (XUID, 16 hex) e device ID (40 hex) editáveis, com validação
- [x] Xbox: "Copiar de outro save…" e "Voltar IDs"
- [x] PC: Steam ID editável (só números)
- [x] Data do save editável, com validação
- [x] Checksum salvo e real, e se confere
- [x] Xbox: assinatura (válida/inválida e o console)
- [x] Gold/Money e Exchange Points (só números)
- [x] Nota sobre o keyvault (assinatura automática ou não) e backups

Desbloqueios (Xbox e PC)
- [x] Roupas do Chris, roupas da Sheva, filtros, munição infinita, arquivos, figuras
- [x] Marcar/desmarcar cada opção, "Marcar todos" e "Desmarcar todos", contagem

Personagens e inventário (Xbox)
- [x] 9 slots do Chris e 9 da Sheva
- [x] 84 espaços do inventário, filtro (todos, ocupados, por classe)
- [x] Editar slot: classe, item (nomes da tabela, ID em hex), quantidade com máximo
- [x] Slot alterado aparece marcado

Salvar
- [x] "Salvar arquivo" só com alterações e campos válidos
- [x] "Salvar como…" (diálogo nativo)
- [x] "Desfazer" volta ao que estava no arquivo
- [x] Backup antes de gravar por cima; assina com o keyvault; confere depois
- [x] Aviso de sucesso/erro com o caminho e o backup
- [x] "Abrir outro" e fechar a janela pedem confirmação com alterações pendentes

## Como foi testado (25/09/2026)

- Save do Xbox editado pela interface nova (quantidade no inventário, roupa
  Warrior, Gold): o código Python antigo confirma checksum, hashes e assinatura
  válidos, e só os 9 bytes esperados mudaram. Backup criado.
- Save de PC editado pela interface (Money, roupas, data): checksum válido e
  valores novos, lidos por uma implementação independente do formato.
- Validação: data inválida fica vermelha e desativa o Salvar.
- Confirmação ao fechar com alterações, Desfazer e Sobre conferidos na tela.
- Windows: compila (9,7 MB, só DLLs do sistema, sem WebView2); não testado em
  execução porque o Wine não está instalado nesta máquina.
