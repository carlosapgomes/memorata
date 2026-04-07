import React from "react";
import { useTranslation } from "react-i18next";
import { Dropdown, SettingContainer, SettingsGroup } from "../../ui";
import { useSettings } from "../../../hooks/useSettings";
import { ApiKeyField } from "../PostProcessingSettingsApi/ApiKeyField";
import type { TranscriptionBackend } from "@/bindings";

const BACKEND_OPTIONS: Array<{
  value: TranscriptionBackend;
  labelKey: string;
  defaultLabel: string;
}> = [
  {
    value: "local",
    labelKey: "settings.general.transcription.backend.options.local",
    defaultLabel: "Local",
  },
  {
    value: "assembly_ai",
    labelKey: "settings.general.transcription.backend.options.assemblyAi",
    defaultLabel: "AssemblyAI",
  },
];

const LANGUAGE_OPTIONS = [
  {
    value: "auto",
    labelKey: "settings.general.transcription.assemblyAiLanguage.options.auto",
    defaultLabel: "Auto",
  },
  {
    value: "pt-BR",
    labelKey: "settings.general.transcription.assemblyAiLanguage.options.ptBR",
    defaultLabel: "Portuguese (Brazil)",
  },
  {
    value: "en",
    labelKey: "settings.general.transcription.assemblyAiLanguage.options.en",
    defaultLabel: "English",
  },
  {
    value: "en_us",
    labelKey: "settings.general.transcription.assemblyAiLanguage.options.enUS",
    defaultLabel: "English (US)",
  },
];

export const TranscriptionSettingsCard: React.FC = () => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();

  const transcriptionBackend =
    (settings?.transcription_backend as TranscriptionBackend | undefined) ??
    "local";
  const assemblyApiKey = settings?.assembly_ai_api_key ?? "";
  const assemblyLanguage = settings?.assembly_ai_language_code ?? "auto";
  const languageOptions = React.useMemo(() => {
    const options = LANGUAGE_OPTIONS.map((option) => ({
      value: option.value,
      label: t(option.labelKey, { defaultValue: option.defaultLabel }),
    }));

    if (!options.some((option) => option.value === assemblyLanguage)) {
      options.push({
        value: assemblyLanguage,
        label: assemblyLanguage,
      });
    }

    return options;
  }, [assemblyLanguage, t]);

  return (
    <SettingsGroup
      title={t("settings.general.transcription.title", {
        defaultValue: "Transcription",
      })}
    >
      <SettingContainer
        title={t("settings.general.transcription.backend.title", {
          defaultValue: "Backend",
        })}
        description={t("settings.general.transcription.backend.description", {
          defaultValue:
            "Choose whether transcriptions use a local model or the AssemblyAI API.",
        })}
        descriptionMode="tooltip"
        grouped
      >
        <Dropdown
          options={BACKEND_OPTIONS.map((option) => ({
            value: option.value,
            label: t(option.labelKey, { defaultValue: option.defaultLabel }),
          }))}
          selectedValue={transcriptionBackend}
          onSelect={(value) =>
            updateSetting(
              "transcription_backend",
              value as TranscriptionBackend,
            )
          }
          disabled={isUpdating("transcription_backend")}
        />
      </SettingContainer>

      <SettingContainer
        title={t("settings.general.transcription.assemblyAiApiKey.title", {
          defaultValue: "AssemblyAI API Key",
        })}
        description={t(
          "settings.general.transcription.assemblyAiApiKey.description",
          {
            defaultValue:
              "Used when the transcription backend is set to AssemblyAI.",
          },
        )}
        descriptionMode="tooltip"
        grouped
      >
        <ApiKeyField
          value={assemblyApiKey}
          onBlur={(value) => updateSetting("assembly_ai_api_key", value)}
          placeholder={t(
            "settings.general.transcription.assemblyAiApiKey.placeholder",
            {
              defaultValue: "Paste a new AssemblyAI API key",
            },
          )}
          maskedPlaceholder={t("settings.apiKeys.maskedPlaceholder", {
            defaultValue: "•••••••• saved",
          })}
          disabled={isUpdating("assembly_ai_api_key")}
          className="min-w-[320px]"
        />
      </SettingContainer>

      <SettingContainer
        title={t("settings.general.transcription.assemblyAiLanguage.title", {
          defaultValue: "AssemblyAI Language",
        })}
        description={t(
          "settings.general.transcription.assemblyAiLanguage.description",
          {
            defaultValue:
              "Choose the language sent to AssemblyAI. Auto enables automatic language detection.",
          },
        )}
        descriptionMode="tooltip"
        grouped
      >
        <Dropdown
          options={languageOptions}
          selectedValue={assemblyLanguage}
          onSelect={(value) =>
            updateSetting("assembly_ai_language_code", value)
          }
          disabled={isUpdating("assembly_ai_language_code")}
        />
      </SettingContainer>
    </SettingsGroup>
  );
};
