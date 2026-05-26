/*
 * 前端主界面。
 *
 * 这是整个 React UI 的核心文件：侧边栏、历史列表、图片预览、编辑态、设置面板、
 * AI 搜索、AI 整理、临时池和多语言文案都集中在这里。
 */

import { useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { Archive, Bot, Check, ChevronDown, ChevronLeft, ChevronRight, Clock, CornerDownLeft, Edit3, Folder as FolderIcon, FolderOpen, FolderPlus, Image as ImageIcon, Pin, Power, RefreshCw, Save, Search, Settings, Star, TestTube2, Trash2, X } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import { open } from '@tauri-apps/plugin-dialog';
import { call, fileSrc, onNewItem, onQuickSuggestionDetected, type AppSettings, type ClipboardItem, type DataDirectoryChangeResult, type Folder, type QuickItem, type QuickSuggestion } from './tauriClient';

const DEFAULT_LIMIT = 120;

type SectionKey = 'api' | 'starred' | 'folders' | 'quickTools' | 'quickPending' | 'quickAccepted';
type NoticeTone = 'info' | 'loading' | 'success' | 'error';
type Language = 'zh' | 'en';

interface StatusNotice {
  message: string;
  tone: NoticeTone;
}

const COPY = {
  zh: {
    clipboardHistory: '剪贴板历史',
    clipboardSidebar: '剪贴板侧边栏',
    searchPlaceholder: '搜索剪贴板',
    aiSearchPlaceholder: 'AI 搜索剪贴板',
    clearSearch: '清空搜索',
    runAiSearch: '运行 AI 搜索',
    semanticSearch: 'LLM 语义搜索',
    aiArchive: 'AI 整理',
    toggleSidebar: '折叠侧边栏',
    enterSearch: '先输入要查找的内容',
    aiSearching: 'AI 正在查找...',
    aiFound: (count: number) => `AI 找到 ${count} 条相关记录`,
    aiOrganizing: 'AI 正在整理分类...',
    aiOrganized: (count: number) => `AI 已整理 ${count} 条记录`,
    settingsSaved: '已保存',
    dataDirectory: '文件保存路径',
    currentDataDirectory: '当前实际数据目录',
    chooseFolder: '选择文件夹',
    useDefaultPath: '恢复默认',
    dataDirectoryApply: '保存路径并重启',
    dataDirectoryPending: (target: string) => `待切换到: ${target}`,
    dataDirectoryHelp: '数据库、图片缓存和后续数据文件都会保存在这里。选择或输入路径后，点击“保存路径并重启”才会迁移并生效。',
    dataDirectoryConfirm: (target: string) => `确认把数据迁移到「${target}」？应用会自动重启。`,
    dataDirectoryRestarting: '正在切换数据目录并重启...',
    loadedModels: (count: number) => `已加载 ${count} 个模型`,
    clipboardEmpty: '剪贴板为空',
    apiKeyPasted: 'API key 已本地粘贴',
    pasteThisItem: '粘贴这条记录',
    star: '收藏',
    editText: '编辑文本',
    ocrImage: 'OCR 图片',
    delete: '删除',
    save: '保存',
    cancel: '取消',
    starred: '收藏',
    noStarred: '暂无收藏记录',
    deleteStarredRecord: '删除收藏记录',
    quickTools: '临时池',
    quickAccepted: '已加入临时池',
    quickPending: '待确认候选',
    temporaryPoolEmpty: '临时池为空',
    noPendingSuggestions: '暂无待确认候选',
    repeatedMeta: (count: number) => `重复 ${count} 次 · 5h 后删`,
    retentionLabel: '保留时间',
    hours24: '24 小时',
    days3: '3 天',
    days7: '7 天',
    accept: '加入',
    reject: '拒绝',
    folders: '文件夹',
    createFolder: '新建文件夹',
    noFoldersYet: '暂无文件夹',
    deleteFolder: '删除文件夹',
    deleteFolderConfirm: (name: string) => `删除文件夹「${name}」？里面的记录会回到普通历史，但仍按原过期时间自动清理。`,
    folderNamePrompt: '文件夹名称',
    noRecordsHere: '这里暂无记录',
    deleteRecord: '删除记录',
    moveSelected: '移动选中记录...',
    settings: '设置',
    settingsLoading: '设置加载中',
    language: '语言',
    captureClipboard: '记录剪贴板历史',
    startWithWindows: '开机启动',
    hideConsoleWindow: '隐藏控制台窗口',
    protocol: '协议',
    openaiCompatible: 'OpenAI 兼容',
    anthropicCompatible: 'Anthropic 兼容',
    openaiBaseUrl: 'OpenAI base URL',
    anthropicBaseUrl: 'Anthropic base URL',
    apiKey: 'API key',
    paste: '粘贴',
    searchArchiveModel: '搜索 / 整理模型',
    ocrModel: 'OCR 模型',
    test: '测试',
    models: '模型',
    imageDimensions: (width?: number | null, height?: number | null) => `${width ?? '?'} x ${height ?? '?'}`,
    fullPreview: '完整预览',
    ocrText: 'OCR 文本',
    pasteOcrText: '粘贴 OCR 文本',
    ocrReady: 'OCR 文本已写回图片记录',
    collect: '收藏',
    edit: '编辑',
  },
  en: {
    clipboardHistory: 'Clipboard History',
    clipboardSidebar: 'Clipboard Sidebar',
    searchPlaceholder: 'Search clipboard',
    aiSearchPlaceholder: 'AI search clipboard',
    clearSearch: 'Clear search',
    runAiSearch: 'Run AI search',
    semanticSearch: 'LLM semantic search',
    aiArchive: 'AI archive',
    toggleSidebar: 'Toggle sidebar',
    enterSearch: 'Enter something to search',
    aiSearching: 'AI is searching...',
    aiFound: (count: number) => `AI found ${count} matching records`,
    aiOrganizing: 'AI is organizing records...',
    aiOrganized: (count: number) => `AI organized ${count} records`,
    settingsSaved: 'Saved',
    dataDirectory: 'Data directory',
    currentDataDirectory: 'Current active data directory',
    chooseFolder: 'Choose folder',
    useDefaultPath: 'Use default',
    dataDirectoryApply: 'Save path and restart',
    dataDirectoryPending: (target: string) => `Pending target: ${target}`,
    dataDirectoryHelp: 'The database, image cache, and future data files will be stored here. Choose a folder or type a path, then click Save path and restart to migrate and apply it.',
    dataDirectoryConfirm: (target: string) => `Move existing data to "${target}" and restart the app?`,
    dataDirectoryRestarting: 'Switching data directory and restarting...',
    loadedModels: (count: number) => `Loaded ${count} models`,
    clipboardEmpty: 'Clipboard is empty',
    apiKeyPasted: 'API key pasted locally',
    pasteThisItem: 'Paste this item',
    star: 'Star',
    editText: 'Edit text',
    ocrImage: 'OCR image',
    delete: 'Delete',
    save: 'Save',
    cancel: 'Cancel',
    starred: 'Starred',
    noStarred: 'No starred records',
    deleteStarredRecord: 'Delete starred record',
    quickTools: 'QuickTools',
    quickAccepted: 'Accepted Pool',
    quickPending: 'Pending Suggestions',
    temporaryPoolEmpty: 'Temporary pool is empty',
    noPendingSuggestions: 'No pending suggestions',
    repeatedMeta: (count: number) => `Repeated ${count} times · deletes after 5h`,
    retentionLabel: 'Retention',
    hours24: '24 hours',
    days3: '3 days',
    days7: '7 days',
    accept: 'Add',
    reject: 'Reject',
    folders: 'Folders',
    createFolder: 'Create folder',
    noFoldersYet: 'No folders yet',
    deleteFolder: 'Delete folder',
    deleteFolderConfirm: (name: string) => `Delete folder "${name}"? Its records return to normal history and keep their original expiration.`,
    folderNamePrompt: 'Folder name',
    noRecordsHere: 'No records here',
    deleteRecord: 'Delete record',
    moveSelected: 'Move selected...',
    settings: 'Settings',
    settingsLoading: 'Settings are loading',
    language: 'Language',
    captureClipboard: 'Record clipboard history',
    startWithWindows: 'Start with Windows',
    hideConsoleWindow: 'Hide console window',
    protocol: 'Protocol',
    openaiCompatible: 'OpenAI compatible',
    anthropicCompatible: 'Anthropic compatible',
    openaiBaseUrl: 'OpenAI base URL',
    anthropicBaseUrl: 'Anthropic base URL',
    apiKey: 'API key',
    paste: 'Paste',
    searchArchiveModel: 'Search / archive model',
    ocrModel: 'OCR model',
    test: 'Test',
    models: 'Models',
    imageDimensions: (width?: number | null, height?: number | null) => `${width ?? '?'} x ${height ?? '?'}`,
    fullPreview: 'Full preview',
    ocrText: 'OCR text',
    pasteOcrText: 'Paste OCR text',
    ocrReady: 'OCR text saved on this image record',
    collect: 'Star',
    edit: 'Edit',
  },
} as const;

type Copy = {
  [Key in keyof typeof COPY.zh]: typeof COPY.zh[Key] extends (...args: infer Args) => string ? (...args: Args) => string : string;
};

export function App() {
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [quickItems, setQuickItems] = useState<QuickItem[]>([]);
  const [quickSuggestions, setQuickSuggestions] = useState<QuickSuggestion[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [savedSettings, setSavedSettings] = useState<AppSettings | null>(null);
  const [modelOptions, setModelOptions] = useState<string[]>(['mimo-v2.5-pro', 'mimo-v2.5', 'mimo-v2-flash']);
  const [query, setQuery] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingText, setEditingText] = useState('');
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [status, setStatus] = useState<StatusNotice | null>(null);
  const [settingsStatus, setSettingsStatus] = useState('');
  const [aiSearchMode, setAiSearchMode] = useState(false);
  const [aiSearchRunning, setAiSearchRunning] = useState(false);
  const [categorizeRunning, setCategorizeRunning] = useState(false);
  const [pendingKeys, setPendingKeys] = useState<Set<string>>(() => new Set());
  const [openSections, setOpenSections] = useState<Record<SectionKey, boolean>>({
    api: false,
    starred: true,
    folders: true,
    quickTools: true,
    quickPending: true,
    quickAccepted: true,
  });
  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const pendingKeysRef = useRef<Set<string>>(new Set());
  const language: Language = settings?.language === 'en' ? 'en' : 'zh';
  const copy = COPY[language];

  function isPending(key: string) {
    return pendingKeys.has(key);
  }

  function setPendingKey(key: string, pending: boolean) {
    const next = new Set(pendingKeysRef.current);
    if (pending) {
      next.add(key);
    } else {
      next.delete(key);
    }
    pendingKeysRef.current = next;
    setPendingKeys(next);
  }

  async function runWithPending<T>(key: string, task: () => Promise<T>): Promise<T | null> {
    if (pendingKeysRef.current.has(key)) return null;
    setPendingKey(key, true);
    try {
      return await task();
    } finally {
      setPendingKey(key, false);
    }
  }

  async function refresh() {
    const [history, folderList, pool, suggestions, appSettings] = await Promise.all([
      call<ClipboardItem[]>('get_history', { limit: DEFAULT_LIMIT, offset: 0 }),
      call<Folder[]>('get_folders'),
      call<QuickItem[]>('get_quick_pool'),
      call<QuickSuggestion[]>('get_quick_suggestions'),
      call<AppSettings>('get_app_settings'),
    ]);
    setItems(history);
    setFolders(folderList);
    setQuickItems(pool);
    setQuickSuggestions(suggestions);
    setSettings(appSettings);
    setSavedSettings(appSettings);
    setSelectedId((current) => current ?? history[0]?.id ?? null);
  }

  useEffect(() => {
    void refresh();

    const cleanups: Array<() => void> = [];
    void onNewItem((item) => {
      setItems((current) => [item, ...current.filter((candidate) => candidate.id !== item.id)]);
      setSelectedId(item.id);
    }).then((cleanup) => cleanups.push(cleanup));

    void onQuickSuggestionDetected((item) => {
      setQuickSuggestions((current) => [item, ...current.filter((candidate) => candidate.id !== item.id)]);
    }).then((cleanup) => cleanups.push(cleanup));

    return () => cleanups.forEach((cleanup) => cleanup());
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const isTyping = Boolean(target?.closest('input, textarea, select, [contenteditable="true"]'));
      if (event.key === 'Escape') {
        event.preventDefault();
        void call('hide_window');
      }

      if (!isTyping && event.key === 'Enter' && selectedId && editingId === null) {
        event.preventDefault();
        const selected = items.find((item) => item.id === selectedId);
        if (selected) {
          void pasteClipboardItem(selected);
        } else {
          void executePaste(selectedId);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedId, editingId, items]);

  useEffect(() => {
    if (!status || status.tone === 'loading') return;
    const timer = window.setTimeout(() => setStatus(null), 5000);
    return () => window.clearTimeout(timer);
  }, [status]);

  const filteredItems = useMemo(() => {
    if (aiSearchMode) return items;
    if (!query.trim()) return items;
    const keyword = query.trim().toLowerCase();
    return items.filter((item) => `${item.preview} ${item.content ?? ''} ${item.ocrText ?? ''}`.toLowerCase().includes(keyword));
  }, [aiSearchMode, items, query]);

  function showStatus(message: string, tone: NoticeTone = 'info') {
    setStatus({ message, tone });
  }

  function toggleSection(key: SectionKey) {
    setOpenSections((current) => ({ ...current, [key]: !current[key] }));
  }

  async function runLocalSearch(keyword: string) {
    setQuery(keyword);
    if (!keyword.trim()) {
      await refresh();
      return;
    }
    const result = await call<ClipboardItem[]>('search_local', { keyword });
    setItems(result);
    setSelectedId(result[0]?.id ?? null);
  }

  function handleSearchChange(value: string) {
    if (aiSearchMode) {
      setQuery(value);
      return;
    }
    void runLocalSearch(value);
  }

  async function clearSearch() {
    await runWithPending('search:clear', async () => {
      setQuery('');
      await refresh();
      searchInputRef.current?.focus();
    });
  }

  function toggleAiSearchMode() {
    setAiSearchMode((current) => {
      const next = !current;
      if (current) {
        setAiSearchRunning(false);
        setStatus(null);
        void runLocalSearch(query);
      } else {
        window.setTimeout(() => searchInputRef.current?.focus(), 20);
      }
      return next;
    });
  }

  async function runAiSearch() {
    if (!query.trim()) {
      showStatus(copy.enterSearch, 'error');
      return;
    }
    await runWithPending('ai:search', async () => {
      setAiSearchRunning(true);
      showStatus(copy.aiSearching, 'loading');
      try {
        const ids = await call<string[]>('search_ai_semantic', { query });
        setItems((current) => {
          const next = current.filter((item) => ids.includes(item.id));
          setSelectedId(next[0]?.id ?? null);
          return next;
        });
        showStatus(copy.aiFound(ids.length), 'success');
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      } finally {
        setAiSearchRunning(false);
      }
    });
  }

  function updateSettings(patch: Partial<AppSettings>) {
    setSettings((current) => current ? { ...current, ...patch } : current);
  }

  async function persistSettings(nextSettings = settings) {
    if (!nextSettings) return null;
    const saved = await call<AppSettings>('save_app_settings', { settings: { ...nextSettings, appEnabled: true } });
    setSettings(saved);
    setSavedSettings(saved);
    setSettingsStatus(copy.settingsSaved);
    return saved;
  }

  function hasPendingDataDirectoryChange(nextSettings: AppSettings) {
    return nextSettings.dataDirectory.trim() !== (savedSettings?.dataDirectory.trim() ?? '');
  }

  async function saveSettings(nextSettings = settings) {
    const pendingDataDirectoryChange = !!nextSettings && hasPendingDataDirectoryChange(nextSettings);
    await runWithPending('settings:save', async () => {
      try {
        if (!nextSettings) return;
        if (pendingDataDirectoryChange) {
          // 数据目录切换不是普通设置保存：Rust 端会先写 bootstrap pending，再通过重启完成迁移。
          const targetLabel = nextSettings.dataDirectory.trim() || copy.useDefaultPath;
          const confirmed = window.confirm(copy.dataDirectoryConfirm(targetLabel));
          if (!confirmed) return;
          setSettingsStatus(copy.dataDirectoryRestarting);
          const result = await call<DataDirectoryChangeResult>('change_data_directory', {
            settings: { ...nextSettings, appEnabled: true },
          });
          setSettings(result.settings);
          setSavedSettings(result.settings);
          setSettingsStatus(result.message);
          if (result.restartRequired) {
            try {
              await call('restart_application');
            } catch {
              // The app exits during restart, so invoke can disconnect before resolving.
            }
          }
          return;
        }
        await persistSettings(nextSettings);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        setSettingsStatus(message);
        if (pendingDataDirectoryChange) {
          window.alert(message);
        }
      }
    });
  }

  async function testAiConnection() {
    await runWithPending('settings:test', async () => {
      try {
        await persistSettings();
        const result = await call<string>('test_ai_connection');
        setSettingsStatus(result);
      } catch (error) {
        setSettingsStatus(error instanceof Error ? error.message : String(error));
      }
    });
  }

  async function refreshModelOptions() {
    await runWithPending('settings:models', async () => {
      try {
        await persistSettings();
        const models = await call<string[]>('list_ai_models');
        setModelOptions(models);
        setSettingsStatus(copy.loadedModels(models.length));
      } catch (error) {
        setSettingsStatus(error instanceof Error ? error.message : String(error));
      }
    });
  }

  async function pasteApiKeyFromClipboard() {
    await runWithPending('settings:pasteKey', async () => {
      try {
        const apiKey = (await navigator.clipboard.readText()).trim();
        if (!apiKey) {
          setSettingsStatus(copy.clipboardEmpty);
          return;
        }
        updateSettings({ apiKey });
        setSettingsStatus(copy.apiKeyPasted);
      } catch (error) {
        setSettingsStatus(error instanceof Error ? error.message : String(error));
      }
    });
  }

  async function chooseDataDirectory() {
    if (!settings) return;
    await runWithPending('settings:path', async () => {
      try {
        const selected = await open({
          directory: true,
          multiple: false,
          defaultPath: settings.dataDirectory || settings.resolvedDataDirectory || undefined,
        });
        const selectedPath = Array.isArray(selected) ? selected[0] : selected;
        if (typeof selectedPath === 'string' && selectedPath.trim()) {
          updateSettings({ dataDirectory: selectedPath });
          setSettingsStatus('');
        }
      } catch (error) {
        setSettingsStatus(error instanceof Error ? error.message : String(error));
      }
    });
  }

  function resetDataDirectory() {
    updateSettings({ dataDirectory: '' });
    setSettingsStatus('');
  }

  async function executePaste(id: string, overrideText?: string, pendingKey = `paste:${id || overrideText || 'override'}`) {
    await runWithPending(pendingKey, async () => {
      try {
        await call('execute_paste', { itemId: id, overrideText });
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function pasteClipboardItem(item: ClipboardItem) {
    await executePaste(item.id, undefined, `paste:item:${item.id}`);
  }

  async function acceptSuggestion(id: string, ttl: number) {
    await runWithPending(`suggestion:${id}`, async () => {
      try {
        const accepted = await call<QuickItem>('accept_quick_suggestion', { id, ttl });
        setQuickSuggestions((current) => current.filter((item) => item.id !== id));
        setQuickItems((current) => [accepted, ...current.filter((item) => item.id !== accepted.id)]);
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function dismissSuggestion(id: string) {
    await runWithPending(`suggestion:${id}`, async () => {
      try {
        await call('dismiss_quick_suggestion', { id });
        setQuickSuggestions((current) => current.filter((item) => item.id !== id));
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function deleteQuickItem(id: string) {
    await runWithPending(`quick:${id}`, async () => {
      try {
        await call('delete_quick_item', { id });
        setQuickItems((current) => current.filter((item) => item.id !== id));
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function starQuickItem(id: string) {
    await runWithPending(`quick:${id}`, async () => {
      try {
        const starred = await call<ClipboardItem>('star_quick_item', { id });
        setQuickItems((current) => current.filter((item) => item.id !== id));
        setItems((current) => [starred, ...current.filter((item) => item.id !== starred.id)]);
        setSelectedId(starred.id);
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  function pasteFromRecord(event: MouseEvent<HTMLElement>, item: ClipboardItem) {
    const target = event.target as HTMLElement;
    if (editingId === item.id || target.closest('button, input, textarea, select, a')) return;
    void pasteClipboardItem(item);
  }

  function stopAndRun(event: MouseEvent, action: () => void) {
    event.stopPropagation();
    action();
  }

  async function saveEdit(id: string) {
    await runWithPending(`edit:${id}`, async () => {
      try {
        const updated = await call<ClipboardItem>('update_item_text', { id, text: editingText });
        setItems((current) => current.map((item) => (item.id === id ? updated : item)));
        setEditingId(null);
        setEditingText('');
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function toggleStar(item: ClipboardItem) {
    await runWithPending(`star:${item.id}`, async () => {
      try {
        const updated = await call<ClipboardItem>('toggle_star', { id: item.id, isStar: !item.isStar });
        setItems((current) => current.map((candidate) => (candidate.id === item.id ? updated : candidate)));
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function removeItem(id: string) {
    await runWithPending(`delete:${id}`, async () => {
      try {
        await call('delete_item', { id });
        setItems((current) => current.filter((item) => item.id !== id));
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function deleteFolder(folder: Folder) {
    const confirmed = window.confirm(copy.deleteFolderConfirm(folder.name));
    if (!confirmed) return;
    await runWithPending(`folder:delete:${folder.id}`, async () => {
      try {
        await call('delete_folder', { id: folder.id });
        setFolders((current) => current.filter((item) => item.id !== folder.id));
        setItems((current) => current.map((item) => item.folderId === folder.id ? { ...item, folderId: null } : item));
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function moveToFolder(itemId: string, folderId: string | null) {
    await runWithPending(`folder:move:${folderId ?? 'none'}:${itemId}`, async () => {
      try {
        const updated = await call<ClipboardItem>('move_to_folder', { itemId, folderId });
        setItems((current) => current.map((item) => (item.id === itemId ? updated : item)));
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function createFolder() {
    const name = window.prompt(copy.folderNamePrompt);
    if (!name?.trim()) return;
    await runWithPending('folder:create', async () => {
      try {
        const folder = await call<Folder>('create_folder', { name: name.trim() });
        setFolders((current) => [...current, folder]);
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function categorize() {
    await runWithPending('ai:categorize', async () => {
      setCategorizeRunning(true);
      showStatus(copy.aiOrganizing, 'loading');
      try {
        const updated = await call<ClipboardItem[]>('trigger_ai_categorize');
        showStatus(copy.aiOrganized(updated.length), 'success');
        await refresh();
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      } finally {
        setCategorizeRunning(false);
      }
    });
  }

  async function runOcr(item: ClipboardItem) {
    await runWithPending(`ocr:${item.id}`, async () => {
      try {
        const updated = await call<ClipboardItem>('trigger_ocr', { imageId: item.id });
        setItems((current) => current.map((candidate) => candidate.id === updated.id ? updated : candidate));
        setSelectedId(updated.id);
        showStatus(copy.ocrReady, 'success');
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  const aiSearchPending = aiSearchRunning || isPending('ai:search');
  const categorizePending = categorizeRunning || isPending('ai:categorize');
  const settingsBusy = ['settings:save', 'settings:test', 'settings:models', 'settings:pasteKey', 'settings:path'].some(isPending);

  return (
    <main className={`shell ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
      <section className="timeline" aria-label={copy.clipboardHistory}>
        <header className={`toolbar ${aiSearchMode ? 'aiSearchActive' : ''}`}>
          <div className={`searchBox ${aiSearchMode ? 'aiActive' : ''}`}>
            {aiSearchMode ? <Bot size={16} /> : <Search size={16} />}
            <input
              ref={searchInputRef}
              value={query}
              onChange={(event) => handleSearchChange(event.target.value)}
              onKeyDown={(event) => {
                if (aiSearchMode && event.key === 'Enter') {
                  event.preventDefault();
                  void runAiSearch();
                }
              }}
              placeholder={aiSearchMode ? copy.aiSearchPlaceholder : copy.searchPlaceholder}
            />
            {query ? (
              <button className="searchClearButton" onClick={() => void clearSearch()} disabled={isPending('search:clear')} title={copy.clearSearch} type="button">
                <X size={15} />
              </button>
            ) : null}
          </div>
          {aiSearchMode ? (
            <button className="iconButton confirmButton" onClick={() => void runAiSearch()} disabled={aiSearchPending || !query.trim()} title={copy.runAiSearch}>
              <CornerDownLeft size={17} />
            </button>
          ) : null}
          <button className={`iconButton ${aiSearchMode ? 'active' : ''}`} onClick={toggleAiSearchMode} disabled={aiSearchPending} title={copy.semanticSearch}>
            <Bot size={17} />
          </button>
          <button className={`iconButton ${categorizePending ? 'active loading' : ''}`} onClick={() => void categorize()} disabled={categorizePending} title={copy.aiArchive}>
            <Archive size={17} />
          </button>
        </header>

        {status ? <StatusLine notice={status} onClose={() => setStatus(null)} /> : null}

        <div className="historyList">
          {filteredItems.map((item) => (
            <article
              key={item.id}
              className={`historyItem ${selectedId === item.id ? 'selected' : ''}`}
              onMouseEnter={() => setSelectedId(item.id)}
              onClick={(event) => pasteFromRecord(event, item)}
              onKeyDown={(event) => {
                if (event.key === 'Enter' && editingId !== item.id) {
                  event.preventDefault();
                  void pasteClipboardItem(item);
                }
              }}
              tabIndex={0}
            >
              <div className="itemHeader">
                <button className="pasteTarget" onClick={(event) => stopAndRun(event, () => void pasteClipboardItem(item))} disabled={isPending(`paste:item:${item.id}`) || isPending(`delete:${item.id}`)} title={copy.pasteThisItem}>
                  {item.kind === 'image' ? <ImageIcon size={16} /> : <Clock size={16} />}
                  <span>{formatTime(item.createdAt)}</span>
                </button>
                <div className="itemActions">
                  <button className="iconButton small" onClick={(event) => stopAndRun(event, () => void toggleStar(item))} disabled={isPending(`star:${item.id}`) || isPending(`delete:${item.id}`)} title={copy.star}>
                    <Star size={15} fill={item.isStar ? 'currentColor' : 'none'} />
                  </button>
                  {item.kind === 'text' ? (
                    <button
                      className="iconButton small"
                      disabled={isPending(`edit:${item.id}`) || isPending(`delete:${item.id}`)}
                      onClick={(event) => stopAndRun(event, () => {
                        setEditingId(item.id);
                        setEditingText(item.content ?? '');
                      })}
                      title={copy.editText}
                    >
                      <Edit3 size={15} />
                    </button>
                  ) : (
                    <button className={`iconButton small ${isPending(`ocr:${item.id}`) ? 'active loading' : ''}`} onClick={(event) => stopAndRun(event, () => void runOcr(item))} disabled={isPending(`ocr:${item.id}`) || isPending(`delete:${item.id}`)} title={copy.ocrImage} aria-busy={isPending(`ocr:${item.id}`)}>
                      <Bot size={15} />
                    </button>
                  )}
                  <button className="iconButton small danger" onClick={(event) => stopAndRun(event, () => void removeItem(item.id))} disabled={isPending(`delete:${item.id}`)} title={copy.delete}>
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>

              {editingId === item.id ? (
                <div className="editorBlock">
                  <textarea value={editingText} disabled={isPending(`edit:${item.id}`)} onChange={(event) => setEditingText(event.target.value)} />
                  <div className="editorActions">
                    <button onClick={() => saveEdit(item.id)} disabled={isPending(`edit:${item.id}`)}>{copy.save}</button>
                    <button onClick={() => setEditingId(null)} disabled={isPending(`edit:${item.id}`)}>{copy.cancel}</button>
                  </div>
                </div>
              ) : item.kind === 'image' ? (
                <ImagePreview item={item} copy={copy} pasteOcrPending={isPending(`paste:ocr:${item.id}`)} onPasteOcr={(text) => void executePaste('', text, `paste:ocr:${item.id}`)} />
              ) : (
                <div className="markdownButton" role="button" tabIndex={-1}>
                  <ReactMarkdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeKatex, rehypeHighlight]}>
                    {item.content ?? item.preview}
                  </ReactMarkdown>
                </div>
              )}
            </article>
          ))}
        </div>
      </section>

      <aside className="sidebar" aria-label={copy.clipboardSidebar}>
        <button className="collapseButton" onClick={() => setSidebarCollapsed((value) => !value)} title={copy.toggleSidebar}>
          {sidebarCollapsed ? <ChevronLeft size={17} /> : <ChevronRight size={17} />}
        </button>

        <SidebarSection title={copy.starred} icon={<Star size={15} />} open={openSections.starred} onToggle={() => toggleSection('starred')}>
          {items.filter((item) => item.isStar).length === 0 ? <div className="emptyHint">{copy.noStarred}</div> : null}
          {items.filter((item) => item.isStar).slice(0, 8).map((item) => (
            <div key={item.id} className="starredRow">
              <button className="sideItem starredSideItem" onClick={() => pasteClipboardItem(item)} disabled={isPending(`paste:item:${item.id}`) || isPending(`delete:${item.id}`)}>{item.preview}</button>
              <button className="iconButton small danger starredDeleteButton" onClick={(event) => stopAndRun(event, () => void removeItem(item.id))} disabled={isPending(`delete:${item.id}`)} title={copy.deleteStarredRecord}>
                <Trash2 size={14} />
              </button>
            </div>
          ))}
        </SidebarSection>

        <SidebarSection title={copy.quickTools} icon={<Pin size={15} />} open={openSections.quickTools} onToggle={() => toggleSection('quickTools')}>
          <QuickFolder title={copy.quickAccepted} count={quickItems.length} open={openSections.quickAccepted} onToggle={() => toggleSection('quickAccepted')} tone="accepted">
            {quickItems.length === 0 ? <div className="emptyHint">{copy.temporaryPoolEmpty}</div> : null}
          {quickItems.map((item) => (
              <QuickPoolRow
                key={item.id}
                item={item}
                onPaste={(content) => executePaste('', content, `paste:quick:${item.id}`)}
                onStar={() => starQuickItem(item.id)}
                onUpdate={(updated) => setQuickItems((current) => current.map((candidate) => candidate.id === updated.id ? updated : candidate))}
                onDelete={() => deleteQuickItem(item.id)}
                pending={isPending(`quick:${item.id}`)}
                pastePending={isPending(`paste:quick:${item.id}`)}
                copy={copy}
              />
          ))}
          </QuickFolder>
          <QuickFolder title={copy.quickPending} count={quickSuggestions.length} open={openSections.quickPending} onToggle={() => toggleSection('quickPending')} tone="pending">
            {quickSuggestions.length === 0 ? <div className="emptyHint">{copy.noPendingSuggestions}</div> : null}
            {quickSuggestions.map((item) => (
              <QuickSuggestionRow key={item.id} item={item} copy={copy} pending={isPending(`suggestion:${item.id}`)} onAccept={(ttl) => acceptSuggestion(item.id, ttl)} onDismiss={() => dismissSuggestion(item.id)} />
            ))}
          </QuickFolder>
        </SidebarSection>

        <SidebarSection
          title={copy.folders}
          icon={<FolderIcon size={15} />}
          open={openSections.folders}
          onToggle={() => toggleSection('folders')}
          actions={<button className="iconButton small" onClick={(event) => stopAndRun(event, () => void createFolder())} disabled={isPending('folder:create')} title={copy.createFolder}><FolderPlus size={14} /></button>}
        >
          {folders.length === 0 ? <div className="emptyHint">{copy.noFoldersYet}</div> : null}
          {folders.map((folder) => (
            <FolderDropTarget
              key={folder.id}
              folder={folder}
              items={items}
              copy={copy}
              folderDeleting={isPending(`folder:delete:${folder.id}`)}
              isItemPastePending={(id) => isPending(`paste:item:${id}`)}
              isItemDeletePending={(id) => isPending(`delete:${id}`)}
              onMove={moveToFolder}
              onPaste={pasteClipboardItem}
              onDeleteFolder={deleteFolder}
              onDeleteItem={removeItem}
            />
          ))}
        </SidebarSection>

        <SidebarSection title={copy.settings} icon={<Settings size={15} />} open={openSections.api} onToggle={() => toggleSection('api')} className="settingsSection">
          {settings ? (
            <SettingsFields
              settings={settings}
              status={settingsStatus}
              modelOptions={modelOptions}
              copy={copy}
              busy={settingsBusy}
              dataDirectoryDirty={hasPendingDataDirectoryChange(settings)}
              onChange={updateSettings}
              onSave={() => void saveSettings()}
              onTest={() => void testAiConnection()}
              onRefreshModels={() => void refreshModelOptions()}
              onPasteKey={() => void pasteApiKeyFromClipboard()}
              onChooseDataDirectory={() => void chooseDataDirectory()}
              onResetDataDirectory={resetDataDirectory}
            />
          ) : <div className="emptyHint">{copy.settingsLoading}</div>}
        </SidebarSection>
      </aside>
    </main>
  );
}

function StatusLine({ notice, onClose }: { notice: StatusNotice; onClose: () => void }) {
  return (
    <div className={`statusLine ${notice.tone}`}>
      <span className={`statusPulse ${notice.tone === 'loading' ? '' : 'idle'}`} aria-hidden="true" />
      <span className="statusMessage">{notice.message}</span>
      <button className="statusClose" onClick={onClose} title="Close status"><X size={15} /></button>
    </div>
  );
}

function SidebarSection({ title, icon, open, onToggle, actions, className, children }: { title: string; icon: ReactNode; open: boolean; onToggle: () => void; actions?: ReactNode; className?: string; children: ReactNode }) {
  return (
    <section className={`sideSection ${className ?? ''} ${open ? 'open' : 'closed'}`}>
      <button className="sectionHeader" onClick={onToggle} type="button">
        {open ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
        {icon}
        <span>{title}</span>
      </button>
      {actions ? <div className="sectionActions">{actions}</div> : null}
      {open ? <div className="sectionBody">{children}</div> : null}
    </section>
  );
}

function QuickFolder({ title, count, open, onToggle, tone, children }: { title: string; count: number; open: boolean; onToggle: () => void; tone: 'pending' | 'accepted'; children: ReactNode }) {
  return (
    <div className={`quickFolder ${tone}`}>
      <button className="quickFolderHeader" onClick={onToggle} type="button">
        {open ? <FolderOpen size={15} /> : <FolderIcon size={15} />}
        <span>{title}</span>
        <span className="countBadge">{count}</span>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
      </button>
      {open ? <div className="quickFolderBody">{children}</div> : null}
    </div>
  );
}

function QuickSuggestionRow({ item, copy, pending, onAccept, onDismiss }: { item: QuickSuggestion; copy: Copy; pending: boolean; onAccept: (ttl: number) => Promise<void>; onDismiss: () => Promise<void>; }) {
  const [ttl, setTtl] = useState(String(24 * 60 * 60));

  return (
    <div className="suggestionRow">
      <div className="quickContentText">{item.content}</div>
      <div className="suggestionMeta">{copy.repeatedMeta(item.hitCount)}</div>
      <label className="quickRetentionLabel" aria-label={copy.retentionLabel}>
        <select value={ttl} onChange={(event) => setTtl(event.target.value)} disabled={pending}>
          <option value={String(24 * 60 * 60)}>{copy.hours24}</option>
          <option value={String(3 * 24 * 60 * 60)}>{copy.days3}</option>
          <option value={String(7 * 24 * 60 * 60)}>{copy.days7}</option>
        </select>
      </label>
      <div className="suggestionActions">
        <button onClick={() => void onAccept(Number(ttl))} disabled={pending}><Check size={14} /> {copy.accept}</button>
        <button className="dangerText" onClick={() => void onDismiss()} disabled={pending}><Trash2 size={14} /> {copy.reject}</button>
      </div>
    </div>
  );
}

function SettingsFields({ settings, status, modelOptions, copy, busy, dataDirectoryDirty, onChange, onSave, onTest, onRefreshModels, onPasteKey, onChooseDataDirectory, onResetDataDirectory }: { settings: AppSettings; status: string; modelOptions: string[]; copy: Copy; busy: boolean; dataDirectoryDirty: boolean; onChange: (patch: Partial<AppSettings>) => void; onSave: () => void; onTest: () => void; onRefreshModels: () => void; onPasteKey: () => void; onChooseDataDirectory: () => void; onResetDataDirectory: () => void; }) {
  const dataDirectoryTarget = settings.dataDirectory.trim() || copy.useDefaultPath;

  return (
    <div className="settingsFields">
      <datalist id="mimo-models">
        {modelOptions.map((model) => <option key={model} value={model} />)}
      </datalist>

      <label className="toggleRow">
        <input type="checkbox" checked={settings.captureEnabled} disabled={busy} onChange={(event) => onChange({ captureEnabled: event.target.checked })} />
        <span><Power size={14} /> {copy.captureClipboard}</span>
      </label>

      <label className="toggleRow">
        <input type="checkbox" checked={settings.runAtStartup} disabled={busy} onChange={(event) => onChange({ runAtStartup: event.target.checked })} />
        <span>{copy.startWithWindows}</span>
      </label>
      <label className="toggleRow">
        <input type="checkbox" checked={settings.hideConsoleWindow} disabled={busy} onChange={(event) => onChange({ hideConsoleWindow: event.target.checked })} />
        <span>{copy.hideConsoleWindow}</span>
      </label>

      <label className="fieldLabel">
        {copy.language}
        <select value={settings.language} disabled={busy} onChange={(event) => onChange({ language: event.target.value as AppSettings['language'] })}>
          <option value="zh">中文</option>
          <option value="en">EN</option>
        </select>
      </label>

      <label className="fieldLabel">
        {copy.protocol}
        <select value={settings.aiProtocol} disabled={busy} onChange={(event) => onChange({ aiProtocol: event.target.value as AppSettings['aiProtocol'] })}>
          <option value="openai">{copy.openaiCompatible}</option>
          <option value="anthropic">{copy.anthropicCompatible}</option>
        </select>
      </label>

      <label className="fieldLabel">
        {copy.openaiBaseUrl}
        <input value={settings.openaiBaseUrl} disabled={busy} onChange={(event) => onChange({ openaiBaseUrl: event.target.value })} />
      </label>
      <label className="fieldLabel">
        {copy.anthropicBaseUrl}
        <input value={settings.anthropicBaseUrl} disabled={busy} onChange={(event) => onChange({ anthropicBaseUrl: event.target.value })} />
      </label>
      <label className="fieldLabel">
        {copy.apiKey}
        <span className="secretRow">
          <input type="password" value={settings.apiKey} disabled={busy} onChange={(event) => onChange({ apiKey: event.target.value })} />
          <button type="button" onClick={onPasteKey} disabled={busy}>{copy.paste}</button>
        </span>
      </label>
      <label className="fieldLabel">
        {copy.searchArchiveModel}
        <input list="mimo-models" value={settings.searchModel} disabled={busy} onChange={(event) => onChange({ searchModel: event.target.value })} />
      </label>
      <label className="fieldLabel">
        {copy.ocrModel}
        <input list="mimo-models" value={settings.ocrModel} disabled={busy} onChange={(event) => onChange({ ocrModel: event.target.value })} />
      </label>
      <div className="settingsActions">
        <button onClick={onSave} disabled={busy}><Save size={14} /> {copy.save}</button>
        <button onClick={onTest} disabled={busy}><TestTube2 size={14} /> {copy.test}</button>
        <button onClick={onRefreshModels} disabled={busy}><RefreshCw size={14} /> {copy.models}</button>
      </div>

      <label className="fieldLabel dataDirectoryField">
        {copy.dataDirectory}
        <div className="pathPickerRow">
          <input
            value={settings.dataDirectory}
            placeholder={settings.resolvedDataDirectory || copy.useDefaultPath}
            disabled={busy}
            onChange={(event) => onChange({ dataDirectory: event.target.value })}
          />
          <div className="pathPickerActions">
            <button type="button" onClick={onChooseDataDirectory} disabled={busy}>{copy.chooseFolder}</button>
            <button type="button" onClick={onResetDataDirectory} disabled={busy || !settings.dataDirectory}>{copy.useDefaultPath}</button>
          </div>
          <button type="button" className="pathApplyButton" onClick={onSave} disabled={busy || !dataDirectoryDirty}>{copy.dataDirectoryApply}</button>
        </div>
        <div className="fieldHelp">{copy.dataDirectoryHelp}</div>
        {dataDirectoryDirty ? <div className="fieldHelp dataDirectoryPending">{copy.dataDirectoryPending(dataDirectoryTarget)}</div> : null}
        <div className="fieldHelp">{copy.currentDataDirectory}: {settings.resolvedDataDirectory}</div>
      </label>
      {status ? <div className="settingsStatus">{status}</div> : null}
    </div>
  );
}

function ImagePreview({ item, copy, pasteOcrPending, onPasteOcr }: { item: ClipboardItem; copy: Copy; pasteOcrPending: boolean; onPasteOcr: (text: string) => void }) {
  const [dataSrc, setDataSrc] = useState('');
  const [imageFailed, setImageFailed] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const filePreviewSrc = fileSrc(item.imagePath);
  const src = dataSrc || filePreviewSrc;
  const ocrText = item.ocrText?.trim();

  useEffect(() => {
    let cancelled = false;
    setDataSrc('');
    setImageFailed(false);
    setExpanded(false);
    if (!item.imagePath) {
      setImageFailed(true);
      return () => {
        cancelled = true;
      };
    }
    void call<string>('get_image_data_url', { id: item.id })
      .then((nextSrc) => {
        if (!cancelled && nextSrc) setDataSrc(nextSrc);
      })
      .catch(() => {
        if (!cancelled && !filePreviewSrc) setImageFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [item.id, item.imagePath, filePreviewSrc]);

  return (
    <div className={`imageCard ${ocrText ? 'withOcr' : ''}`}>
      <div className="imagePreviewPane">
        <button
          className="imageThumbButton"
          onClick={(event) => {
            event.stopPropagation();
            if (src && !imageFailed) setExpanded((value) => !value);
          }}
          title={copy.fullPreview}
          type="button"
        >
          {src && !imageFailed ? <img src={src} alt={item.preview} onError={() => setImageFailed(true)} /> : <ImageIcon size={42} />}
        </button>
        <span>{copy.imageDimensions(item.width, item.height)}</span>
      </div>
      {ocrText ? (
        <>
          <div className="imageOcrDivider" aria-hidden="true" />
          <button className="ocrTextPane" onClick={(event) => { event.stopPropagation(); onPasteOcr(ocrText); }} disabled={pasteOcrPending} title={copy.pasteOcrText} type="button">
            <span>{copy.ocrText}</span>
            <p>{ocrText}</p>
          </button>
        </>
      ) : null}
      {expanded && src && !imageFailed ? (
        <button className="inlineImagePreview" onClick={(event) => { event.stopPropagation(); setExpanded(false); }} title={copy.fullPreview} type="button">
          <img src={src} alt={copy.fullPreview} />
        </button>
      ) : null}
    </div>
  );
}

function FolderDropTarget({ folder, items, copy, folderDeleting, isItemPastePending, isItemDeletePending, onMove, onPaste, onDeleteFolder, onDeleteItem }: { folder: Folder; items: ClipboardItem[]; copy: Copy; folderDeleting: boolean; isItemPastePending: (id: string) => boolean; isItemDeletePending: (id: string) => boolean; onMove: (itemId: string, folderId: string | null) => Promise<void>; onPaste: (item: ClipboardItem) => Promise<void>; onDeleteFolder: (folder: Folder) => Promise<void>; onDeleteItem: (id: string) => Promise<void>; }) {
  const [open, setOpen] = useState(false);
  const [moveValue, setMoveValue] = useState('');
  const [movePending, setMovePending] = useState(false);
  const folderItems = items.filter((item) => item.folderId === folder.id).slice(0, 5);
  return (
    <div className="folderBlock">
      <div className="folderHeaderRow">
        <button className="folderHeader" onClick={() => setOpen((value) => !value)} type="button">
          {open ? <FolderOpen size={15} /> : <FolderIcon size={15} />}
          <span>{folder.name}</span>
          <span className="countBadge">{folderItems.length}</span>
          {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>
        <button className="iconButton small danger folderDeleteButton" onClick={() => void onDeleteFolder(folder)} disabled={folderDeleting} title={copy.deleteFolder} type="button">
          <Trash2 size={14} />
        </button>
      </div>
      {open ? (
        <div className="folderBody">
          {folderItems.length === 0 ? <div className="emptyHint">{copy.noRecordsHere}</div> : null}
          {folderItems.map((item) => (
            <div key={item.id} className="folderRecordRow">
              <button className="sideItem" onClick={() => void onPaste(item)} disabled={isItemPastePending(item.id) || isItemDeletePending(item.id)}>{item.preview}</button>
              <button className="iconButton small danger folderItemDeleteButton" onClick={() => void onDeleteItem(item.id)} disabled={isItemDeletePending(item.id)} title={copy.deleteRecord} type="button">
                <Trash2 size={14} />
              </button>
            </div>
          ))}
          <select
            className="folderMoveSelect"
            value={moveValue}
            disabled={movePending}
            onChange={(event) => {
              const value = event.target.value;
              setMoveValue(value);
              if (!value) return;
              setMovePending(true);
              void onMove(value, folder.id).finally(() => {
                setMovePending(false);
                setMoveValue('');
              });
            }}
          >
            <option value="">{copy.moveSelected}</option>
            {items.map((item) => <option key={item.id} value={item.id}>{item.preview.slice(0, 42)}</option>)}
          </select>
        </div>
      ) : null}
    </div>
  );
}

function QuickPoolRow({ item, copy, pending, pastePending, onPaste, onStar, onUpdate, onDelete }: { item: QuickItem; copy: Copy; pending: boolean; pastePending: boolean; onPaste: (content: string) => Promise<void>; onStar: () => Promise<void>; onUpdate: (item: QuickItem) => void; onDelete: () => Promise<void> }) {
  const [content, setContent] = useState(item.content);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);

  async function saveEdit() {
    if (saving) return;
    setSaving(true);
    try {
      const ttl = item.isPinned ? 0 : Math.max(24 * 60 * 60, (item.expiresAt ?? 0) - Math.floor(Date.now() / 1000));
      const updated = await call<QuickItem>('update_quick_item', { id: item.id, content, ttl });
      onUpdate(updated);
      setEditing(false);
    } finally {
      setSaving(false);
    }
  }

  const rowBusy = pending || saving;

  return (
    <div className="quickRow">
      <button className="quickContentText pasteable" onClick={() => void onPaste(content)} disabled={pastePending || rowBusy}>{content}</button>
      {editing ? (
        <div className="quickEditBlock">
          <textarea value={content} onChange={(event) => setContent(event.target.value)} disabled={saving} />
          <div className="editorActions compact">
            <button onClick={() => void saveEdit()} disabled={saving}>{copy.save}</button>
            <button onClick={() => { setContent(item.content); setEditing(false); }} disabled={saving}>{copy.cancel}</button>
          </div>
        </div>
      ) : null}
      <div className="itemActions quickItemActions">
        <button className="quickActionButton" onClick={() => void onStar()} disabled={rowBusy} title={copy.collect}>
          <Star size={17} />
          <span>{copy.collect}</span>
        </button>
        <button className="quickActionButton" onClick={() => setEditing((value) => !value)} disabled={rowBusy} title={copy.edit}>
          <Edit3 size={17} />
          <span>{copy.edit}</span>
        </button>
        <button className="quickActionButton danger" onClick={() => void onDelete()} disabled={rowBusy} title={copy.delete}>
          <Trash2 size={17} />
          <span>{copy.delete}</span>
        </button>
      </div>
    </div>
  );
}

function formatTime(seconds: number) {
  return new Date(seconds * 1000).toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
}
