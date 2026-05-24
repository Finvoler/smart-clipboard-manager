import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ClipboardItem, Folder, QuickItem } from './types';

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

export function fileSrc(path?: string | null): string | undefined {
  if (!path) return undefined;
  return hasTauri ? convertFileSrc(path) : path;
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (hasTauri) return invoke<T>(command, args);

  if (command === 'get_history') return demoItems as T;
  if (command === 'get_folders') return [] as T;
  if (command === 'get_quick_pool') return [] as T;
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

export type { ClipboardItem, Folder, QuickItem };
