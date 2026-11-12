# HotDownloader Core

此 crate 为 Tauri 客户端和独立服务提供共享下载能力。
任务契约、调度、平台请求、文件传输和收尾处理都在这里实现；各运行时负责接入自己的存储、文件系统与通知方式。

## 目录与公开入口

| 入口 | 职责 |
| --- | --- |
| `task::{contract, state, service, rules}` | 任务数据、状态持久化、命令编排与重试规则。 |
| `download::{engine, worker, context, config, link, ports, postprocess}` | 调度、下载执行、运行时端口与通用收尾素材。路径、传输和解密细节位于同一目录。 |
| `platforms::{qqmusic, kuwo}`、`platforms::Platform` | 平台查询、链接、登录凭据与歌词处理。 |
| `adapters::local` | JSON 任务仓库、普通文件打开与删除、LRC、封面文件和音频标签写入。 |
| `settings` | 设置快照的字段级合并、冲突判定与歌手分隔符解析。 |

`lib.rs` 只声明这些领域入口。Tauri 与独立服务通过上述公开路径接入核心；
Android SAF、系统通知和窗口功能由 Tauri 运行时适配器实现。

## 任务状态与调度

- `TaskRepository` 保存和读取完整任务快照。Tauri 通过 `TauriTaskIo` 接入 `data.json`，独立服务使用 `JsonTaskRepository`。
- `TaskStatus::Interrupted` 表示进程重启后等待用户恢复的任务。`resume_task` 从实际文件偏移继续，保留音质与错误重试次数；`retry_task` 用于处理下载错误。
- `task::identity` 按平台和歌曲 MID（缺失时回退到非零歌曲 ID）或媒体 MID 识别同一录音。未结束任务优先复用；已完成且文件仍在时，重名策略同样适用，即使标题、专辑或目标路径已经变化。运行时可通过 `TaskEnvironment::existing_download` 接入持久化下载台账。
- `TaskEventSink` 输出任务更新和删除事件。核心先持久化稳定状态，再发送事件；Tauri 事件名称由适配器定义。
- `DownloadEngine` 管理队列、并发、暂停、取消和资源回收。`DownloadTaskRunner` 执行下载并报告异常。
- `download::queue::QueueControl` 在下载并发之间共享 QQ 取链预算，请求串行且开始时间至少间隔 1 秒。连续 3 首不同歌曲取链异常，或接口明确返回限流/验证码提示时暂停派发；同曲的音质候选只计一次，成功取链清零。临时网络错误重试耗尽后直接报告该任务失败，不继续遍历低音质。
- 队列控制与按 ID/状态批量操作由 `TaskService` 执行；按状态选择覆盖完整任务记录。暂停的等待任务不占用下载并发槽。全局恢复保留此前手动暂停的决定；用户可重新选择音质重试同一任务，重置重试预算并保留旧品质片段。
- `DownloadConfig` 解析设置并校验下载目录。运行时通过 `DownloadConfigProvider` 提供设置与默认目录，worker 启动时获取配置快照。

## 下载流程

