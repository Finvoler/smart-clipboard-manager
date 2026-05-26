/* 前端共享类型定义，和 Rust models.rs 基本一一对应。 */

export type ClipboardKind = 'text' | 'image';

export interface ClipboardItem {
  id: string;
  kind: ClipboardKind;
  content?: string | null;
  imagePath?: string | null;
  preview: string;
  isStar: boolean;
  folderId?: string | null;
  createdAt: number;
  updatedAt: number;
  expiresAt?: number | null;
  mimeType?: string | null;
  width?: number | null;
  height?: number | null;
  imageHash?: string | null;
  ocrText?: string | null;
}

export interface Folder {
  id: string;
  name: string;
  createdAt: number;
}

export interface QuickItem {
  id: string;
  content: string;
  hitCount: number;
  createdAt: number;
  updatedAt: number;
  expiresAt?: number | null;
  isPinned: boolean;
}

export interface QuickSuggestion {
  id: string;
  content: string;
  hitCount: number;
  createdAt: number;
  updatedAt: number;
}

export interface AppSettings {
  appEnabled: boolean;
  captureEnabled: boolean;
  interceptWinV: boolean;
  runAtStartup: boolean;
  hideConsoleWindow: boolean;
  dataDirectory: string;
  resolvedDataDirectory: string;
  aiProtocol: 'openai' | 'anthropic';
  openaiBaseUrl: string;
  anthropicBaseUrl: string;
  apiKey: string;
  searchModel: string;
  ocrModel: string;
  language: 'zh' | 'en';
}

export interface DataDirectoryChangeResult {
  settings: AppSettings;
  message: string;
  restartRequired: boolean;
}
