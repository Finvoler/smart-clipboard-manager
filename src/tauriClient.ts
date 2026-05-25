import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppSettings, ClipboardItem, Folder, QuickItem } from './types';

const hasTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

const demoItems: ClipboardItem[] = [
  {
    id: 'demo-md',
    kind: 'text',
    content: '### Markdown + LaTeX\n\nEuler identity: $e^{i\\pi}+1=0$\n\n```cpp\n#include <iostream>\nint main() { std::cout << "clipboard"; }\n```',
    preview: 'Markdown + LaTeX + C++ code sample',
    isStar: false,
    createdAt: Math.floor(Date.now() / 1000),
    updatedAt: Math.floor(Date.now() / 1000),
  },
];

const demoSettings: AppSettings = {
  appEnabled: true,
  captureEnabled: true,
  interceptWinV: true,
  runAtStartup: false,
  hideConsoleWindow: true,
  aiProtocol: 'openai',
  openaiBaseUrl: 'https://api.xiaomimimo.com/v1',
  anthropicBaseUrl: 'https://api.xiaomimimo.com/anthropic',
  apiKey: '',
  searchModel: 'mimo-v2.5-pro',
  ocrModel: 'mimo-v2.5',
};

export function fileSrc(path?: string | null): string | undefined {
  if (!path) return undefined;
  return hasTauri ? convertFileSrc(path) : path;
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (hasTauri) return invoke<T>(command, args);

  if (command === 'get_history') return demoItems as T;
  if (command === 'get_folders') return [] as T;
  if (command === 'get_quick_pool') return [] as T;
  if (command === 'get_app_settings') return demoSettings as T;
  if (command === 'save_app_settings') return { ...demoSettings, ...(args?.settings as Partial<AppSettings> | undefined) } as T;
  if (command === 'test_ai_connection') return 'Demo runtime: Tauri is not available' as T;
  if (command === 'list_ai_models') return ['mimo-v2.5-pro', 'mimo-v2.5', 'mimo-v2-flash'] as T;
  if (command === 'search_local') return demoItems as T;
  if (command === 'hide_window') return undefined as T;
  if (command === 'execute_paste') return undefined as T;
  throw new Error(`Tauri runtime is unavailable for ${command}`);
}

export async function onNewItem(callback: (item: ClipboardItem) => void): Promise<() => void> {
  if (!hasTauri) return () => undefined;
  const unlisten = await listen<ClipboardItem>('on_new_item', (event) => callback(event.payload));
  return unlisten;
}

export async function onQuickPoolExtracted(callback: (item: QuickItem) => void): Promise<() => void> {
  if (!hasTauri) return () => undefined;
  const unlisten = await listen<QuickItem>('on_quick_pool_extracted', (event) => callback(event.payload));
  return unlisten;
}

export type { AppSettings, ClipboardItem, Folder, QuickItem };
