# Memorata — Plano Detalhado de Refatoração

> Para implementação incremental com TDD. Escopo: transformar o fork do Handy em produto novo, sem legado de ditado.

## Objetivo

Entregar um desktop app focado em:
1) Gravar áudio localmente
2) Processar transcrição
3) Disponibilizar arquivo final para download/export

Sem:
- Tray
- Hotkeys globais
- Paste em app ativo
- Compatibilidade com modo legado de ditado

## Diretrizes mandatórias

- Produto novo: aceitar mudanças quebrando compatibilidade com upstream
- App operando em janela principal (foreground)
- Gatilhos de gravação por botões de UI
- Arquivo final baixável, sem edição interna

## Arquitetura alvo (v2)

Fluxo principal:
UI (Start/Pause/Stop) -> SessionController -> AudioRecorder -> WAV -> TranscriptionService -> ArtifactExporter -> Download

Estados:
- idle
- recording
- paused
- processing
- completed
- failed

## Decisões de implementação

1) No curto prazo, controlar sessão via comandos Tauri dedicados (UI-first).
2) Eliminar dependência funcional de paste no pipeline final.
3) Gerar artefato .txt no backend (sidecar do .wav inicialmente).
4) Manter histórico de transcrição como fonte de consulta para export.
5) Remoção física de módulos legados será feita em fases para reduzir risco.

## Plano por fases (detalhado)

### Fase 1 — Base de controle de sessão (UI-first)

Objetivo:
- Introduzir comandos explícitos para iniciar/parar sessão pela UI.

Arquivos:
- `src-tauri/src/transcription_coordinator.rs`
- `src-tauri/src/commands/transcription.rs`
- `src-tauri/src/lib.rs`
- `src/bindings.ts`
- `src/components/recording/RecordingSessionControls.tsx`
- `src/App.tsx`

Tarefas:
1. Expor estado de sessão no coordinator (recording/processing).
2. Criar comandos Tauri:
   - `get_recording_session_state`
   - `start_recording_session`
   - `stop_recording_session`
3. Registrar comandos no `lib.rs`.
4. Adicionar componente de controle na UI principal.
5. Polling de estado para refletir status atual em tela.

Critério de aceite:
- Usuário consegue iniciar e parar gravação sem hotkey.
- Estado muda corretamente entre idle/recording/processing.

### Fase 2 — Remoção do output por paste

Objetivo:
- Encerrar pipeline sem automação de entrada em app terceiro.

Arquivos:
- `src-tauri/src/actions.rs`
- `src-tauri/src/managers/history.rs`

Tarefas:
1. Remover chamada de `paste` no final da transcrição.
2. Persistir artefato textual (`.txt`) junto ao áudio gravado.
3. Manter update de histórico.

Critério de aceite:
- Nenhuma transcrição é colada automaticamente no sistema.
- Existe arquivo `.txt` correspondente ao áudio processado.

### Fase 3 — Tray/hotkey legado para desativação total

Objetivo:
- App não depender de tray nem hotkeys para operação principal.

Arquivos:
- `src-tauri/src/lib.rs`
- `src-tauri/src/tray.rs`
- `src-tauri/src/cli.rs`
- `src/components/settings/*` (itens de UI legados)

Tarefas:
1. Tornar tray semanticamente desativado por padrão.
2. Garantir que chamadas residuais de tray não quebrem runtime (no-op seguro).
3. Remover inicialização de shortcuts da UI.
4. Remover gradualmente controles de shortcut/paste/tray das telas de settings.

Critério de aceite:
- Fluxo principal funciona 100% sem tray/hotkeys.
- Sem crash por ausência de estado de tray.

### Fase 4 — Download/export do arquivo final

Objetivo:
- Disponibilizar arquivo de transcrição para usuário baixar/exportar.

Arquivos:
- `src-tauri/src/commands/history.rs`
- `src/components/settings/history/HistorySettings.tsx` (ou nova tela de sessão)

Tarefas:
1. Comando de export/download por entrada de histórico.
2. Botão de download na UI.
3. Garantir que arquivo não seja editável dentro do app.

Critério de aceite:
- Usuário baixa arquivo `.txt` final da transcrição com 1 clique.

### Fase 5 — Limpeza estrutural e renomeação de produto

Objetivo:
- Consolidar Memorata como produto independente.

Arquivos:
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `package.json`
- `README.md`
- assets/icons/nome de app

Tarefas:
1. Renomear app/branding para Memorata.
2. Remover módulos não utilizados (clipboard/hotkeys/tray legacy).
3. Atualizar documentação de build e operação.

Critério de aceite:
- Build e UX sem nomenclatura Handy.
- Superfície de código legado reduzida.

## Estratégia de testes

### Backend (Rust)

- Testes unitários de coordinator (estado e transições)
- Testes de comando:
  - start quando idle
  - stop quando recording
  - erro quando processing
- Teste de persistência de artefato `.txt`

### Frontend (TS/React)

- Render do painel de gravação
- Botões habilitados/desabilitados conforme estado
- Tratamento de erros de comando

## Riscos e mitigação

1) Semântica de pause/resume
- Risco: pipeline atual não tem pause/resume nativo
- Mitigação: decidir especificação de pause antes da Fase 4

2) Dívida de módulos legados
- Risco: remover tudo de uma vez quebra build
- Mitigação: desativação progressiva + no-op + remoção final por lote

3) Compatibilidade cross-platform
- Risco: diferenças de permissões e áudio por OS
- Mitigação: manter núcleo de captura existente e mudar apenas bordas de controle/output

## Estado atual (iniciado)

Implementação iniciada neste ciclo com:
- Fork clonado em `/projects/memorata`
- Comandos de sessão UI-first adicionados (start/stop/state)
- UI de controles de sessão adicionada na tela principal
- Inicialização de shortcuts removida do fluxo principal da UI
- Pipeline de paste removido do fim da transcrição
- Geração de artefato `.txt` no backend adicionada
- Prefixo de arquivo alterado para `memorata-`
- Tray com fallback seguro (no-op quando indisponível) iniciado
- `--no-tray` default ligado

## Próximos passos imediatos

1. Finalizar bindings TS para novos comandos e tipos (se necessário regenerar com specta no ambiente local).
2. Implementar export/download explícito de arquivo final na UI de histórico.
3. Iniciar retirada de settings legadas (paste/hotkey/tray) da interface.
4. Definir semântica exata de `pause/resume` (bloqueante para a próxima etapa de controle completo).