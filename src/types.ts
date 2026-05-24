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
