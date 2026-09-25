# 🎵 HotDownloader

> 基于共享 Rust 下载核心和 Vue 3 的音乐下载工具，支持 Tauri 桌面端、Android 端与 Docker/Web 部署。

![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.77.2+-orange.svg)
![Node](https://img.shields.io/badge/node-22.12+-green.svg)

---

## ✨ 特性

- 🔍 **音乐搜索**：支持多音源的关键词搜索、搜索建议、热搜词，结果支持分页加载更多
- 🎤 **关联浏览**：点击搜索结果中的歌手或专辑名称查看详情，返回时保留搜索状态与滚动位置
- 📋 **歌单搜索与导入**：支持歌单关键词搜索，或通过歌单链接/ID 导入并批量下载
- 🔎 **结果批量下载**：在搜索结果、歌单等列表中勾选多首歌曲统一加入下载队列
- ⬇️ **智能下载**：多任务并发、断点续传、下载链接按需刷新与自动重试
- 🔄 **自动降级**：以指定音质为起点，按用户配置的顺序选择可用品质
- 📊 **实时速度**：任务列表显示实时下载速度
- 🎵 **音频解密**：支持加密格式音频解密
- 🎤 **歌词与标签**：获取歌词并写入音频文件标签（支持封面、歌词），可独立下载 `.lrc` 歌词文件
- 🔐 **账号登录**：支持扫码登录和手动输入 `uin` / `authst` 登录，解锁会员歌曲与更高音质
- 🔁 **重复文件处理**：下载前检测同名文件，支持询问、覆盖、保留两份、取消四种策略，可在设置中配置
- 📱 **Android 适配**：默认使用系统 Download 目录，也支持通过 SAF 选择公共下载目录
- 🖥️ **系统托盘**：桌面端常驻托盘，支持显示/隐藏主窗口与退出应用，左键点击托盘图标显示窗口
- 📋 **任务管理**：按任务状态分类，支持批量删除、全部重试、取消、恢复；支持批量清除已下载或全部历史任务；桌面端支持“打开文件位置”；任务列表分页渲染，任务量大时保持流畅
- 🔔 **下载通知**：任务结果与链接状态变化时弹出应用内通知
- ⬆️ **检查更新**：从 GitHub Releases 检查新版本，展示更新说明，仅列出当前平台可用的安装包直链
- 🚪 **退出确认**：窗口关闭时若有进行中的下载任务，会先弹出二次确认
- ⚙️ **个性化设置**：默认音质、自动降级、下载目录、文件命名模板、歌手分隔符、并发数、自动跳转任务页、写入歌曲标签、保存 LRC 歌词等
- 🎨 **深色模式**：跟随系统主题，沉浸式视觉体验
- 💾 **持久化**：Tauri 客户端保存本地数据；独立服务在持久化数据目录中保存任务、设置与登录状态

---

## 🖥️ 技术栈

| 前端                    | 后端                    |
| ----------------------- | ----------------------- |
| Vue 3 (Composition API) | Rust 共享核心            |
| TypeScript              | Tokio (异步运行时)       |
| Vite                    | Reqwest (HTTP 客户端)    |
| Pinia                   | lofty (音频标签写入)     |
| Vue Router (Hash 模式)  | Tauri 2 / 独立 HTTP 服务 |
| Naive UI                | rumqttc (MQTT 登录)      |
|                         | Android SAF 适配器       |

---

## 📦 环境要求

请根据目标平台准备对应环境：

- **通用**：Rust、Node.js 22.12+ 与 npm
- **桌面端**：参考 [Tauri 桌面端前置要求](https://tauri.app/start/prerequisites/#system-dependencies)
- **Android 端**：参考 [Tauri Android 前置要求](https://tauri.app/start/prerequisites/#android)

---

## 🚀 快速开始

### 1. 克隆仓库

```bash
git clone https://github.com/lerdb/HotDownloader.git
cd HotDownloader
```

### 2. 安装依赖

```bash
npm install
```

### 3. 桌面端开发运行

```bash
npm run tauri dev
```

### 4. 桌面端构建

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`。

### 5. Docker/Web 部署

先在项目根目录创建 `.env`，设置至少 16 个字符的随机访问令牌：

```dotenv
HOTDOWNLOADER_TOKEN=请替换为随机生成的长令牌
```

然后启动服务：

```bash
docker compose up --build -d
```

浏览器访问 `http://服务器地址:8787`，输入 `.env` 中的访问令牌。容器在 `/data` 保存设置、QQ 凭据、任务记录和下载文件；Compose 使用 `hotdownloader-data` 卷持久化此目录。下载任务由服务进程持续执行，重新打开网页即可查看进度。服务进程重启后，可在任务页重试此前进行中的任务。

若允许非本机访问，请使用 HTTPS 反向代理保护浏览器与服务之间的访问令牌。独立服务的接口和环境变量详见 [服务说明](crates/hotdownloader-server/README.md)。

### 6. Android 端开发运行

```bash
npx tauri android dev
```

### 7. Android 端构建

```bash
npx tauri android build
```

---

## 📄 许可证

本项目基于 [Apache License 2.0](LICENSE) 开源。

### 第三方组件许可

本软件打包了完整的第三方许可证声明，随安装包分发：

- `NOTICE`：所有第三方组件的名称、版本、许可证标识
- `THIRD_PARTY_LICENSES.txt`：上述组件的许可证全文

安装后可在应用安装目录中找到以上文件；也可在应用内「设置 → 关于」页面查看组件列表，点击任意组件可展开对应许可证全文。

> 注：v1.3.2 之前的版本在 `NOTICE` 中仅列出直接依赖。自 v1.3.2 起，包含全部传递依赖及完整许可证文本。

---

## ⚠️ 免责声明

**HotDownloader 仅用于学习和研究目的。**

用户需自行承担使用本软件所带来的法律责任。请确保你下载的音乐文件拥有合法的使用权，遵守相关音乐平台的版权规定。本项目开发者不对任何侵权行为负责。
