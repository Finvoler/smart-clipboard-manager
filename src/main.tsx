/* React 前端入口：挂载 App，导入全局样式、KaTeX 和代码高亮主题。 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import 'katex/dist/katex.min.css';
import 'highlight.js/styles/github.css';
import './styles.css';
import { App } from './App';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
