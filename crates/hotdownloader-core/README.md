# HotDownloader Core

这个 crate 为 Tauri 客户端与独立服务提供共享下载能力，包含任务契约、音质规则、状态仓库、下载调度器与普通文件系统实现。

- `TaskRepository` 保存和读取完整任务快照；Tauri 客户端通过 `TauriTaskIo` 接到现有 `data.json`，独立进程可使用 `JsonTaskRepository`。
- `TaskStatus::Interrupted` 表示进程重启后等待用户恢复的任务。`resume_task` 恢复时读取实际文件偏移，保留原有音质与错误重试次数；`retry_task` 处理下载错误。
- `TaskEventSink` 输出完整任务更新与删除事件。核心按先持久化稳定状态、再发送事件的顺序运行；Tauri 事件名称由适配器定义。
- `FileDeleter` 删除任务文件；普通路径使用 `LocalFileDeleter`，Android SAF URI 由 Tauri 适配器处理。
- `CompletionNotifier` 将下载完成提示交给运行时。桌面和 Android 使用系统通知适配器，独立服务通过任务事件向在线页面展示进度与结果。
- `DownloadEngine` 负责队列、并发、暂停、取消和资源回收；`DownloadTaskRunner` 执行具体下载并报告异常。Tauri 客户端由 `TauriTaskRunner` 组装平台端口后调用共享核心 worker。
- `DownloadConfig` 统一解析设置、校验下载目录，并由 `DownloadConfigProvider` 接收运行时提供的设置和默认目录。worker 在启动时获取配置快照。
- `PlatformDownloadLinkProvider` 在核心中实现 QQ 音乐与酷我链接请求，继续使用 `DownloadLinkProvider` 的临时网络错误分类和有限重试。QQ 凭据从 `QqCredentialSource` 获取；桌面端沿用原登录模块，独立进程可以调用 `PlatformDownloadLinkProvider::from_credentials_file(path)`。
- `local_download_file` 负责普通文件的父目录创建、续传偏移校验和文件偏移重置。Android SAF 文件访问由 Tauri 适配器处理。
- `DownloadProgressSink` 统一接收下载进度、收尾结果和任务状态；Tauri 实现先更新 Rust 任务表，再发送现有兼容事件。取消任务时由 `FileDeleter` 清理文件。
- `http_transfer` 持有下载专用 HTTP 客户端、Range 请求与响应校验、流读取、速度采样和断流重试。`decryption` 根据加密扩展名及 ekey 按绝对偏移处理 QMC 数据；两者均可在独立 Rust 进程中使用。
- `DownloadPostprocessor` 在流传输完成后处理歌词、封面、音频标签及独立 LRC 文件。`postprocess` 提供普通文件实现；Tauri 桥接 Android SAF 文件访问。
- `download_worker` 现包含完整任务循环：确定目标、获取或刷新链接、打开文件、处理暂停与取消、写入 HTTP 流和执行收尾。`DownloadFileOpener` 返回实际偏移及 SAF URI；核心提供 `LocalDownloadFileOpener`，Tauri 提供 SAF 适配器。
- `platforms` 包含 QQ 音乐与酷我的搜索、歌手、专辑、歌单、推荐、封面、歌词及解析逻辑。需歌手分隔符的函数直接接收 `&str`；桌面 IPC 入口与独立服务分别从各自设置中提供该参数。

独立进程的 QQ 凭据文件是一个 JSON 对象，可包含 `loginUin` 和 `authst`；自动刷新还使用 `refreshToken`、`refreshKey`，以及登录时得到的 `accessToken`、`openid`、`loginResponseData`。服务支持匿名访问，并在读取凭据时校验 JSON 格式。刷新后更新凭据字段并保留文件中的其他设置。该文件存放在持久化目录，并应限制访问权限；Linux 上刷新写入使用同目录临时文件和原子替换。

QQ 扫码、MQTT 会话、手动登录与凭据刷新位于 `qq_login`，通过 `LoginCredentialStore` 接收 Tauri Store 或普通 JSON 文件。Android SAF 文件打开和标签回写由 Tauri 适配器实现。普通文件下载、平台查询、歌词及收尾可由独立 Rust 进程调用；运行方式见[独立服务说明](../hotdownloader-server/README.md)。
