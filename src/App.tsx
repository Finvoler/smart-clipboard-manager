import { useEffect, useMemo, useState } from 'react';
import { Archive, Bot, ChevronLeft, ChevronRight, Clock, Edit3, FolderPlus, Image as ImageIcon, Pin, Search, Star, Trash2, X } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import rehypeHighlight from 'rehype-highlight';
import rehypeKatex from 'rehype-katex';
import remarkGfm from 'remark-gfm';
import remarkMath from 'remark-math';
import { call, fileSrc, onNewItem, onQuickPoolExtracted, type ClipboardItem, type Folder, type QuickItem } from './tauriClient';

const DEFAULT_LIMIT = 120;

export function App() {
  const [items, setItems] = useState<ClipboardItem[]>([]);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [quickItems, setQuickItems] = useState<QuickItem[]>([]);
  const [query, setQuery] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingText, setEditingText] = useState('');
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [status, setStatus] = useState('');

  async function refresh() {
    const [history, folderList, pool] = await Promise.all([
      call<ClipboardItem[]>('get_history', { limit: DEFAULT_LIMIT, offset: 0 }),
      call<Folder[]>('get_folders'),
      call<QuickItem[]>('get_quick_pool'),
    ]);
    setItems(history);
    setFolders(folderList);
    setQuickItems(pool);
    setSelectedId((current) => current ?? history[0]?.id ?? null);
  }

  useEffect(() => {
    void refresh();

    const cleanups: Array<() => void> = [];
    void onNewItem((item) => {
      setItems((current) => [item, ...current.filter((candidate) => candidate.id !== item.id)]);
      setSelectedId(item.id);
    }).then((cleanup) => cleanups.push(cleanup));

    void onQuickPoolExtracted((item) => {
      setQuickItems((current) => [item, ...current.filter((candidate) => candidate.id !== item.id)]);
    }).then((cleanup) => cleanups.push(cleanup));

    return () => cleanups.forEach((cleanup) => cleanup());
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        void call('hide_window');
      }

      if (event.key === 'Enter' && selectedId && editingId === null) {
        event.preventDefault();
        void executePaste(selectedId);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [selectedId, editingId]);

  const filteredItems = useMemo(() => {
    if (!query.trim()) return items;
    const keyword = query.trim().toLowerCase();
    return items.filter((item) => `${item.preview} ${item.content ?? ''}`.toLowerCase().includes(keyword));
  }, [items, query]);

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

  async function runAiSearch() {
    try {
      const ids = await call<string[]>('search_ai_semantic', { query });
      setStatus(`AI returned ${ids.length} candidate ids`);
      setItems((current) => current.filter((item) => ids.includes(item.id)));
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function executePaste(id: string, overrideText?: string) {
    await call('execute_paste', { itemId: id, overrideText });
  }

  async function saveEdit(id: string) {
    const updated = await call<ClipboardItem>('update_item_text', { id, text: editingText });
    setItems((current) => current.map((item) => (item.id === id ? updated : item)));
    setEditingId(null);
    setEditingText('');
  }

  async function toggleStar(item: ClipboardItem) {
    const updated = await call<ClipboardItem>('toggle_star', { id: item.id, isStar: !item.isStar });
    setItems((current) => current.map((candidate) => (candidate.id === item.id ? updated : candidate)));
  }

  async function removeItem(id: string) {
    await call('delete_item', { id });
    setItems((current) => current.filter((item) => item.id !== id));
  }

  async function moveToFolder(itemId: string, folderId: string | null) {
    const updated = await call<ClipboardItem>('move_to_folder', { itemId, folderId });
    setItems((current) => current.map((item) => (item.id === itemId ? updated : item)));
  }

  async function createFolder() {
    const name = window.prompt('Folder name');
    if (!name?.trim()) return;
    const folder = await call<Folder>('create_folder', { name: name.trim() });
    setFolders((current) => [...current, folder]);
  }

  async function categorize() {
    try {
      const updated = await call<ClipboardItem[]>('trigger_ai_categorize');
      setStatus(`AI categorized ${updated.length} records`);
      await refresh();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function runOcr(item: ClipboardItem) {
    try {
      const created = await call<ClipboardItem>('trigger_ocr', { imageId: item.id });
      setItems((current) => [created, ...current]);
      setSelectedId(created.id);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <main className={`shell ${sidebarCollapsed ? 'sidebar-collapsed' : ''}`}>
      <section className="timeline" aria-label="Clipboard history">
        <header className="toolbar">
          <div className="searchBox">
            <Search size={16} />
            <input
              value={query}
              onChange={(event) => void runLocalSearch(event.target.value)}
              placeholder="Search clipboard"
              autoFocus
            />
          </div>
          <button className="iconButton" onClick={runAiSearch} title="LLM semantic search">
            <Bot size={17} />
          </button>
          <button className="iconButton" onClick={categorize} title="AI archive">
            <Archive size={17} />
          </button>
          <button className="iconButton" onClick={() => void call('hide_window')} title="Hide">
            <X size={17} />
          </button>
        </header>

        {status ? <div className="statusLine">{status}</div> : null}

        <div className="historyList">
          {filteredItems.map((item) => (
            <article
              key={item.id}
              className={`historyItem ${selectedId === item.id ? 'selected' : ''}`}
              onMouseEnter={() => setSelectedId(item.id)}
            >
              <div className="itemHeader">
                <button className="pasteTarget" onClick={() => executePaste(item.id)} title="Paste this item">
                  {item.kind === 'image' ? <ImageIcon size={16} /> : <Clock size={16} />}
                  <span>{formatTime(item.createdAt)}</span>
                </button>
                <div className="itemActions">
                  <button className="iconButton small" onClick={() => toggleStar(item)} title="Star">
                    <Star size={15} fill={item.isStar ? 'currentColor' : 'none'} />
                  </button>
                  {item.kind === 'text' ? (
                    <button
                      className="iconButton small"
                      onClick={() => {
                        setEditingId(item.id);
                        setEditingText(item.content ?? '');
                      }}
                      title="Edit text"
                    >
                      <Edit3 size={15} />
                    </button>
                  ) : (
                    <button className="iconButton small" onClick={() => runOcr(item)} title="OCR image">
                      <Bot size={15} />
                    </button>
                  )}
                  <button className="iconButton small danger" onClick={() => removeItem(item.id)} title="Delete">
                    <Trash2 size={15} />
                  </button>
                </div>
              </div>

              {editingId === item.id ? (
                <div className="editorBlock">
                  <textarea value={editingText} onChange={(event) => setEditingText(event.target.value)} />
                  <div className="editorActions">
                    <button onClick={() => saveEdit(item.id)}>Save</button>
                    <button onClick={() => setEditingId(null)}>Cancel</button>
                  </div>
                </div>
              ) : item.kind === 'image' ? (
                <ImagePreview item={item} />
              ) : (
                <button className="markdownButton" onClick={() => executePaste(item.id)}>
                  <ReactMarkdown remarkPlugins={[remarkGfm, remarkMath]} rehypePlugins={[rehypeKatex, rehypeHighlight]}>
                    {item.content ?? item.preview}
                  </ReactMarkdown>
                </button>
              )}
            </article>
          ))}
        </div>
      </section>

      <aside className="sidebar" aria-label="Clipboard sidebar">
        <button className="collapseButton" onClick={() => setSidebarCollapsed((value) => !value)} title="Toggle sidebar">
          {sidebarCollapsed ? <ChevronLeft size={17} /> : <ChevronRight size={17} />}
        </button>

        <section className="sideSection">
          <h2><Star size={15} /> Starred</h2>
          {items.filter((item) => item.isStar).slice(0, 8).map((item) => (
            <button key={item.id} className="sideItem" onClick={() => executePaste(item.id)}>{item.preview}</button>
          ))}
        </section>

        <section className="sideSection">
          <div className="sectionTitleRow">
            <h2><FolderPlus size={15} /> Folders</h2>
            <button className="iconButton small" onClick={createFolder} title="Create folder"><FolderPlus size={14} /></button>
          </div>
          {folders.map((folder) => (
            <FolderDropTarget key={folder.id} folder={folder} items={items} onMove={moveToFolder} onPaste={executePaste} />
          ))}
        </section>

        <section className="sideSection">
          <h2><Pin size={15} /> Quick Pool</h2>
          {quickItems.map((item) => (
            <QuickPoolRow key={item.id} item={item} onPaste={(content) => executePaste('', content)} />
          ))}
        </section>
      </aside>
    </main>
  );
}

function ImagePreview({ item }: { item: ClipboardItem }) {
  const src = fileSrc(item.imagePath);
  return (
    <button className="imageCard" onClick={() => item.content && undefined}>
      {src ? <img src={src} alt={item.preview} /> : <ImageIcon size={42} />}
      <span>{item.width ?? '?'} x {item.height ?? '?'}</span>
      {src ? <img className="hoverImage" src={src} alt="Full preview" /> : null}
    </button>
  );
}

function FolderDropTarget({ folder, items, onMove, onPaste }: { folder: Folder; items: ClipboardItem[]; onMove: (itemId: string, folderId: string | null) => Promise<void>; onPaste: (id: string) => Promise<void>; }) {
  const folderItems = items.filter((item) => item.folderId === folder.id).slice(0, 5);
  return (
    <div className="folderBlock">
      <div className="folderName">{folder.name}</div>
      {folderItems.map((item) => (
        <button key={item.id} className="sideItem" onClick={() => onPaste(item.id)}>{item.preview}</button>
      ))}
      <select defaultValue="" onChange={(event) => event.target.value && void onMove(event.target.value, folder.id)}>
        <option value="">Move selected...</option>
        {items.map((item) => <option key={item.id} value={item.id}>{item.preview.slice(0, 42)}</option>)}
      </select>
    </div>
  );
}

function QuickPoolRow({ item, onPaste }: { item: QuickItem; onPaste: (content: string) => Promise<void> }) {
  const [content, setContent] = useState(item.content);
  const [ttl, setTtl] = useState('86400');

  async function save() {
    await call<QuickItem>('update_quick_item', { id: item.id, content, ttl: Number(ttl) });
  }

  return (
    <div className="quickRow">
      <button className="sideItem strong" onClick={() => onPaste(content)}>{content}</button>
      <input value={content} onChange={(event) => setContent(event.target.value)} />
      <select value={ttl} onChange={(event) => setTtl(event.target.value)}>
        <option value="3600">1h</option>
        <option value="86400">24h</option>
        <option value="604800">7d</option>
        <option value="0">Pin</option>
      </select>
      <button onClick={save}>Save</button>
    </div>
  );
}

function formatTime(seconds: number) {
  return new Date(seconds * 1000).toLocaleString(undefined, { month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
}
