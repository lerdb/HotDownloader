# 🎵 HotDownloader

> 基于 Tauri 2 和 Vue 3 的跨平台音乐下载工具，支持桌面端（Windows/macOS/Linux）与 Android 端。

![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.77.2+-orange.svg)
![Node](https://img.shields.io/badge/node-18+-green.svg)

---

## ✨ 特性

- 🔍 **音乐搜索**：关键词搜索、搜索建议、热搜词，支持搜索结果分页加载更多
- 📋 **歌单导入**：支持音乐歌单链接/ID 导入并批量下载
- ⬇️ **智能下载**：多任务并发、断点续传、链接过期自动重试，下载链接实时获取
- 🔄 **自动降级**：指定音质不可用时按预设顺序自动降级
- 📊 **实时速度**：任务列表显示实时下载速度
- 🎵 **音频解密**：支持加密格式音频解密
- 🎤 **歌词与标签**：获取歌词并写入音频文件标签（支持封面、歌词），可独立下载 `.lrc` 歌词文件
- 🔐 **账号登录**：支持扫码登录和手动输入 `uin` / `authst` 登录，解锁会员歌曲与更高音质
- 🔁 **重复文件处理**：下载前检测同名文件，支持询问、覆盖、保留两份、取消四种策略，可在设置中配置
- 📱 **Android 适配**：通过 SAF 选择公共下载目录；未选择时默认使用系统 Download 目录
- 📋 **任务管理**：等待/下载/暂停/完成/错误状态分类，支持批量删除、重试、取消、恢复；桌面端支持“打开文件位置”
- 🔔 **下载通知**：任务完成/失败/链接过期时弹出应用内通知
- ⚙️ **个性化设置**：默认音质、自动降级、下载目录、文件命名模板、并发数、自动跳转任务页、写入歌曲标签、保存 LRC 歌词等
- 🎨 **深色模式**：跟随系统主题，沉浸式视觉体验
- 💾 **本地持久化**：任务、设置、搜索历史、登录状态本地保存

---

## 🖥️ 技术栈

| 前端                    | 后端                    |
| ----------------------- | ----------------------- |
| Vue 3 (Composition API) | Rust (Tauri 2)          |
| TypeScript              | Tokio (异步运行时)      |
| Vite                    | Reqwest (HTTP 客户端)   |
| Pinia                   | lofty (音频标签写入)    |
| Vue Router (Hash 模式)  | rumqttc (MQTT 登录)     |
| Naive UI                | tauri-plugin-store      |
|                         | tauri-plugin-android-fs |

---

## 📦 环境要求

请根据目标平台准备对应环境：

- **通用**：Rust ≥ 1.77.2、Node.js ≥ 18、npm/pnpm/yarn
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

### 5. Android 端开发运行

```bash
npx tauri android dev
```

### 6. Android 端构建

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

安装后可在应用安装目录中找到以上文件。

> 注：v1.3.2 之前的版本在 `NOTICE` 中仅列出直接依赖。自 v1.3.2 起，包含全部传递依赖及完整许可证文本。

---

## ⚠️ 免责声明

**HotDownloader 仅用于学习和研究目的。**

用户需自行承担使用本软件所带来的法律责任。请确保你下载的音乐文件拥有合法的使用权，遵守相关音乐平台的版权规定。本项目开发者不对任何侵权行为负责。
