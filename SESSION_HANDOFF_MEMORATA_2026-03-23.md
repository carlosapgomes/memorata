# Memorata — Session Handoff (2026-03-23)

## 1) Objetivo do fork
Transformar o Handy em produto novo (Memorata), com fluxo:
Gravar -> (Pause/Resume sem processar) -> Stop final -> Processar -> Arquivo .txt para download.

Regras já acordadas:
- Sem tray como feature principal
- Sem hotkeys globais para operação principal
- Sem paste no app ativo
- Sem modo legado de ditado
- Processamento apenas no Stop/Finish

---

## 2) Estado atual do código

### Implementado
1. Fork criado em `/projects/memorata`
2. Plano de refatoração salvo em `/projects/memorata/REFATORACAO_MEMORATA.md`
3. Controle de sessão UI-first implementado (backend + frontend)
4. Estado `paused` implementado no pipeline de sessão
5. Pause/Resume reais no recorder layer (não simulados com stop/start)
6. Processamento ocorre apenas no Stop final
7. Paste removido do fluxo final de transcrição
8. Geração de artefato `.txt` no backend
9. Comando de export do transcript (`export_transcript_file`) adicionado
10. Botão de download de transcript adicionado no History UI
11. Limpeza de UI legada iniciada (General/Advanced/Debug sem hotkey/paste/tray)
12. Prefixo de gravação alterado de `handy-` para `memorata-`

### Principais comandos de sessão (Tauri)
- `get_recording_session_state`
- `start_recording_session`
- `pause_recording_session`
- `resume_recording_session`
- `stop_recording_session`

Estados atuais retornados:
- `idle`
- `recording`
- `paused`
- `processing`

---

## 3) Arquivos alterados nesta trilha

### Backend (Rust)
- `src-tauri/src/actions.rs`
- `src-tauri/src/audio_toolkit/audio/recorder.rs`
- `src-tauri/src/cli.rs`
- `src-tauri/src/commands/history.rs`
- `src-tauri/src/commands/transcription.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/managers/audio.rs`
- `src-tauri/src/managers/history.rs`
- `src-tauri/src/transcription_coordinator.rs`
- `src-tauri/src/tray.rs`

### Frontend (React/TS)
- `src/App.tsx`
- `src/bindings.ts`
- `src/components/recording/RecordingSessionControls.tsx` (novo)
- `src/components/settings/history/HistorySettings.tsx`
- `src/components/settings/general/GeneralSettings.tsx`
- `src/components/settings/advanced/AdvancedSettings.tsx`
- `src/components/settings/debug/DebugSettings.tsx`

### Documentação
- `REFATORACAO_MEMORATA.md` (novo)
- `SESSION_HANDOFF_MEMORATA_2026-03-23.md` (este arquivo)

---

## 4) Pontos importantes de implementação

1. Pause/Resume na captura
- `AudioRecorder` agora aceita `Cmd::Pause` e `Cmd::Resume`.
- `AudioRecordingManager` tem `pause_recording()` e `resume_recording()`.

2. Stop após pause
- No recorder foi aplicado comportamento `capture_on_drain` para evitar ingestão indevida de áudio quando o stop acontece saindo de estado pausado.

3. Coordinator
- `Stage` agora contempla `Paused(String)`.
- Novos comandos internos de sessão: `StartSession`, `PauseSession`, `ResumeSession`, `StopSession`.
- Flags de observabilidade: `is_recording`, `is_paused`, `is_processing`.

4. Finalização sem paste
- Em `actions.rs`, o caminho final não usa mais `utils::paste(...)`.
- Em vez disso, gera artefato `.txt` via `HistoryManager::save_transcript_artifact(...)`.

5. Download no History
- Novo comando backend: `export_transcript_file(id) -> path`.
- Frontend lê arquivo e dispara download via Blob/link.

---

## 5) Pendências imediatas (próxima sessão)

1. Validar compilação e testes completos
- Ainda não foi possível executar build/test local neste container por ausência de toolchain (rust/cargo/bun etc.).

2. Revisão final de remoção de legado
- Confirmar se resta referência funcional obrigatória a tray/hotkey/paste nos caminhos críticos.

3. i18n
- Alguns labels de download usam `defaultValue`; opcionalmente adicionar chaves de tradução.

4. Branding
- Ainda há referências de nome antigo em metadados (`Cargo.toml`, título etc.).

5. Commit de checkpoint
- Gerar commit(s) lógicos após validação de build.

---

## 6) Limitação de ambiente observada
Neste container atual faltam ferramentas para ciclo completo de desenvolvimento do projeto:
- `rustc`, `cargo`, `bun`, `cmake`, `clang`, etc.

Node/Python já existem no ambiente atual (Node 20, npm, pnpm, Python 3.11), mas não bastam para build Tauri/Rust.

---

## 7) Comandos sugeridos ao retomar

```bash
cd /projects/memorata
git status

# instalar deps JS
bun install

# gerar/revalidar bindings (se necessário)
bun run tauri dev

# build frontend
bun run build

# build tauri
bun run tauri build
```

(ajustar conforme Dockerfile novo)

---

## 8) Estado git atual
Há alterações locais não commitadas em múltiplos arquivos (backend + frontend + docs).