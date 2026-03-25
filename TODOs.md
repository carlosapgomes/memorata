# TODOs — Memorata

Atualizado em: 2026-03-24

## Pendências principais

- [x] Feedback visual de gravação
  - [x] Ícone de gravação pulsante
  - [x] Timer (`hh:mm:ss`)
  - [x] Estado de pausa visualmente distinto

- [x] Download no histórico
  - [x] Validar fluxo ponta a ponta (frontend -> comando Tauri -> arquivo final)
  - [x] Mostrar feedback claro de sucesso/erro no download

- [x] Remover resquícios de branding "Handy"
  - [x] Trocar ícone grande no canto superior esquerdo
  - [x] Ajustar cor e tamanho para manter consistência visual

- [ ] Item 6: Diarização AssemblyAI com configuração por sessão
  - [x] A) Tipos de opção de sessão (backend)
    - [x] Struct `StartSessionOptions` com `enable_diarization` e `speakers_expected`
    - [x] Validação: diarização ON exige speakers >= 2
    - [x] Normalização: diarização OFF força speakers = 1
    - [x] Default manual: diarização true, speakers 2
  - [x] B) Snapshot por sessão
    - [x] Coordinator armazena opções no Start
    - [x] Coordinator limpa opções no Stop/Cancel/ProcessingFinished
    - [x] `get_session_options()` para leitura do snapshot
  - [x] C) UI no RecordingSessionControls
    - [x] Checkbox "Diarização" (default true)
    - [x] Campo numérico "Speakers" (default 2)
    - [x] Checkbox OFF => speakers=1 e campo desabilitado
    - [x] Enviar opções no comando startRecordingSession
    - [x] Tratar erro `assembly_ai_speakers_expected_invalid`
    - [x] Desabilitar controles quando state != idle
  - [x] D) Integração AssemblyAI em transcription_service.rs
    - [x] Aceitar opções de sessão no fluxo transcribe
    - [x] Payload: incluir `speaker_labels: true` e `speakers_expected` quando diarização ON
    - [x] Struct para `utterances` na resposta
    - [x] Erro explícito se diarização ON mas utterances vazio/inválido
  - [x] E) Formatação do resultado diarizado
    - [x] Formato: `Speaker A: ...` / `Speaker B: ...`
    - [x] Ignorar utterances vazios
    - [x] Preservar ordem temporal
  - [x] F) Testes adicionais
    - [x] Teste de payload com diarização ON/OFF
    - [x] Teste de formatação de utterances
    - [x] Teste de erro sem utterances
  - [x] G) i18n + docs
    - [x] Textos EN/PT para labels de diarização
    - [x] Atualizar README com fluxo de configuração por sessão

## Melhorias opcionais

- [ ] Qualidade de escuta do WAV no histórico
  - [ ] Manter 16k para transcrição
  - [ ] Avaliar salvar WAV em taxa nativa para playback melhor

- [ ] UX de configuração
  - [ ] Adicionar seletor de backend na UI
  - [ ] Adicionar seletor de idioma do AssemblyAI na UI

## Feito nesta sessão

- [x] Corrigido bug de volume/ganho do WAV (pipeline i32/24-bit packed)
- [x] Nível de áudio do Memorata alinhado ao WAV de referência (FFmpeg)
- [x] Diagnóstico de transcrição ilegível no AssemblyAI concluído: faltava definir idioma (`pt`)
- [x] AssemblyAI: controle de idioma no app
  - [x] Adicionar `assembly_ai_language_code` na config (default `auto`)
  - [x] Enviar `language_code` quando definido (ex.: `pt`)
  - [x] Enviar `language_detection=true` quando `auto`
  - [x] Normalizar códigos (`pt-BR`/`pt_br` -> `pt`, `zh-Hans`/`zh-Hant` -> `zh`)
  - [x] Retornar erro explícito para código inválido
- [x] Feedback visual de processamento
  - [x] Indicador por etapas (`Preparando áudio` -> `Transcrevendo` -> `Salvando`)
  - [x] Ícone pulsante/piscando durante processamento
- [x] Feedback visual de gravação
  - [x] Ícone pulsante durante gravação
  - [x] Timer hh:mm:ss no overlay e na UI principal
  - [x] Estado de pausa visualmente distinto (ícone opaco, timer congelado)
  - [x] Backend emite eventos de estado `paused` para o overlay
- [x] Download de transcrição no histórico
  - [x] Comando `download_transcript_file` com diálogo de salvamento
  - [x] Toast de sucesso/erro no download
  - [x] Botão desabilitado durante operação (evitar cliques duplos)
