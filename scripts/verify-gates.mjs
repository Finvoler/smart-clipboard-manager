import { readFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

const requiredFiles = [
  'src-tauri/src/platform/windows_impl.rs',
  'src-tauri/src/db.rs',
  'src-tauri/src/quick_pool.rs',
  'src/App.tsx',
  'docs/reference-analysis.md',
];

for (const file of requiredFiles) {
  if (!existsSync(join(root, file))) {
    throw new Error(`Missing required file: ${file}`);
  }
}

const commands = readFileSync(join(root, 'src-tauri/src/commands.rs'), 'utf8');
const expectedCommands = [
  'hide_window',
  'execute_paste',
  'get_history',
  'update_item_text',
  'delete_item',
  'toggle_star',
  'get_folders',
  'move_to_folder',
  'get_quick_pool',
  'update_quick_item',
  'search_local',
  'search_ai_semantic',
  'trigger_ai_categorize',
  'trigger_ocr',
];

for (const command of expectedCommands) {
  if (!commands.includes(`fn ${command}`)) {
    throw new Error(`Missing IPC command: ${command}`);
  }
}

const ai = readFileSync(join(root, 'src-tauri/src/ai.rs'), 'utf8');
if (/loop\s*\{|set_interval|poll/i.test(ai)) {
  throw new Error('AI module must stay manual-trigger only; polling-like code was found.');
}

const tauriConfig = JSON.parse(readFileSync(join(root, 'src-tauri/tauri.conf.json'), 'utf8'));
if (tauriConfig.app.windows[0].visible !== false) {
  throw new Error('Main window must start hidden and only show from Win+V.');
}

console.log('Gate verification checks passed.');
