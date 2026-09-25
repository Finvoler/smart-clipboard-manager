/*
 * 前端主界面。
 *
 * 这是整个 React UI 的核心文件：侧边栏、历史列表、图片预览、编辑态、设置面板、
 * AI 搜索、AI 整理、临时池和多语言文案都集中在这里。
 */

import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type MouseEvent, type PointerEvent, type ReactNode } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Archive, Bot, Check, ChevronDown, ChevronLeft, ChevronRight, ChevronUp, CircleAlert, CircleCheck, Clock, CornerDownLeft, Edit3, Folder as FolderIcon, FolderOpen, FolderPlus, Image as ImageIcon, Info, LoaderCircle, Pin, Power, RefreshCw, Save, ScanText, Search, Settings, Star, TestTube2, Trash2, X } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import { open } from '@tauri-apps/plugin-dialog';
import { call, fileSrc, onNewItem, onPanelShown, onQuickSuggestionDetected, type AppSettings, type ClipboardItem, type DataDirectoryChangeResult, type Folder, type QuickItem, type QuickSuggestion } from './tauriClient';

const DEFAULT_LIMIT = 0;
const remarkPlugins = [remarkGfm, remarkMath];
const rehypePlugins = [rehypeKatex, rehypeHighlight];
const RecordMarkdown = memo(function RecordMarkdown({ text }: { text: string }) {
  return <ReactMarkdown remarkPlugins={remarkPlugins} rehypePlugins={rehypePlugins}>{text}</ReactMarkdown>;
});

