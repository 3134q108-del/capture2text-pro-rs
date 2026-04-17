import { invoke } from '@tauri-apps/api/core';

export async function readFile(path: string): Promise<string> {
  return invoke<string>('read_file', { path });
}