- `download::worker` 完成目标选择、链接获取与刷新、文件打开、暂停与取消处理、HTTP 流写入及收尾。
- `download::filename` 与 `download::path` 统一生成、截断和重命名目标。文件名单组件同时限制为最多 255 个 UTF-8 字节和 UTF-16 单元，Windows 还根据绝对目录计算 259 单元的完整路径预算；预留歌词、封面的扩展名长度。过长名称使用稳定标识和 `...` 结尾，重名序号继续计入预算；启用标签写入时保留完整歌曲信息。无法仅靠缩短文件名解决的目录会明确报错。
- `download::transfer` 提供下载专用 HTTP 客户端、Range 请求与响应校验、流读取、速度采样和断流重试。
- `download::decryption` 根据加密扩展名和 ekey，按绝对偏移处理 QMC 数据。
- `DownloadFileOpener` 返回实际文件偏移及 SAF URI。核心提供 `LocalDownloadFileOpener`；Android SAF 由 Tauri 适配器接入。
- `adapters::local::download_file` 处理普通文件的父目录创建、续传偏移校验和文件偏移重置。
- `DownloadPostprocessor` 在流传输后处理歌词、封面、音频标签和独立歌词、封面文件，返回已保存的附属文件路径，供删除下载文件时一并清理。`download::postprocess` 获取通用素材并统一文件命名，`adapters::local::postprocess` 提供普通文件实现。`downloadCover` 开关默认关闭，独立封面与音频标签共用 `coverQuality` 设置。
- 歌词、封面各最多尝试 3 次，间隔 0.5、1 秒；接口明确返回无歌词时允许没有歌词。获取或保存失败通过 `Result` 返回，worker 保留音频并记录失败，收尾成功后才发送完成事件。标签在同目录临时副本中写入，重新读取核对标题、歌手、专辑、歌词和封面后替换原文件；收尾失败的重试保持原音质。
- QQ 歌词解析接受 QRC XML、解密后的普通 LRC 及明确的纯音乐说明。成功响应的空歌词不触发重试，封面与基础标签继续处理；开启独立 LRC 时，确认无歌词的响应写入“暂无歌词”，明确的纯音乐说明保留在歌词中。缺失字段、未知响应结构、网络及解密异常仍按错误处理。
- `DownloadProgressSink` 接收下载进度、收尾结果和任务状态。Tauri 实现先更新 Rust 任务表，再发送兼容事件。
- `FileDeleter` 清理取消任务时的文件。普通路径使用 `LocalFileDeleter`，Android SAF URI 由 Tauri 适配器处理。

## 平台请求与登录

- `platforms` 包含 QQ 音乐与酷我的搜索、歌手、专辑、歌单、推荐、封面、歌词和解析逻辑。
- 需要歌手分隔符的函数直接接收 `&str`。桌面 IPC 入口与独立服务分别从各自设置中提供该参数。
- `PlatformDownloadLinkProvider` 请求 QQ 音乐与酷我下载链接，通过 `DownloadLinkProvider` 的错误分类处理临时网络故障并有限重试。
- QQ 下载凭据由 `QqCredentialSource` 提供。
  - 桌面端使用登录模块。
  - 独立服务可调用 `PlatformDownloadLinkProvider::from_credentials_file(path)`。
- `platforms::qqmusic::login` 实现 QQ 扫码、MQTT 会话、手动登录和凭据刷新，通过 `LoginCredentialStore` 接入 Tauri Store 或普通 JSON 文件。Tauri 下载适配器调用 `download_auth` 校验凭据并尝试刷新；刷新失败时继续使用旧凭据，并由 Tauri 提示用户。

## 运行时接入

### Tauri 桌面端与 Android 端

- `TauriTaskRunner` 组装平台端口后调用共享 worker。
- Android SAF 文件打开与标签回写由 Tauri 适配器实现。
- `CompletionNotifier` 将下载完成提示交给桌面或 Android 的系统通知适配器。

### 独立服务

- 普通文件下载、平台查询、歌词和收尾处理可在独立 Rust 进程中复用。
- 独立服务使用 JSON 任务仓库和普通文件系统实现，通过任务事件向在线页面展示进度与结果。
- 独立服务的启动、持久化与更新步骤见[独立服务说明](../hotdownloader-server/README.md)。

## QQ 凭据文件

独立服务的 QQ 凭据文件是 JSON 对象：

- `loginUin`、`authst`：当前登录凭据。
- `refreshToken`、`refreshKey`：自动刷新所需字段。
- `accessToken`、`openid`、`loginResponseData`：登录时保存的附加字段。

- 文件不存在时，QQ 平台请求按匿名方式访问。
- 读取现有文件时，核心会校验 JSON 格式。
- 刷新凭据后，核心更新对应字段并保留文件中的其他设置。

凭据文件存放在持久化目录，应限制访问权限。
Linux 上刷新写入使用同目录临时文件和原子替换。
