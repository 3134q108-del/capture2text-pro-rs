import { invoke } from "@tauri-apps/api/core";

export type VoiceLevel = "Standard" | "HighDefinition";

export type AzureVoice = {
  id: string;
  name: string;
  locale: string;
  gender: string;
  level: VoiceLevel;
  sample_rate: number;
};

export type AzureCredentialsStatus = {
  configured: boolean;
  region: string | null;
};

export function saveAzureCredentials(key: string, region: string): Promise<void> {
  return invoke("save_azure_credentials", { key, region });
}

export function getAzureCredentialsStatus(): Promise<AzureCredentialsStatus> {
  return invoke("get_azure_credentials_status");
}

export function deleteAzureCredentials(): Promise<void> {
  return invoke("delete_azure_credentials");
}

export function testAzureConnection(): Promise<void> {
  return invoke("test_azure_connection");
}

export function listAzureVoices(lang: string): Promise<AzureVoice[]> {
  return invoke("list_azure_voices", { lang });
}
