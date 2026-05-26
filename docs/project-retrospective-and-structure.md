# Smart Clipboard 项目复盘与仓库结构说明

## 1. 这个项目最后做成了什么

这是一个基于 Tauri + React + Rust 的 Windows 智能剪贴板管理器，核心能力包括：

- 文本 / 图片剪贴板历史记录
- 自定义 `Win+V` 面板
- 托盘常驻与快速显示
- 图片 OCR
- AI 语义搜索与 AI 归档
- 文件夹分类、收藏、编辑、删除
- 临时池（Quick Pool）与高频短语候选
- 自定义数据目录：数据库、图片缓存和后续本地数据文件可以迁移到用户指定目录
- 开机启动

## 2. 这次做项目踩过的主要坑

### 2.1 莫名其妙弹出 cmd / PowerShell 新窗口

这个问题实际上分成了两层：

第一层是我自己代码里确实写错的东西：

- 启动快捷方式的 `ShowCmd` 一开始用了 `SW_SHOWMINNOACTIVE(7)`，这会让 Shell 以“最小化但可见”的方式启动目标程序。
- 重启子进程时没有加 `CREATE_NO_WINDOW`，理论上会给“控制台窗口被带出来”留下口子。
- Tauri release 一开始少了 `custom-protocol` feature，导致正式版去连 `127.0.0.1:1420` 的 dev server。

这些都是真 bug，也都已经修掉了。

但第二层才是最关键的误判：

- 你重启后看到的那个窗口，后来通过进程树确认，并不是 `smart_clipboard.exe` 拉起来的。
- 它的父进程是 `svchost.exe`，不是 `explorer.exe`。
- 继续查后发现真正来源是 Windows Terminal 自己的 UWP `StartupTask`：`StartTerminalOnLoginTask`，状态是 `Enabled(2)`。
- 所以那个 PowerShell 窗口本质上是 Windows Terminal 开机自启，不是本项目在直接启动终端。

最终修复动作有两类：

- 修我自己的 bug：`SW_HIDE`、移除 registry fallback、`CREATE_NO_WINDOW`、补上 `custom-protocol`
- 修系统环境干扰：关闭 Windows Terminal 的 `StartTerminalOnLoginTask`，并在它的 `settings.json` 里写 `startOnUserLogin: false`

### 2.2 Win 键 / Win+V 状态老是“松不开”或者判错

这个问题的根本原因不是“没监听到按键”，而是**状态机不完整**。

一开始最容易犯的错误是把逻辑写成：

- 看到 Win 按下就记一个布尔值
- 看到 V 按下就弹窗
- 看到 Win 抬起就清空

这种写法会漏掉几个关键分支：

- Win 被我们吞掉后，什么时候要“补发”给系统
- 用户按的是 Win+V 还是 Win+别的键
- Win 被吞掉但 V 抬起了，此时该不该结束 smart mode
- 有 injected key event 时要不要忽略
- 任何异常路径退出时，内部状态是否完整复位

最后稳定下来的做法是 4 个原子状态：

- `win_down`
- `win_forwarded`
- `smart_win_v_active`
- `held_win_vk`

它的核心思想是：

- Win 按下时先吞掉，不立刻决定交给谁
- 如果后续第二个键是 `V`，那就拦截并弹出本应用
- 如果第二个键不是 `V`，才把 Win 键补发回系统
- 收尾时所有状态统一走 `reset_win_hotkey_state()` 清零

真正难点不是 API，而是把所有转移边界想清楚。

### 2.3 数据保存路径不能只写在数据库里

后面加“文件保存路径”设置时，一个关键点是：**数据库打开之前就必须知道数据库在哪**。所以不能把“自定义数据目录”只存在 `app_settings` 表里，否则启动时会先去默认目录打开数据库，根本读不到用户想切换到哪里。

最后采用的是一个两层结构：