function moveGlassHighlight(event: PointerEvent<HTMLElement>) {
  const button = (event.target as Element).closest('button');
  if (!button || !event.currentTarget.contains(button)) return;
  const bounds = button.getBoundingClientRect();
  button.style.setProperty('--glint-x', `${event.clientX - bounds.left}px`);
  button.style.setProperty('--glint-y', `${event.clientY - bounds.top}px`);
}

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
    expandRecord: '展开全部',
    collapseRecord: '收起',
    expandOcrText: '展开 OCR 文字',
    collapseOcrText: '收起 OCR 文字',
    closeNotice: '关闭通知',
    clipboardSidebar: '剪贴板侧边栏',
    searchPlaceholder: '搜索剪贴板',
    aiSearchPlaceholder: 'AI 搜索剪贴板',
    clearSearch: '清空搜索',
    runAiSearch: '运行 AI 搜索',
    semanticSearch: 'LLM 语义搜索',
    aiArchive: 'AI 整理',
    toggleSidebar: '折叠侧边栏',
    enterSearch: '先输入要查找的内容',
    aiSearching: 'AI 正在搜索全部历史，记录较多时可能需要较长时间...',
    aiFound: (count: number) => `AI 找到 ${count} 条相关记录`,
    aiOrganizing: 'AI 正在整理最近 300 条未归档记录，可能需要较长时间...',
    aiOrganized: (count: number) => `AI 已整理 ${count} 条记录`,
    settingsSaved: '已保存',
    actionFailed: '失败',
    actionWorking: '处理中',
    testPassed: '测试成功',
    chosen: '已选择',
    restored: '已选择默认',
    dataDirectory: '文件保存路径',
    currentDataDirectory: '当前实际数据目录',
    chooseFolder: '选择文件夹',
    useDefaultPath: '恢复默认',
    dataDirectoryApply: '保存路径并重启',
    dataDirectoryInvalid: '路径无效',
    dataDirectoryFailed: '路径保存失败',
    dataDirectoryHelp: '数据库、图片缓存和后续数据文件都会保存在这里。只能选择或输入已存在的文件夹；点击“保存路径并重启”后才会迁移并生效。',
    dataDirectoryConfirm: (target: string) => `确认把数据迁移到「${target}」？应用会自动重启。`,
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
    showModelOptions: '显示模型候选',
    hideModelOptions: '收起模型候选',
    localOcr: '图片 OCR',
    localOcrDescription: 'Windows 本地 OCR，无需 API Key；请安装所需语言的系统 OCR 语言包',
    test: '测试',
    models: '模型',
    imageDimensions: (width?: number | null, height?: number | null) => `${width ?? '?'} x ${height ?? '?'}`,
    fullPreview: '完整预览',
    stopPreview: '停止预览',
    ocrText: 'OCR 文本',
    pasteOcrText: '粘贴 OCR 文本',
    ocrReady: 'OCR 文本已写回图片记录',
    collect: '收藏',
    edit: '编辑',
  },
  en: {
    clipboardHistory: 'Clipboard History',
    expandRecord: 'Expand all',
    collapseRecord: 'Collapse',
    expandOcrText: 'Expand OCR text',
    collapseOcrText: 'Collapse OCR text',
    closeNotice: 'Dismiss notification',
    clipboardSidebar: 'Clipboard Sidebar',
    searchPlaceholder: 'Search clipboard',
    aiSearchPlaceholder: 'AI search clipboard',
    clearSearch: 'Clear search',
    runAiSearch: 'Run AI search',
    semanticSearch: 'LLM semantic search',
    aiArchive: 'AI archive',
    toggleSidebar: 'Toggle sidebar',
    enterSearch: 'Enter something to search',
    aiSearching: 'AI is searching all history; large libraries may take a while...',
    aiFound: (count: number) => `AI found ${count} matching records`,
    aiOrganizing: 'AI is organizing up to 300 recent uncategorized records; this may take a while...',
    aiOrganized: (count: number) => `AI organized ${count} records`,
    settingsSaved: 'Saved',
    actionFailed: 'Failed',
    actionWorking: 'Working',
    testPassed: 'Test passed',
    chosen: 'Selected',
    restored: 'Default selected',
    dataDirectory: 'Data directory',
    currentDataDirectory: 'Current active data directory',
    chooseFolder: 'Choose folder',
    useDefaultPath: 'Use default',
    dataDirectoryApply: 'Save path and restart',
    dataDirectoryInvalid: 'Invalid path',
    dataDirectoryFailed: 'Path save failed',
    dataDirectoryHelp: 'The database, image cache, and future data files will be stored here. Choose or enter an existing folder, then click Save path and restart to migrate and apply it.',
    dataDirectoryConfirm: (target: string) => `Move existing data to "${target}" and restart the app?`,
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
    showModelOptions: 'Show model options',
    hideModelOptions: 'Collapse model options',
    localOcr: 'Image OCR',
    localOcrDescription: 'Windows local OCR, no API key. Requires an installed Windows OCR language pack.',
    test: 'Test',
    models: 'Models',
    imageDimensions: (width?: number | null, height?: number | null) => `${width ?? '?'} x ${height ?? '?'}`,
    fullPreview: 'Full preview',
    stopPreview: 'Stop preview',
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
  const [allItems, setAllItems] = useState<ClipboardItem[]>([]);
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
  const [expandedIds, setExpandedIds] = useState<Set<string>>(() => new Set());
  const [pasteResetToken, setPasteResetToken] = useState(0);
  const [visiblyClippedIds, setVisiblyClippedIds] = useState<Set<string>>(() => new Set());
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [folderDialogOpen, setFolderDialogOpen] = useState(false);
  const [folderNameDraft, setFolderNameDraft] = useState('');
  const [folderDialogError, setFolderDialogError] = useState('');
  const [status, setStatus] = useState<StatusNotice | null>(null);
  const [actionFeedback, setActionFeedback] = useState<Record<string, { tone: 'success' | 'error'; label: string; detail?: string }>>({});
  const actionTimers = useRef<Record<string, ReturnType<typeof setTimeout>>>({});
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
  const scrollParentRef = useRef<HTMLDivElement | null>(null);
  const aiSearchModeRef = useRef(false);
  const queryRef = useRef('');
  const searchGeneration = useRef(0);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const language: Language = settings?.language === 'en' ? 'en' : 'zh';
  const copy = COPY[language];

  function flashAction(key: string, tone: 'success' | 'error', label: string, detail?: string) {
    clearTimeout(actionTimers.current[key]);
    setActionFeedback((current) => ({ ...current, [key]: { tone, label, detail } }));
    actionTimers.current[key] = setTimeout(() => setActionFeedback((current) => {
      const next = { ...current };
      delete next[key];
      return next;
    }), 3000);
  }

  useEffect(() => () => Object.values(actionTimers.current).forEach(clearTimeout), []);

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
    const generation = ++searchGeneration.current;
    const hasLocalFilter = Boolean(queryRef.current.trim() && !aiSearchModeRef.current);
    const [history, completeHistory, folderList, pool, suggestions, appSettings] = await Promise.all([
      hasLocalFilter
        ? call<ClipboardItem[]>('search_local_light', { keyword: queryRef.current })
        : call<ClipboardItem[]>('get_history_light', { limit: DEFAULT_LIMIT, offset: 0 }),
      hasLocalFilter ? call<ClipboardItem[]>('get_history_light', { limit: DEFAULT_LIMIT, offset: 0 }) : Promise.resolve(null),
      call<Folder[]>('get_folders'),
      call<QuickItem[]>('get_quick_pool'),
      call<QuickSuggestion[]>('get_quick_suggestions'),
      call<AppSettings>('get_app_settings'),
    ]);
    if (generation === searchGeneration.current) {
      setItems(history);
      setExpandedIds(new Set());
      setSelectedId((current) => current && history.some((item) => item.id === current) ? current : history[0]?.id ?? null);
    }
    setAllItems(completeHistory ?? history);
    setFolders(folderList);
    setQuickItems(pool);
    setQuickSuggestions(suggestions);
    setSettings(appSettings);
    setSavedSettings(appSettings);
  }

  useEffect(() => {
    void refresh().catch((error) => showStatus(String(error), 'error'));

    const cleanups: Array<() => void> = [];
    let disposed = false;
    const registerCleanup = (cleanup: () => void) => disposed ? cleanup() : cleanups.push(cleanup);
    void onNewItem((item) => {
      setAllItems((current) => [item, ...current.filter((candidate) => candidate.id !== item.id)]);
      if (aiSearchModeRef.current) return;
      if (queryRef.current.trim()) {
        void runLocalSearch(queryRef.current).catch((error) => showStatus(String(error), 'error'));
        return;
      }
      setItems((current) => {
        if (aiSearchModeRef.current) return current;
        return [item, ...current.filter((candidate) => candidate.id !== item.id)];
      });
      setSelectedId(item.id);
    }).then(registerCleanup).catch((error) => showStatus(String(error), 'error'));

    void onQuickSuggestionDetected((item) => {
      setQuickSuggestions((current) => [item, ...current.filter((candidate) => candidate.id !== item.id)]);
    }).then(registerCleanup).catch((error) => showStatus(String(error), 'error'));

    void onPanelShown(() => {
      void call<AppSettings>('get_app_settings').then((saved) => {
        setSettings(saved);
        setSavedSettings(saved);
        setActionFeedback({});
      }).catch((error) => showStatus(String(error), 'error'));
    }).then(registerCleanup).catch((error) => showStatus(String(error), 'error'));

    return () => {
      disposed = true;
      ++searchGeneration.current;
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
      cleanups.forEach((cleanup) => cleanup());
    };
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const isTyping = Boolean(target?.closest('input, textarea, select, [contenteditable="true"]'));
      if (event.key === 'Escape') {
        if (event.defaultPrevented) return;
        if (folderDialogOpen) {
          event.preventDefault();
          setFolderDialogOpen(false);
          return;
        }
        if (editingId !== null) {
          event.preventDefault();
          setEditingId(null);
          return;
        }
        event.preventDefault();
        void call('hide_window');
        return;
      }

      const isInteractive = Boolean(target?.closest('button, a, [role="button"]'));
      if (!event.defaultPrevented && !isTyping && !isInteractive && event.key === 'Enter' && selectedId && editingId === null) {
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
  }, [selectedId, editingId, folderDialogOpen, items]);

  useEffect(() => {
    if (!status || status.tone === 'loading') return;
    const timer = window.setTimeout(() => setStatus(null), status.tone === 'error' ? 8000 : 4500);
    return () => window.clearTimeout(timer);
  }, [status]);

  // SQLite searches full content; light rows only contain a shortened preview.
  // Filtering the preview again would discard valid matches later in a record.
  const filteredItems = items;
  const getItemKey = useMemo(() => (index: number) => items[index].id, [items]);

  const virtualizer = useVirtualizer({
    count: filteredItems.length,
    getItemKey,
    getScrollElement: () => scrollParentRef.current,
    estimateSize: () => 120,
    measureElement: (element) => element.getBoundingClientRect().height,
    overscan: 5,
  });

  // Measure before paint after content changes. ResizeObserver additionally
  // handles window resizing and images that finish loading later.
  useLayoutEffect(() => {
    scrollParentRef.current?.querySelectorAll<HTMLDivElement>('[data-index]').forEach((element) => {
      virtualizer.measureElement(element);
    });
    const measureClipping = () => {
      const clipped = new Set<string>();
      scrollParentRef.current?.querySelectorAll<HTMLElement>('.recordContent:not(.imageContent):not(.expanded)').forEach((element) => {
        const body = element.querySelector<HTMLElement>('.markdownButton');
        // The reveal state adds 28 px of bottom padding; exclude it when
        // measuring content after a resize.
        const revealPadding = element.classList.contains('isTruncated') ? 28 : 0;
        if (body && body.scrollHeight - revealPadding > 221) {
          const id = element.dataset.itemId;
          if (id) clipped.add(id);
        }
      });
      setVisiblyClippedIds((current) => {
        for (const id of expandedIds) {
          if (current.has(id)) clipped.add(id);
        }
        if (current.size === clipped.size && [...current].every((id) => clipped.has(id))) return current;
        return clipped;
      });
    };
    measureClipping();
    let frame = 0;
    const scheduleMeasurement = () => {
      if (!frame) frame = requestAnimationFrame(() => { frame = 0; measureClipping(); });
    };
    const list = scrollParentRef.current;
    list?.addEventListener('scroll', scheduleMeasurement, { passive: true });
    window.addEventListener('resize', scheduleMeasurement);
    return () => {
      list?.removeEventListener('scroll', scheduleMeasurement);
      window.removeEventListener('resize', scheduleMeasurement);
      if (frame) cancelAnimationFrame(frame);
    };
  }, [virtualizer, filteredItems, expandedIds, editingId, sidebarCollapsed]);

  async function toggleRecordExpanded(item: ClipboardItem) {
    if (!expandedIds.has(item.id) && item.kind === 'text' && item.content == null) {
      try {
        const full = await call<ClipboardItem>('get_item', { id: item.id });
        setItems((current) => current.map((entry) => entry.id === item.id ? { ...entry, content: full.content } : entry));
      } catch (error) {
        showStatus(String(error), 'error');
        return;
      }
    }
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(item.id)) next.delete(item.id);
      else next.add(item.id);
      return next;
    });
  }

  function showStatus(message: string, tone: NoticeTone = 'info') {
    setStatus({ message, tone });
  }

  function toggleSection(key: SectionKey) {
    setOpenSections((current) => ({ ...current, [key]: !current[key] }));
  }

  async function runLocalSearch(keyword: string) {
    const generation = ++searchGeneration.current;
    if (!keyword.trim()) {
      await refresh();
      return;
    }
    const result = await call<ClipboardItem[]>('search_local_light', { keyword });
    if (generation !== searchGeneration.current || aiSearchModeRef.current) return;
    setItems(result);
    setExpandedIds(new Set());
    setSelectedId(result[0]?.id ?? null);
  }

  function handleSearchChange(value: string) {
    setQuery(value);
    queryRef.current = value;
    ++searchGeneration.current;
    if (aiSearchMode) {
      setStatus(null);
      return;
    }
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    searchTimerRef.current = setTimeout(() => {
      void runLocalSearch(value).catch((error) => showStatus(String(error), 'error'));
    }, 250);
  }

  async function clearSearch() {
    await runWithPending('search:clear', async () => {
      setStatus(null);
      setQuery('');
      queryRef.current = '';
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
      await refresh();
      searchInputRef.current?.focus();
    });
  }

  function toggleAiSearchMode() {
    ++searchGeneration.current;
    if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    const next = !aiSearchModeRef.current;
    aiSearchModeRef.current = next;
    setAiSearchMode(next);
    if (!next) {
      setAiSearchRunning(false);
      setStatus(null);
      void runLocalSearch(queryRef.current).catch((error) => showStatus(String(error), 'error'));
    } else {
      window.setTimeout(() => searchInputRef.current?.focus(), 20);
    }
  }

  async function runAiSearch() {
    if (!query.trim()) {
      showStatus(copy.enterSearch, 'error');
      return;
    }
    await runWithPending('ai:search', async () => {
      const generation = ++searchGeneration.current;
      setAiSearchRunning(true);
      showStatus(copy.aiSearching, 'loading');
      try {
        const ids = await call<string[]>('search_ai_semantic', { query });
        const fullItems = await call<ClipboardItem[]>('get_items_by_ids', { ids });
        if (generation !== searchGeneration.current || !aiSearchModeRef.current) return;
        setItems(fullItems);
        setSelectedId(fullItems[0]?.id ?? null);
        showStatus(copy.aiFound(fullItems.length), 'success');
      } catch (error) {
        if (generation === searchGeneration.current && aiSearchModeRef.current) {
          showStatus(error instanceof Error ? error.message : String(error), 'error');
        }
      } finally {
        setAiSearchRunning(false);
      }
    });
  }

  function updateSettings(patch: Partial<AppSettings>) {
    if ('dataDirectory' in patch) {
      clearTimeout(actionTimers.current['settings:applyPath']);
      setActionFeedback((current) => {
        const next = { ...current };
        delete next['settings:applyPath'];
        return next;
      });
    }
    setSettings((current) => current ? { ...current, ...patch } : current);
  }

  async function saveToggle(key: 'captureEnabled' | 'runAtStartup' | 'hideConsoleWindow', value: boolean) {
    if (!settings || pendingKeysRef.current.has('settings:toggle')) return;
    const previous = settings[key];
    updateSettings({ [key]: value });
    await runWithPending('settings:toggle', async () => {
      try {
        const saved = await call<AppSettings>('save_app_settings', {
          settings: { ...(savedSettings ?? settings), [key]: value, appEnabled: true },
        });
        setSavedSettings(saved);
        setSettings((current) => current ? { ...current, [key]: saved[key] } : saved);
      } catch (error) {
        setSettings((current) => current ? { ...current, [key]: previous } : current);
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function persistSettings(nextSettings = settings) {
    if (!nextSettings) return null;
    const saved = await call<AppSettings>('save_app_settings', { settings: { ...nextSettings, appEnabled: true } });
    setSettings(saved);
    setSavedSettings(saved);
    return saved;
  }

  function hasPendingDataDirectoryChange(nextSettings: AppSettings) {
    return nextSettings.dataDirectory.trim() !== (savedSettings?.dataDirectory.trim() ?? '');
  }

  async function saveSettings(nextSettings = settings, actionKey: 'settings:save' | 'settings:applyPath' = 'settings:save') {
    const pendingDataDirectoryChange = !!nextSettings && hasPendingDataDirectoryChange(nextSettings);
    await runWithPending(actionKey, async () => {
      try {
        if (!nextSettings) return;
        if (pendingDataDirectoryChange) {
          // 数据目录切换不是普通设置保存：Rust 端会先写 bootstrap pending，再通过重启完成迁移。
          const targetLabel = nextSettings.dataDirectory.trim() || copy.useDefaultPath;
          const confirmed = window.confirm(copy.dataDirectoryConfirm(targetLabel));
          if (!confirmed) return;
          const result = await call<DataDirectoryChangeResult>('change_data_directory', {
            settings: { ...nextSettings, appEnabled: true },
          });
          setSettings(result.settings);
          setSavedSettings(result.settings);
          flashAction(actionKey, 'success', copy.settingsSaved);
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
        flashAction(actionKey, 'success', copy.settingsSaved);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        flashAction(actionKey, 'error', actionKey === 'settings:applyPath'
          ? (message.startsWith('Invalid data directory:') ? copy.dataDirectoryInvalid : copy.dataDirectoryFailed)
          : copy.actionFailed, message);
      }
    });
  }

  async function testAiConnection() {
    await runWithPending('settings:test', async () => {
      try {
        if (!settings) return;
        // Test commits only the API fields being tested. Other draft settings,
        // especially a pending data-directory migration, stay untouched.
        const saved = await call<AppSettings>('save_app_settings', { settings: {
          ...(savedSettings ?? settings),
          aiProtocol: settings.aiProtocol,
          openaiBaseUrl: settings.openaiBaseUrl,
          anthropicBaseUrl: settings.anthropicBaseUrl,
          apiKey: settings.apiKey,
          searchModel: settings.searchModel,
          appEnabled: true,
        } });
        setSavedSettings(saved);
        setSettings((current) => current ? {
          ...current,
          aiProtocol: saved.aiProtocol,
          openaiBaseUrl: saved.openaiBaseUrl,
          anthropicBaseUrl: saved.anthropicBaseUrl,
          apiKey: saved.apiKey,
          searchModel: saved.searchModel,
        } : saved);
        await call<string>('test_ai_connection', { settings: saved });
        flashAction('settings:test', 'success', copy.testPassed);
      } catch (error) {
        flashAction('settings:test', 'error', copy.actionFailed, error instanceof Error ? error.message : String(error));
      }
    });
  }

  async function refreshModelOptions() {
    await runWithPending('settings:models', async () => {
      try {
        if (!settings) return;
        const models = await call<string[]>('list_ai_models', { settings });
        setModelOptions(models);
        flashAction('settings:models', 'success', copy.loadedModels(models.length));
      } catch (error) {
        flashAction('settings:models', 'error', copy.actionFailed, error instanceof Error ? error.message : String(error));
      }
    });
  }

  async function pasteApiKeyFromClipboard() {
    await runWithPending('settings:pasteKey', async () => {
      try {
        const apiKey = (await navigator.clipboard.readText()).trim();
        if (!apiKey) {
          flashAction('settings:pasteKey', 'error', copy.actionFailed, copy.clipboardEmpty);
          return;
        }
        updateSettings({ apiKey });
        flashAction('settings:pasteKey', 'success', copy.chosen);
      } catch (error) {
        flashAction('settings:pasteKey', 'error', copy.actionFailed, error instanceof Error ? error.message : String(error));
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
          flashAction('settings:path', 'success', copy.chosen);
        }
      } catch (error) {
        flashAction('settings:path', 'error', copy.actionFailed, error instanceof Error ? error.message : String(error));
      }
    });
  }

  function resetDataDirectory() {
    updateSettings({ dataDirectory: '' });
    flashAction('settings:resetPath', 'success', copy.restored);
  }

  async function executePaste(id: string, overrideText?: string, pendingKey = `paste:${id || overrideText || 'override'}`) {
    await runWithPending(pendingKey, async () => {
      try {
        await call('execute_paste', { itemId: id, overrideText });
        setExpandedIds(new Set());
        setPasteResetToken((current) => current + 1);
        setEditingId(null);
        scrollParentRef.current?.scrollTo(0, 0);
        virtualizer.scrollToOffset(0);
        setSelectedId(items[0]?.id ?? null);
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function pasteClipboardItem(item: ClipboardItem) {
    if (item.kind === 'text' && item.content) {
      await executePaste('', item.content, `paste:item:${item.id}`);
    } else {
      await executePaste(item.id, undefined, `paste:item:${item.id}`);
    }
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
        setAllItems((current) => [starred, ...current.filter((item) => item.id !== starred.id)]);
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
        setAllItems((current) => current.map((item) => (item.id === id ? updated : item)));
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
        setAllItems((current) => current.map((candidate) => (candidate.id === item.id ? updated : candidate)));
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
        setAllItems((current) => current.filter((item) => item.id !== id));
        setSelectedId((current) => current === id ? items.find((item) => item.id !== id)?.id ?? null : current);
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
        setAllItems((current) => current.map((item) => item.folderId === folder.id ? { ...item, folderId: null } : item));
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
        setAllItems((current) => current.map((item) => (item.id === itemId ? updated : item)));
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  async function createFolder(name: string) {
    if (!name.trim()) return;
    await runWithPending('folder:create', async () => {
      try {
        const folder = await call<Folder>('create_folder', { name: name.trim() });
        setFolders((current) => [...current, folder]);
        setFolderDialogOpen(false);
        setFolderNameDraft('');
        setFolderDialogError('');
      } catch (error) {
        setFolderDialogError(error instanceof Error ? error.message : String(error));
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
        setAllItems((current) => current.map((candidate) => candidate.id === updated.id ? updated : candidate));
        setSelectedId(updated.id);
        showStatus(copy.ocrReady, 'success');
      } catch (error) {
        showStatus(error instanceof Error ? error.message : String(error), 'error');
      }
    });
  }

  const aiSearchPending = aiSearchRunning || isPending('ai:search');
  const categorizePending = categorizeRunning || isPending('ai:categorize');
  const settingsBusy = ['settings:save', 'settings:applyPath', 'settings:test', 'settings:models', 'settings:pasteKey', 'settings:path', 'settings:toggle'].some(isPending);

  return (
    <main className={`shell ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`} onPointerMove={moveGlassHighlight}>
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

        {status ? <StatusLine notice={status} closeLabel={copy.closeNotice} onClose={() => setStatus(null)} /> : null}

        <div className="historyList" ref={scrollParentRef}>
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const item = filteredItems[virtualRow.index];
              const canExpandText = item.kind === 'text' &&
                ((item.preview.endsWith('...') && [...item.preview].length === 183) || visiblyClippedIds.has(item.id));
              return (
                <div
                  key={item.id}
                  className="historyRow"
                  ref={virtualizer.measureElement}
                  data-index={virtualRow.index}
                  style={{ position: 'absolute', top: 0, left: 0, width: '100%', transform: `translateY(${virtualRow.start}px)` }}
                >
                  <article
                    className={`historyItem ${selectedId === item.id ? 'selected' : ''}`}
                    onMouseEnter={() => setSelectedId(item.id)}
                    onClick={(event) => pasteFromRecord(event, item)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter' && event.target === event.currentTarget && editingId !== item.id) {
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
                            onClick={(event) => stopAndRun(event, () => void runWithPending(`edit:${item.id}`, async () => {
                              try {
                                let content = item.content;
                                if (!content) {
                                  const full = await call<ClipboardItem>('get_item', { id: item.id });
                                  content = full.content ?? '';
                                }
                                setEditingId(item.id);
                                setEditingText(content);
                              } catch (error) {
                                showStatus(error instanceof Error ? error.message : String(error), 'error');
                              }
                            }))}
                            title={copy.editText}
                          >
                            <Edit3 size={15} />
                          </button>
                        ) : (
                          <button className={`iconButton small ${isPending(`ocr:${item.id}`) ? 'active loading' : ''}`} onClick={(event) => stopAndRun(event, () => void runOcr(item))} disabled={isPending(`ocr:${item.id}`) || isPending(`delete:${item.id}`)} title={copy.ocrImage} aria-busy={isPending(`ocr:${item.id}`)}>
                            <ScanText size={15} />
                          </button>
                        )}
                        <button className="iconButton small danger" onClick={(event) => stopAndRun(event, () => void removeItem(item.id))} disabled={isPending(`delete:${item.id}`)} title={copy.delete}>
                          <Trash2 size={15} />
                        </button>
                      </div>
                    </div>

                    <div id={`record-content-${item.id}`} data-item-id={item.id} className={`recordContent ${item.kind === 'image' ? 'imageContent' : ''} ${expandedIds.has(item.id) || editingId === item.id ? 'expanded' : ''} ${canExpandText && !expandedIds.has(item.id) && editingId !== item.id ? 'isTruncated' : ''}`}>
                    {editingId === item.id ? (
                      <div className="editorBlock">
                        <textarea value={editingText} disabled={isPending(`edit:${item.id}`)} onChange={(event) => setEditingText(event.target.value)} />
                        <div className="editorActions">
                          <button onClick={() => saveEdit(item.id)} disabled={isPending(`edit:${item.id}`)}>{copy.save}</button>
                          <button onClick={() => setEditingId(null)} disabled={isPending(`edit:${item.id}`)}>{copy.cancel}</button>
                        </div>
                      </div>
                    ) : item.kind === 'image' ? (
                      <ImagePreview item={item} copy={copy} pasteOcrPending={isPending(`paste:ocr:${item.id}`)} ocrExpanded={expandedIds.has(item.id)} pasteResetToken={pasteResetToken} onToggleOcr={() => void runWithPending(`expand:${item.id}`, () => toggleRecordExpanded(item))} onPasteOcr={(text) => void executePaste('', text, `paste:ocr:${item.id}`)} />
                    ) : (
                      <div className="markdownButton" role="button" tabIndex={-1}>
                        {canExpandText && !expandedIds.has(item.id) ? <p className="recordPreviewText">{item.preview}</p> : <RecordMarkdown text={item.content ?? item.preview} />}
                      </div>
                    )}
                    {canExpandText && !expandedIds.has(item.id) && editingId !== item.id ? (
                      <button className="recordRevealButton" type="button" aria-label={copy.expandRecord} title={copy.expandRecord} aria-expanded={false} aria-controls={`record-content-${item.id}`} disabled={isPending(`expand:${item.id}`)} onClick={(event) => stopAndRun(event, () => void runWithPending(`expand:${item.id}`, () => toggleRecordExpanded(item)))}>
                        <span aria-hidden="true">···</span>
                      </button>
                    ) : null}
                    </div>
                    {canExpandText && expandedIds.has(item.id) && editingId !== item.id ? (
                      <button className="recordCollapseButton" type="button" aria-expanded={true} aria-controls={`record-content-${item.id}`} onClick={(event) => stopAndRun(event, () => void toggleRecordExpanded(item))}><ChevronUp size={14} aria-hidden="true" /><span>{copy.collapseRecord}</span></button>
                    ) : null}
                  </article>
                </div>
              );
            })}
          </div>
        </div>
      </section>

      <aside className="sidebar" aria-label={copy.clipboardSidebar}>
        <button className="collapseButton" onClick={() => setSidebarCollapsed((value) => !value)} title={copy.toggleSidebar}>
          {sidebarCollapsed ? <ChevronLeft size={17} /> : <ChevronRight size={17} />}
        </button>

        <SidebarSection title={copy.starred} icon={<Star size={15} />} open={openSections.starred} onToggle={() => toggleSection('starred')}>
          {allItems.filter((item) => item.isStar).length === 0 ? <div className="emptyHint">{copy.noStarred}</div> : null}
          {allItems.filter((item) => item.isStar).map((item) => (
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
                onError={(error) => showStatus(error instanceof Error ? error.message : String(error), 'error')}
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
          actions={<button className="iconButton small" onClick={(event) => stopAndRun(event, () => { setFolderNameDraft(''); setFolderDialogError(''); setFolderDialogOpen(true); })} disabled={isPending('folder:create')} title={copy.createFolder}><FolderPlus size={14} /></button>}
        >
          {folders.length === 0 ? <div className="emptyHint">{copy.noFoldersYet}</div> : null}
          {folders.map((folder) => (
            <FolderDropTarget
              key={folder.id}
              folder={folder}
              items={allItems}
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
              feedback={actionFeedback}
              pendingKeys={pendingKeys}
              modelOptions={modelOptions}
              copy={copy}
              busy={settingsBusy}
              dataDirectoryDirty={hasPendingDataDirectoryChange(settings)}
              onChange={updateSettings}
              onToggle={(key, value) => void saveToggle(key, value)}
              onSave={() => void saveSettings()}
              onApplyPath={() => void saveSettings(settings, 'settings:applyPath')}
              onTest={() => void testAiConnection()}
              onRefreshModels={() => void refreshModelOptions()}
              onPasteKey={() => void pasteApiKeyFromClipboard()}
              onChooseDataDirectory={() => void chooseDataDirectory()}
              onResetDataDirectory={resetDataDirectory}
            />
          ) : <div className="emptyHint">{copy.settingsLoading}</div>}
        </SidebarSection>
      </aside>
      {folderDialogOpen ? <div className="dialogBackdrop" onMouseDown={(event) => { if (event.target === event.currentTarget && !isPending('folder:create')) setFolderDialogOpen(false); }}>
        <form className="glassDialog" role="dialog" aria-modal="true" aria-labelledby="folder-dialog-title" onSubmit={(event) => { event.preventDefault(); void createFolder(folderNameDraft); }} onKeyDown={(event) => { if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); if (!isPending('folder:create')) setFolderDialogOpen(false); } }}>
          <div className="glassDialogHeader"><span className="glassDialogIcon"><FolderPlus size={18} /></span><h2 id="folder-dialog-title">{copy.createFolder}</h2><button className="dialogClose" type="button" title={copy.cancel} aria-label={copy.cancel} disabled={isPending('folder:create')} onClick={() => setFolderDialogOpen(false)}><X size={16} /></button></div>
          <label className="dialogField">{copy.folderNamePrompt}<input autoFocus value={folderNameDraft} onChange={(event) => { setFolderNameDraft(event.target.value); setFolderDialogError(''); }} maxLength={80} placeholder={copy.folderNamePrompt} disabled={isPending('folder:create')} /></label>
          {folderDialogError ? <div className="dialogError" role="alert">{folderDialogError}</div> : null}
          <div className="glassDialogActions"><button type="button" disabled={isPending('folder:create')} onClick={() => setFolderDialogOpen(false)}>{copy.cancel}</button><button className="dialogSubmit" type="submit" disabled={isPending('folder:create') || !folderNameDraft.trim()}>{isPending('folder:create') ? <span className="buttonSpinner" aria-hidden="true" /> : <FolderPlus size={15} />}<span>{isPending('folder:create') ? copy.actionWorking : copy.createFolder}</span></button></div>
        </form>
      </div> : null}
    </main>
  );
}

function StatusLine({ notice, closeLabel, onClose }: { notice: StatusNotice; closeLabel: string; onClose: () => void }) {
  return (
    <div className={`statusLine ${notice.tone}`} role={notice.tone === 'error' ? 'alert' : 'status'}>
      <span className="statusIcon" aria-hidden="true">{notice.tone === 'loading' ? <LoaderCircle size={17} /> : notice.tone === 'success' ? <CircleCheck size={17} /> : notice.tone === 'error' ? <CircleAlert size={17} /> : <Info size={17} />}</span>
      <span className="statusMessage">{notice.message}</span>
      <button className="statusClose" onClick={onClose} title={closeLabel} aria-label={closeLabel}><X size={15} /></button>
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

function GlassSelect({ value, options, onChange, disabled, label, className = '' }: { value: string; options: { value: string; label: string }[]; onChange: (value: string) => void; disabled?: boolean; label: string; className?: string }) {
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const currentIndex = Math.max(0, options.findIndex((option) => option.value === value));
  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: globalThis.PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', closeOutside);
    return () => document.removeEventListener('pointerdown', closeOutside);
  }, [open]);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  const select = (index: number) => {
    const option = options[index];
    if (option) onChange(option.value);
    setOpen(false);
    rootRef.current?.querySelector<HTMLButtonElement>('.glassSelectTrigger')?.focus();
  };
  return (
    <div className={`glassSelect ${className} ${open ? 'isOpen' : ''}`} ref={rootRef}>
      <button className="glassSelectTrigger" type="button" role="combobox" aria-label={label} aria-expanded={open} aria-haspopup="listbox" disabled={disabled} title={options[currentIndex]?.label ?? label} onClick={() => { setActiveIndex(currentIndex); setOpen((current) => !current); }} onKeyDown={(event) => {
        if (event.key === 'Escape' && open) { event.preventDefault(); event.stopPropagation(); setOpen(false); return; }
        if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
          event.preventDefault();
          const next = open ? (activeIndex + (event.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length : currentIndex;
          setActiveIndex(next);
          setOpen(true);
        }
        if (event.key === 'Enter' && open) { event.preventDefault(); select(activeIndex); }
      }}>
        <span className="glassSelectValue">{options[currentIndex]?.label ?? label}</span><ChevronDown size={15} aria-hidden="true" />
      </button>
      {open ? <div className="glassSelectMenu" role="listbox" aria-label={label}>
        {options.map((option, index) => <button key={`${option.value}-${index}`} className={`glassSelectOption ${index === activeIndex ? 'isActive' : ''}`} type="button" role="option" aria-selected={value === option.value} title={option.label} onMouseEnter={() => setActiveIndex(index)} onClick={() => select(index)}><span>{option.label}</span>{value === option.value ? <Check size={14} aria-hidden="true" /> : null}</button>)}
      </div> : null}
    </div>
  );
}

function ModelInput({ value, options, onChange, disabled, label, showOptionsLabel, hideOptionsLabel }: { value: string; options: string[]; onChange: (value: string) => void; disabled: boolean; label: string; showOptionsLabel: string; hideOptionsLabel: string }) {
  const [open, setOpen] = useState(false);
  const [filtering, setFiltering] = useState(false);
  const [activeIndex, setActiveIndex] = useState(0);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const matches = filtering ? options.filter((option) => option.toLowerCase().includes(value.trim().toLowerCase())) : options;
  useEffect(() => {
    if (!open) return;
    const closeOutside = (event: globalThis.PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', closeOutside);
    return () => document.removeEventListener('pointerdown', closeOutside);
  }, [open]);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);
  return <div className="modelInput" ref={rootRef}>
    <input value={value} disabled={disabled} role="combobox" aria-label={label} aria-autocomplete="list" aria-expanded={open && matches.length > 0} onFocus={() => { setActiveIndex(0); setFiltering(false); setOpen(true); }} onChange={(event) => { onChange(event.target.value); setActiveIndex(0); setFiltering(true); setOpen(true); }} onKeyDown={(event) => {
      if (event.key === 'Escape' && open) { event.preventDefault(); event.stopPropagation(); setOpen(false); return; }
      if (!matches.length) return;
      if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
        event.preventDefault();
        setActiveIndex((current) => (current + (event.key === 'ArrowDown' ? 1 : -1) + matches.length) % matches.length);
        setOpen(true);
      }
      if (event.key === 'Enter' && open) { event.preventDefault(); onChange(matches[activeIndex] ?? matches[0]); setFiltering(false); setOpen(false); }
    }} />
    <button className={`modelToggle ${open ? 'isOpen' : ''}`} type="button" disabled={disabled} aria-label={open ? hideOptionsLabel : showOptionsLabel} title={open ? hideOptionsLabel : showOptionsLabel} aria-expanded={open} onPointerDown={(event) => event.preventDefault()} onClick={(event) => { event.preventDefault(); event.stopPropagation(); setActiveIndex(0); setFiltering(false); setOpen((current) => !current); }}>{open ? <ChevronUp size={14} /> : <ChevronDown size={14} />}</button>
    {open && matches.length > 0 ? <div className="glassSelectMenu modelSuggestions" role="listbox" aria-label={label}>
      {matches.map((option, index) => <button className={`glassSelectOption ${index === activeIndex ? 'isActive' : ''}`} key={option} type="button" role="option" aria-selected={option === value} title={option} onMouseEnter={() => setActiveIndex(index)} onClick={() => { onChange(option); setFiltering(false); setOpen(false); }}><span>{option}</span>{option === value ? <Check size={14} aria-hidden="true" /> : null}</button>)}
    </div> : null}
  </div>;
}

function QuickSuggestionRow({ item, copy, pending, onAccept, onDismiss }: { item: QuickSuggestion; copy: Copy; pending: boolean; onAccept: (ttl: number) => Promise<void>; onDismiss: () => Promise<void>; }) {
  const [ttl, setTtl] = useState(String(24 * 60 * 60));

  return (
    <div className="suggestionRow">
      <div className="quickContentText">{item.content}</div>
      <div className="suggestionMeta">{copy.repeatedMeta(item.hitCount)}</div>
      <div className="quickRetentionLabel"><GlassSelect label={copy.retentionLabel} value={ttl} onChange={setTtl} disabled={pending} options={[{ value: String(24 * 60 * 60), label: copy.hours24 }, { value: String(3 * 24 * 60 * 60), label: copy.days3 }, { value: String(7 * 24 * 60 * 60), label: copy.days7 }]} /></div>
      <div className="suggestionActions">
        <button onClick={() => void onAccept(Number(ttl))} disabled={pending}><Check size={14} /> {copy.accept}</button>
        <button className="dangerText" onClick={() => void onDismiss()} disabled={pending}><Trash2 size={14} /> {copy.reject}</button>
      </div>
    </div>
  );
}

function FeedbackButton({ actionKey, feedback, pending, disabled, label, workingLabel, icon, className = '', onClick }: { actionKey: string; feedback: Record<string, { tone: 'success' | 'error'; label: string; detail?: string }>; pending: boolean; disabled: boolean; label: string; workingLabel: string; icon?: ReactNode; className?: string; onClick: () => void }) {
  const result = feedback[actionKey];
  return <button className={`feedbackButton ${className} ${pending ? 'isBusy' : ''} ${result ? `is${result.tone === 'success' ? 'Success' : 'Error'}` : ''}`} type="button" onClick={onClick} disabled={disabled} aria-live="polite" title={result?.detail ?? result?.label ?? label}>
    {pending ? <span className="buttonSpinner" aria-hidden="true" /> : result ? (result.tone === 'success' ? <Check size={14} aria-hidden="true" /> : <X size={14} aria-hidden="true" />) : icon}
    <span className="feedbackLabel">{pending ? workingLabel : result?.label ?? label}</span>
  </button>;
}

function SettingsFields({ settings, feedback, pendingKeys, modelOptions, copy, busy, dataDirectoryDirty, onChange, onToggle, onSave, onApplyPath, onTest, onRefreshModels, onPasteKey, onChooseDataDirectory, onResetDataDirectory }: { settings: AppSettings; feedback: Record<string, { tone: 'success' | 'error'; label: string; detail?: string }>; pendingKeys: Set<string>; modelOptions: string[]; copy: Copy; busy: boolean; dataDirectoryDirty: boolean; onChange: (patch: Partial<AppSettings>) => void; onToggle: (key: 'captureEnabled' | 'runAtStartup' | 'hideConsoleWindow', value: boolean) => void; onSave: () => void; onApplyPath: () => void; onTest: () => void; onRefreshModels: () => void; onPasteKey: () => void; onChooseDataDirectory: () => void; onResetDataDirectory: () => void; }) {
  return (
    <div className="settingsFields">
      <label className="toggleRow">
        <input type="checkbox" checked={settings.captureEnabled} disabled={busy} onChange={(event) => onToggle('captureEnabled', event.target.checked)} />
        <span><Power size={14} /> {copy.captureClipboard}</span>
      </label>

      <label className="toggleRow">
        <input type="checkbox" checked={settings.runAtStartup} disabled={busy} onChange={(event) => onToggle('runAtStartup', event.target.checked)} />
        <span>{copy.startWithWindows}</span>
      </label>
      <label className="toggleRow">
        <input type="checkbox" checked={settings.hideConsoleWindow} disabled={busy} onChange={(event) => onToggle('hideConsoleWindow', event.target.checked)} />
        <span>{copy.hideConsoleWindow}</span>
      </label>

      <div className="fieldLabel">
        {copy.language}
        <GlassSelect label={copy.language} value={settings.language} disabled={busy} onChange={(value) => onChange({ language: value as AppSettings['language'] })} options={[{ value: 'zh', label: '中文' }, { value: 'en', label: 'EN' }]} />
      </div>

      <div className="fieldLabel">
        {copy.protocol}
        <GlassSelect label={copy.protocol} value={settings.aiProtocol} disabled={busy} onChange={(value) => onChange({ aiProtocol: value as AppSettings['aiProtocol'] })} options={[{ value: 'openai', label: copy.openaiCompatible }, { value: 'anthropic', label: copy.anthropicCompatible }]} />
      </div>

      <div className="fieldLabel">
        {copy.openaiBaseUrl}
        <input aria-label={copy.openaiBaseUrl} value={settings.openaiBaseUrl} disabled={busy} onChange={(event) => onChange({ openaiBaseUrl: event.target.value })} />
      </div>
      <div className="fieldLabel">
        {copy.anthropicBaseUrl}
        <input aria-label={copy.anthropicBaseUrl} value={settings.anthropicBaseUrl} disabled={busy} onChange={(event) => onChange({ anthropicBaseUrl: event.target.value })} />
      </div>
      <div className="fieldLabel">
        {copy.apiKey}
        <span className="secretRow">
          <input aria-label={copy.apiKey} type="password" value={settings.apiKey} disabled={busy} onChange={(event) => onChange({ apiKey: event.target.value })} />
          <FeedbackButton actionKey="settings:pasteKey" feedback={feedback} pending={pendingKeys.has('settings:pasteKey')} disabled={busy} label={copy.paste} workingLabel={copy.actionWorking} onClick={onPasteKey} />
        </span>
      </div>
      <div className="fieldLabel">
        {copy.searchArchiveModel}
        <ModelInput label={copy.searchArchiveModel} showOptionsLabel={copy.showModelOptions} hideOptionsLabel={copy.hideModelOptions} value={settings.searchModel} options={modelOptions} disabled={busy} onChange={(value) => onChange({ searchModel: value })} />
      </div>
      <div className="fieldLabel">
        {copy.localOcr}
        <span>{copy.localOcrDescription}</span>
      </div>
      <div className="settingsActions">
        <FeedbackButton actionKey="settings:save" feedback={feedback} pending={pendingKeys.has('settings:save')} disabled={busy} label={copy.save} workingLabel={copy.actionWorking} icon={<Save size={14} />} onClick={onSave} />
        <FeedbackButton actionKey="settings:test" feedback={feedback} pending={pendingKeys.has('settings:test')} disabled={busy} label={copy.test} workingLabel={copy.actionWorking} icon={<TestTube2 size={14} />} onClick={onTest} />
        <FeedbackButton actionKey="settings:models" feedback={feedback} pending={pendingKeys.has('settings:models')} disabled={busy} label={copy.models} workingLabel={copy.actionWorking} icon={<RefreshCw size={14} />} onClick={onRefreshModels} />
      </div>

      <div className="fieldLabel dataDirectoryField">
        {copy.dataDirectory}
        <div className="pathPickerRow">
          <input
            aria-label={copy.dataDirectory}
            value={settings.dataDirectory}
            placeholder={settings.resolvedDataDirectory || copy.useDefaultPath}
            disabled={busy}
            onChange={(event) => onChange({ dataDirectory: event.target.value })}
          />
          <div className="pathPickerActions">
            <FeedbackButton actionKey="settings:path" feedback={feedback} pending={pendingKeys.has('settings:path')} disabled={busy} label={copy.chooseFolder} workingLabel={copy.actionWorking} onClick={onChooseDataDirectory} />
            <FeedbackButton actionKey="settings:resetPath" feedback={feedback} pending={false} disabled={busy || !settings.dataDirectory} label={copy.useDefaultPath} workingLabel={copy.actionWorking} onClick={onResetDataDirectory} />
          </div>
          <FeedbackButton actionKey="settings:applyPath" className="pathApplyButton" feedback={feedback} pending={pendingKeys.has('settings:applyPath')} disabled={busy || !dataDirectoryDirty} label={copy.dataDirectoryApply} workingLabel={copy.actionWorking} onClick={onApplyPath} />
        </div>
        <div className="fieldHelp">{copy.dataDirectoryHelp}</div>
        <div className="fieldHelp">{copy.currentDataDirectory}: {settings.resolvedDataDirectory}</div>
      </div>
    </div>
  );
}

function ImagePreview({ item, copy, pasteOcrPending, ocrExpanded, pasteResetToken, onToggleOcr, onPasteOcr }: { item: ClipboardItem; copy: Copy; pasteOcrPending: boolean; ocrExpanded: boolean; pasteResetToken: number; onToggleOcr: () => void; onPasteOcr: (text: string) => void }) {
  const [dataSrc, setDataSrc] = useState('');
  const [imageFailed, setImageFailed] = useState(false);
  const [expanded, setExpanded] = useState(false);
  useEffect(() => setExpanded(false), [pasteResetToken]);
  const [ocrOverflow, setOcrOverflow] = useState(false);
  const ocrPaneRef = useRef<HTMLDivElement | null>(null);
  const ocrTextRef = useRef<HTMLParagraphElement | null>(null);
  const filePreviewSrc = fileSrc(item.imagePath);
  const src = dataSrc || filePreviewSrc;
  const ocrText = item.ocrText?.trim();

  useLayoutEffect(() => {
    if (!ocrText || ocrExpanded) return;
    const measure = () => {
      const text = ocrTextRef.current;
      const pane = ocrPaneRef.current;
      setOcrOverflow(Boolean(text && pane && (text.scrollHeight > text.clientHeight + 1 || pane.scrollHeight > pane.clientHeight + 1)));
    };
    measure();
    const observer = new ResizeObserver(measure);
    if (ocrPaneRef.current) observer.observe(ocrPaneRef.current);
    return () => observer.disconnect();
  }, [ocrText, ocrExpanded]);

  useEffect(() => {
    let cancelled = false;
    setDataSrc('');
    setImageFailed(false);
    setExpanded(false);
    if (!item.imagePath) {
      setImageFailed(true);
      return () => { cancelled = true; };
    }
    // If fileSrc already gives us a URL, try loading it first via <img onError>
    // If that fails, fall back to base64 data URL via IPC
    if (filePreviewSrc) {
      return () => { cancelled = true; };
    }
    void call<string>('get_image_data_url', { id: item.id })
      .then((nextSrc) => {
        if (!cancelled && nextSrc) setDataSrc(nextSrc);
      })
      .catch(() => {
        if (!cancelled) setImageFailed(true);
      });
    return () => { cancelled = true; };
  }, [item.id, item.imagePath]);

  return (
    <div className={`imageCard ${ocrText ? 'withOcr' : ''} ${ocrExpanded ? 'ocrExpanded' : ''}`}>
      <div className="imagePreviewPane">
        <button
          className="imageThumbButton"
          onClick={(event) => {
            event.stopPropagation();
            if (src && !imageFailed) setExpanded((value) => !value);
          }}
          title={expanded ? copy.stopPreview : copy.fullPreview}
          type="button"
        >
          {src && !imageFailed ? <img src={src} alt={item.preview} onError={() => {
            // If fileSrc URL failed, try base64 fallback
            if (!dataSrc && item.imagePath) {
              call<string>('get_image_data_url', { id: item.id })
                .then((nextSrc) => { if (nextSrc) setDataSrc(nextSrc); })
                .catch(() => setImageFailed(true));
            } else {
              setImageFailed(true);
            }
          }} /> : <ImageIcon size={42} />}
        </button>
        <span>{copy.imageDimensions(item.width, item.height)}</span>
        {ocrText && ocrExpanded ? (
          <button className="ocrCollapseButton" type="button" aria-label={copy.collapseOcrText} title={copy.collapseOcrText} aria-expanded={true} onClick={(event) => { event.stopPropagation(); onToggleOcr(); }}><ScanText size={13} aria-hidden="true" /><ChevronUp size={13} aria-hidden="true" /></button>
        ) : null}
      </div>
      {expanded && src && !imageFailed ? (
        <button className="inlineImagePreview" onClick={(event) => { event.stopPropagation(); setExpanded(false); }} title={copy.stopPreview} type="button">
          <img src={src} alt={copy.fullPreview} />
        </button>
      ) : null}
      {ocrText ? (
        <>
          <div className="imageOcrDivider" aria-hidden="true" />
          <div className={`ocrPaneWrap ${ocrOverflow && !ocrExpanded ? 'isTruncated' : ''}`} ref={ocrPaneRef}>
            <button className="ocrTextPane" onClick={(event) => { event.stopPropagation(); onPasteOcr(ocrText); }} disabled={pasteOcrPending} title={copy.pasteOcrText} type="button">
              <span>{copy.ocrText}</span>
              <p ref={ocrTextRef}>{ocrText}</p>
            </button>
            {ocrOverflow && !ocrExpanded ? (
              <button className="ocrRevealButton" type="button" aria-label={copy.expandOcrText} title={copy.expandOcrText} aria-expanded={false} onClick={(event) => { event.stopPropagation(); onToggleOcr(); }}><ScanText size={13} aria-hidden="true" /><ChevronDown size={12} aria-hidden="true" /></button>
            ) : null}
          </div>
        </>
      ) : null}
    </div>
  );
}

function FolderDropTarget({ folder, items, copy, folderDeleting, isItemPastePending, isItemDeletePending, onMove, onPaste, onDeleteFolder, onDeleteItem }: { folder: Folder; items: ClipboardItem[]; copy: Copy; folderDeleting: boolean; isItemPastePending: (id: string) => boolean; isItemDeletePending: (id: string) => boolean; onMove: (itemId: string, folderId: string | null) => Promise<void>; onPaste: (item: ClipboardItem) => Promise<void>; onDeleteFolder: (folder: Folder) => Promise<void>; onDeleteItem: (id: string) => Promise<void>; }) {
  const [open, setOpen] = useState(false);
  const [moveValue, setMoveValue] = useState('');
  const [movePending, setMovePending] = useState(false);
  const folderItems = items.filter((item) => item.folderId === folder.id);
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
          <GlassSelect
            className="folderMoveSelect"
            label={copy.moveSelected}
            value={moveValue}
            disabled={movePending}
            options={[{ value: '', label: copy.moveSelected }, ...items.map((item) => ({ value: item.id, label: item.preview.slice(0, 80) }))]}
            onChange={(value) => {
              setMoveValue(value);
              if (!value) return;
              setMovePending(true);
              void onMove(value, folder.id).finally(() => {
                setMovePending(false);
                setMoveValue('');
              });
            }}
          />
        </div>
      ) : null}
    </div>
  );
}

function QuickPoolRow({ item, copy, pending, pastePending, onPaste, onStar, onUpdate, onError, onDelete }: { item: QuickItem; copy: Copy; pending: boolean; pastePending: boolean; onPaste: (content: string) => Promise<void>; onStar: () => Promise<void>; onUpdate: (item: QuickItem) => void; onError: (error: unknown) => void; onDelete: () => Promise<void> }) {
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
    } catch (error) {
      onError(error);
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