- `%APPDATA%\com.local.smartclipboard\storage-bootstrap.json`：很小的引导配置，只记录 `customDataDir` 和 `pendingMigration`
- 真正的数据目录：保存 `smart_clipboard.sqlite`、`images\` 和后续本地数据文件

切换目录时不会立刻在当前进程里搬数据库，而是：

1. 前端保存设置并确认迁移
2. Rust 写入 bootstrap 的 `pendingMigration`
3. 应用重启
4. 新进程在 SQLite 打开前复制旧目录到新目录、重写图片路径、清掉 pending

这次还踩了两个细节坑：

- PowerShell 5.1 写 JSON 的 UTF-8 可能带 BOM，`serde_json` 默认不吃 BOM，所以读取 bootstrap 时要主动剥掉 BOM。
- 如果迁移已经复制完数据库，但还没来得及清 `pendingMigration` 就中断，下一次启动不能因为“目标目录非空”直接失败；目标里已有 `smart_clipboard.sqlite` 时要把它视为可收敛状态。

最新版还给 Tauri capability 补上了 `dialog:default`，否则前端 `@tauri-apps/plugin-dialog` 的“选择文件夹”按钮不会真正打开系统选择器。同时路径输入框也允许手工输入，作为系统对话框之外的交互兜底。

### 2.4 H 盘 exe 重复启动、重启失败、数据目录与 exe 同目录冲突

这是这轮里最折腾、也最值得记住的一组问题。

一开始暴露出来的是两个现象：

- 双击 `H:\Clipboard\SmartClipboard.exe` 之后，托盘里会出现两个 Smart Clipboard 进程。
- 点击“保存路径并重启”后，Windows 会弹出“找不到 `\\` 文件”。

根因后来拆成了三层：

1. **没有单实例保护**
  - 程序每启动一次都会完整初始化托盘、剪贴板监听和窗口。
  - 所以从 H 盘重复打开 exe 时，系统层面不会帮你合并实例，应用自己也没拦。

2. **Windows 的 `cmd /c start` 不是可靠的重启器**
  - 当路径带引号、空格、中文目录或者 `\\?\` 前缀时，`cmd start` 的解析非常脆。
  - 这次直接复现成了“找不到 `\\` 文件”。

3. **允许把数据目录指到 exe 所在目录后，迁移逻辑不能再把整个目录当成应用私有目录**
  - 以前如果把整个目录 copy/remove，会把 `SmartClipboard.exe` 一起搬动甚至删掉。
  - 这在 `H:\Clipboard` 这种“exe 和数据库同目录”的便携式布局下是不能接受的。

最后稳定下来的修复方案是：

- Tauri 启动最前面挂 `tauri-plugin-single-instance`，让第二次启动只激活已有窗口。
- Windows 重启改成隐藏 PowerShell `Start-Process`，并延迟 1.5 秒等旧进程退出。
- 对重启路径先做 `\\?\` / `\\?\UNC\` 规范化，再做 PowerShell 单引号转义。
- 数据目录迁移只复制 / 删除 `smart_clipboard.sqlite*` 与 `images\`。
- 目标目录校验只拦已有 Smart Clipboard 数据的目录，不拦 `SmartClipboard.exe` 这类无关文件。
- v0.1.7 额外补了退出状态标记：普通点 X 仍然隐藏到托盘，但托盘退出、应用重启、Tauri 退出事件开始后不再拦截关闭；panic 自动重启也会在退出中禁用，避免关机阶段再拉起重启 helper。

这部分最后不是只靠单元测试收尾，而是直接用实际部署在 H 盘的 exe 做了往返验证：

- `H:\Clipboard -> E:\SmartClipboardRestartSmoke`
- `E:\SmartClipboardRestartSmoke -> H:\Clipboard`

验证通过的标准不是“看起来重启了”，而是：

- `%APPDATA%\com.local.smartclipboard\storage-bootstrap.json` 的 `pendingMigration == null`
- 目标目录数据库真实存在
- 临时目录被清理
- `H:\Clipboard\SmartClipboard.exe` 仍然存在
- 运行中只剩一个 SmartClipboard 进程

## 3. 这次我自己犯过的错误

### 3.1 过早下结论

我一开始过早把“开机终端窗口”归因为项目自己的启动快捷方式，虽然方向不完全错，但没有第一时间做**进程父子链**核查，导致排查路线兜了圈子。

### 3.2 修了真实 bug，但一度没命中“用户看到的那个窗口”

`SW_SHOWMINNOACTIVE -> SW_HIDE`、`CREATE_NO_WINDOW` 这些修复都对，但它们修的是“项目自身可能带出的窗口”，不是截图里那个真正由 Windows Terminal StartupTask 拉起的窗口。也就是说，我修到了 bug，但没有第一时间修到“你眼前那个现象”的直接来源。

### 3.3 对 release / dev 行为边界检查不够早

`custom-protocol` 缺失这类问题，本应在第一次 release 自测时就定位，不该拖到用户看到 `127.0.0.1 refused connection` 才补。

### 3.4 发布文案和代码行为一度脱节

README / Release 里一度还保留了：

- registry fallback
- minimized launch style

但代码已经改成了：

- 清理旧 registry 项，不再回退写入
- startup shortcut 用 `SW_HIDE`

这说明我在“修代码”和“修文案”之间同步不够及时，后来已经补齐到 `v0.1.5`。

## 4. 这次项目用了什么语言，怎么分层

### 4.1 语言

这个项目主要用到：

- Rust：后端、系统集成、数据库、AI 调用、Windows 原生 API
- TypeScript：前端类型与 Tauri IPC 调用层
- TSX / React：主界面
- CSS：界面样式
- JSON：Tauri 配置、capability、schema
- JavaScript（Node ESM）：构建校验脚本
- SQL：嵌在 Rust 里，通过 `rusqlite` 执行

### 4.2 架构分层

整体是一个典型的 Tauri 双端架构：

- 前端：React + TypeScript
- 桌面壳：Tauri
- 后端：Rust
- 数据层：SQLite
- Windows 集成层：Win32 API
- AI 适配层：HTTP API（OpenAI-compatible / Anthropic-compatible）

简单理解：

1. React 负责显示 UI
2. 前端通过 Tauri command 调 Rust
3. Rust 命令层调数据库 / 平台集成 / AI 模块
4. Windows 系统交互由 `windows_impl.rs` 直接处理
5. 数据持久化全部走 `db.rs`

## 5. 作为资深程序员看，这个架构写得怎么样

### 5.1 写得比较好的地方

- 数据层集中在 `db.rs`，没有散到命令层和 UI 里
- 平台相关逻辑收进 `platform/`，没有污染所有业务模块
- 前后端共享模型清晰，`models.rs` / `types.ts` 对应关系明确
- release 构建和 verify 脚本有了基本发布闸门
- Win+V 最后收敛成明确状态机，而不是一堆零散 if

### 5.2 不够好的地方

这个仓库还不是“屎山”，但已经出现了几个明显的可维护性问题：

- `src/App.tsx` 太大，承担了过多 UI、状态、文案、交互职责
- `windows_impl.rs` 过长，剪贴板、快捷键、窗口恢复、启动快捷方式都挤在一个文件里
- `db.rs` 同时承担 schema、迁移、查询、业务规则，规模继续长大会很重
- 错误类型大多用 `String` 往上抛，调试和分层表达都不够强
- 配置项、AI 协议细节、模型默认值分散在多个地方
- 数据目录迁移、重启、自启动、便携式部署之间的约束一开始没有被当成一个整体设计，导致需求一变化就连续冒边界 bug

### 5.3 更好的架构演进方向

如果要继续往长期维护方向走，我建议这样拆：

#### 后端 Rust

- `platform/windows_impl.rs` 拆成：
  - `clipboard_listener.rs`
  - `hotkey.rs`
  - `window_control.rs`
  - `startup.rs`
  - `input_simulation.rs`

- `db.rs` 拆成：
  - `schema.rs`
  - `settings_repo.rs`
  - `history_repo.rs`
  - `quick_pool_repo.rs`
  - `folder_repo.rs`

- 命令层可按领域拆：
  - `commands/history.rs`
  - `commands/settings.rs`
  - `commands/quick_pool.rs`
  - `commands/ai.rs`

- 错误统一改成领域错误枚举，不要大量 `Result<T, String>`

#### 前端 React

- `App.tsx` 拆成页面容器 + 组件树
- 多语言文案抽离到单独文件
- IPC 封装保持在 `tauriClient.ts`，但业务 hooks 拆出去
- 图片预览、设置面板、历史列表、Quick Pool 独立组件化

## 6. 这个项目有没有“屎山”

结论：**还没到屎山，但已经长成一块“单体大石头”了。**

原因是：

- 逻辑总体还是可读、可构建、可验证的
- 关键模块职责基本还能辨认
- 没有那种完全没边界、没人敢动的循环依赖

但如果继续不拆：

- `App.tsx`
- `windows_impl.rs`
- `db.rs`

这三个文件会最先变成真正的维护负担。

## 7. 你要看懂这个完整项目，需要学会哪些语言和知识

### 必学语言

- TypeScript
- React / TSX
- Rust
- CSS
- JSON

### 还要懂的配套知识

- Tauri 的 command / event / window 生命周期
- SQLite 基本 CRUD 和 schema
- Windows 键盘钩子、前台窗口、快捷方式、模拟输入
- HTTP API 调用和 JSON 解析
- 基本打包 / 构建流程（Vite、Cargo、Tauri build）

### 建议学习顺序

1. 先看 TypeScript + React 基础
2. 再看 Tauri 前后端通信
3. 再看 Rust 基础语法与所有权
4. 然后看 SQLite 与 `rusqlite`
5. 最后再看 Windows API 和键盘钩子

如果一上来直接读 `windows_impl.rs`，会非常痛苦。

## 8. 这次问题的技术实现总结

如果只看这次修复，可以把技术实现压缩成四个点：

1. **单实例**
  - 用 `tauri-plugin-single-instance`。
  - 第二次启动不继续初始化系统资源，而是把主窗口 show / focus。

2. **重启**
  - 不用 `cmd /c start`，改用 PowerShell `Start-Process`。
  - 为了兼容 Windows 的奇怪路径前缀，要先归一化路径，再做 PowerShell 字符串转义。

3. **数据目录引导**
  - 数据目录不能只存 SQLite，因为打开 SQLite 之前就要知道目录在哪。
  - 所以用 `%APPDATA%\com.local.smartclipboard\storage-bootstrap.json` 做 pre-DB bootstrap。

4. **迁移幂等与同目录部署**
  - 迁移只移动应用数据，不碰 exe。
  - 启动时看见 `pendingMigration` 要能继续收敛，而不是直接报“目录非空”。

## 9. 下次做新项目必须先确认的开发规范

这些规范最好在项目第一周就定死，不然后面都会变成补洞：

1. 明确“安装目录”和“数据目录”是否允许同目录。
2. 任何自启动、重启、单实例、托盘常驻需求都要在第一个可运行版本就做实机验证，不要只在 IDE 里跑 dev。
3. Windows 路径相关逻辑禁止依赖 `cmd /c start` 这类有历史包袱的壳命令，优先直接 API 或 PowerShell `Start-Process`。
4. 所有涉及文件迁移的逻辑都必须先定义“应用真正拥有的文件集合”，禁止对整个目录做想当然的 copy/remove。
5. release 行为和 dev 行为要分开写验收清单，至少覆盖：启动、重启、升级、迁移、自启动、卸载残留。
6. 前后端配置项必须有单一事实来源，避免 README、release notes、默认值、UI 文案四处漂移。
7. 每修一次线上问题，都要补一个最小回归测试或一条可重复的 smoke case，不然同类问题会回来。

## 10. 这套架构是不是最优

结论很直接：**不是最优，但方向基本正确。**

它适合一个 Windows 本地桌面工具快速做出完整能力闭环，尤其是：

- React 做 UI 迭代快
- Rust 做本地性能、系统集成和 SQLite 稳定
- Tauri 把桌面壳成本压低

但如果目标是长期迭代并持续加复杂系统能力，当前代码组织还需要继续拆：

- 前端把巨型 `App.tsx` 拆掉
- 后端把 `windows_impl.rs` / `db.rs` 拆成更细领域模块
- 把 `String` 错误改成结构化错误
- 给路径迁移、重启、自启动做单独的集成测试层

也就是说，**架构选型不差，主要问题不是框架错，而是边界在实现阶段收得还不够早。**

## 8. 仓库结构总览

下面按当前保留形式说明每个被跟踪文件负责什么。

### 顶层文件

- `.env.example`：示例环境变量模板，不包含真实密钥
- `.gitignore`：忽略本地依赖、构建产物、数据库、图片历史、zip 包
- `README.md`：面向使用者的项目说明、安装、开发、发布说明
- `index.html`：Vite 前端挂载页
- `package.json`：前端依赖与 npm scripts
- `package-lock.json`：前端依赖锁文件
- `tsconfig.json`：TypeScript 编译配置
- `vite.config.ts`：Vite dev server 和 watch 配置

### docs/

- `docs/reference-analysis.md`：前期参考产品 / 实现思路分析
- `docs/project-retrospective-and-structure.md`：本文件，项目复盘与结构学习说明

### scripts/

- `scripts/verify-gates.mjs`：轻量发布校验脚本，检查关键文件 / 命令 / 配置是否存在

### release/

- `release/RELEASE_NOTES_v0.1.1.md`：当前正式版本的发布说明与校验值
- `release/RELEASE_NOTES_v0.1.2.md`：数据目录功能版本的发布说明、实测结果与校验值
- `release/RELEASE_NOTES_v0.1.3.md`：设置页排版顺序版本的发布说明与校验值
- `release/RELEASE_NOTES_v0.1.4.md`：设置页间距修复版本的发布说明与校验值
- `release/RELEASE_NOTES_v0.1.5.md`：上一版本的发布说明、路径保存交互修复与校验值
- `release/RELEASE_NOTES_v0.1.6.md`：单实例、重启与数据目录迁移修复版本的发布说明
- `release/RELEASE_NOTES_v0.1.7.md`：当前正式版本的发布说明，记录关机/退出阶段加固

### src/

- `src/main.tsx`：React 前端入口
- `src/App.tsx`：主界面容器，管理大部分前端状态与交互
- `src/styles.css`：全局与主界面样式
- `src/tauriClient.ts`：前端调用 Tauri command / 订阅事件的统一封装
- `src/types.ts`：前端业务类型定义

### src-tauri/

- `src-tauri/Cargo.toml`：Rust 依赖与 crate 配置
- `src-tauri/Cargo.lock`：Rust 依赖锁文件
- `src-tauri/build.rs`：Tauri 构建脚本入口
- `src-tauri/tauri.conf.json`：Tauri 应用配置、窗口配置、前端构建入口

### src-tauri/capabilities/

- `src-tauri/capabilities/default.json`：Tauri capability 配置

### src-tauri/gen/schemas/

这些是 Tauri 生成的 schema 文件，严格说属于生成物，但体积很小，而且对理解 capability / schema 边界有帮助，所以保留：

- `src-tauri/gen/schemas/acl-manifests.json`
- `src-tauri/gen/schemas/capabilities.json`
- `src-tauri/gen/schemas/desktop-schema.json`
- `src-tauri/gen/schemas/windows-schema.json`

### src-tauri/icons/

- `src-tauri/icons/icon.ico`：应用图标

### src-tauri/src/

- `src-tauri/src/main.rs`：原生可执行入口
- `src-tauri/src/lib.rs`：后端总装配入口
- `src-tauri/src/models.rs`：前后端共享数据模型
- `src-tauri/src/commands.rs`：Tauri IPC 命令层
- `src-tauri/src/db.rs`：SQLite 数据层
- `src-tauri/src/ai.rs`：AI 协议适配与调用
- `src-tauri/src/quick_pool.rs`：临时池候选抽取规则

### src-tauri/src/platform/

- `src-tauri/src/platform/mod.rs`：平台抽象入口
- `src-tauri/src/platform/windows_impl.rs`：Windows 原生实现
- `src-tauri/src/platform/fallback.rs`：非 Windows 占位实现

## 9. 这次收尾阶段我判断哪些东西是冗余产物

### 已判定为可删的本地产物

- `node_modules/`：依赖缓存，可随时 `npm install` 重建
- `dist/`：前端构建产物，可由 `npm run build` 重建
- `_reference/`：本地参考竞品目录，不是项目源码
- `release/SmartClipboard-v0.1.0-windows-x64/`：旧版本地解包目录
- `release/SmartClipboard-v0.1.0-windows-x64.zip`：旧版本地 zip
- `release/SmartClipboard-v0.1.1-windows-x64/`：旧版本地解包目录
- `release/SmartClipboard-v0.1.1-windows-x64.zip`：旧版本地 zip（GitHub Release 已有，不必留本地仓库）
- `release/SmartClipboard-v0.1.2-windows-x64/`：旧版本地解包目录
- `release/SmartClipboard-v0.1.2-windows-x64.zip`：旧版本地 zip（GitHub Release 已有，不必留本地仓库）
- `release/SmartClipboard-v0.1.3-windows-x64/`：旧版本地解包目录
- `release/SmartClipboard-v0.1.3-windows-x64.zip`：旧版本地 zip（GitHub Release 已有，不必留本地仓库）
- `release/SmartClipboard-v0.1.4-windows-x64/`：旧版本地解包目录
- `release/SmartClipboard-v0.1.4-windows-x64.zip`：旧版本地 zip（GitHub Release 已有，不必留本地仓库）
- `release/SmartClipboard-v0.1.5-windows-x64/`：当前版本地解包目录
- `release/SmartClipboard-v0.1.5-windows-x64.zip`：当前版本地 zip（GitHub Release 上传后不必留本地仓库）

### 保留但说明原因的文件

- `release/RELEASE_NOTES_v0.1.1.md`：小而有用，保留历史版本说明与校验值
- `release/RELEASE_NOTES_v0.1.2.md`：数据目录功能版本说明、校验值与实测记录
- `release/RELEASE_NOTES_v0.1.3.md`：设置页排版顺序版本说明、校验值与修复记录
- `release/RELEASE_NOTES_v0.1.4.md`：设置页间距修复版本说明、校验值与修复记录
- `release/RELEASE_NOTES_v0.1.5.md`：上一版本说明、校验值与文件路径保存交互修复记录
- `release/RELEASE_NOTES_v0.1.6.md`：单实例/重启/迁移安全修复记录
- `release/RELEASE_NOTES_v0.1.7.md`：当前版本说明、退出与关机阶段加固记录
- `src-tauri/gen/schemas/*`：生成文件，但有学习价值，也不大
- `docs/reference-analysis.md`：前期设计痕迹，保留做背景资料

额外说明：为了让“删掉 dist 后的干净仓库”仍能执行 `cargo test`，`src-tauri/build.rs` 会在编译期自动补一个极小的占位 `dist/index.html`。真正的前端资源仍然来自 `npm run build`。

## 9.1 最新版数据目录实测结果

最新版用真实 release exe 做过两轮路径切换：

- `H:\Clipboard\SmartClipboard.exe` 启动后，设置页实际生效目录就是 `H:\Clipboard`。
- 通过真实 UI 从 `H:\Clipboard` 切到 `E:\SmartClipboardRestartSmoke`，重启后 `storage-bootstrap.json` 指向 `E:\SmartClipboardRestartSmoke`，目标目录出现 `smart_clipboard.sqlite`，H 盘 exe 仍然存在且运行中只剩一个进程。
- 再从 `E:\SmartClipboardRestartSmoke` 切回 `H:\Clipboard`，重启后 `storage-bootstrap.json` 回到 `H:\Clipboard`，`pendingMigration == null`，H 盘数据库恢复存在，临时目录被迁移清空。

这次不是只做脚本层验证，而是直接对 `H:\Clipboard\SmartClipboard.exe` 做真实往返切换。最终确认：数据已经稳定保存在 H 盘，重启后不会多开，迁移也不会碰 `SmartClipboard.exe` 本体。

## 9.2 当前机器的实际生效版本与启动源

当前机器已经核对过“谁会在重启或开机后真正工作”：

- 运行中的 Smart Clipboard 只有 H 盘这一份：`H:\Clipboard\SmartClipboard.exe`
- 唯一开机启动入口是 `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\Smart Clipboard Manager.lnk`
- 这个启动快捷方式明确指向 `H:\Clipboard\SmartClipboard.exe --startup`
- 没有 Smart Clipboard 的 registry `Run` 残留
- 没有 Smart Clipboard 的计划任务残留

这意味着只要清掉仓库里旧的 release exe 副本，就不会出现“电脑重启后多个不同版本一起干活”的情况。

v0.1.3 又修了一次设置页信息架构：`文件保存路径` 不再夹在 API base URL、API key、模型选择之间，而是移动到“保存 / 测试 / 模型”按钮之后，作为设置页最后的本地存储配置块。这样 API/模型设置和本地数据目录设置在视觉顺序上分开，用户不会误以为文件路径属于大模型配置。

v0.1.4 继续调整这个区域的视觉间距：`文件保存路径` 模块顶部增加留白和分隔线，避免它的标题贴着上面的“模型”按钮，看起来像同一个 API 配置组。

v0.1.5 修正了一个更关键的交互误导：之前“选择文件夹”只会改输入框，必须再点上方的“保存”才会迁移，但保存按钮已经属于 API/模型区域，用户很容易以为路径已经生效。现在文件路径模块内部有独立的“保存路径并重启”按钮；当输入路径与已保存路径不同时，会显示“待切换到”，而“当前实际数据目录”继续只表示已经生效、正在写入的目录。

v0.1.7 针对关机时短暂弹窗做了排查：系统日志里没有 SmartClipboard、PowerShell 重启 helper 或路径解析失败记录；更可疑的是退出阶段仍可能沿用“关闭窗口等于隐藏托盘”的常驻逻辑。现在代码把“用户关闭窗口”和“应用正在退出”分开，退出/重启/系统结束会放行，平时点 X 仍然回到托盘。

## 10. 我这次可用的 skill 体系

下面是当前环境里我可调用的技能（skills）：

- `brutalist-skill`：工业粗野 / 瑞士硬核 UI
- `clone-website`：网站复刻与反向重建
- `expand-course-notes`：课程笔记扩写
- `gpt-taste`：更强表达力的前端审美与动效
- `harness-multi-agent-engineering`：并发执行大任务
- `minimalist-skill`：极简编辑感 UI
- `redesign-skill`：现有网站 / 应用重设计
- `soft-skill`：柔和精致高级 UI
- `taste-skill`：高审美前端实现
- `project-setup-info-local`：完整项目初始化与脚手架
- `get-search-view-results`：读取 VS Code 搜索视图结果
- `agent-customization`：自定义 agent / skill / prompt 文件

当前环境里还能用的专门 agent 有：

- `Course Bulk`
- `Parallel Executor`
- `Explore`

## 11. 我对自己 skill 体系的复盘

如果要系统构建一个更强的 skill 体系，我会把它分成四层：

### 第一层：通用工程技能

- 仓库清理
- 发布流程
- 测试与验证
- 配置排错
- Git / Release / CI

### 第二层：平台技能

- Windows 桌面集成
- Tauri / Electron
- 前端构建工具链
- Python / Node / Rust 项目组织

### 第三层：领域技能

- 剪贴板类应用
- OCR / 图像处理
- AI 检索与归档
- 课程内容处理
- 网站复刻 / UI 重设计

### 第四层：工作流技能

- 并行 agent 协作
- Prompt / instruction / skill 自定义
- 代码审查模式
- 发布与回归检查 checklist

如果把这四层搭好，很多项目就能从“靠临场发挥”变成“靠稳定模板 + 特定领域知识”来做，错误会少很多。
