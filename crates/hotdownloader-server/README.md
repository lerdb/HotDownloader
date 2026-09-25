# HotDownloader 独立服务

此 crate 组装共享 Rust 核心，独立持有任务队列、下载文件、QQ 凭据和进度广播。浏览器重新连接时可获取当前任务快照并继续查看进度。

运行：`cargo run --manifest-path crates/hotdownloader-server/Cargo.toml`。数据目录由 `HOTDOWNLOADER_DATA_DIR` 指定，默认 `./data`；监听地址由 `HOTDOWNLOADER_BIND` 指定，默认 `127.0.0.1:8787`。对外监听时需设置至少 16 个字符的 `HOTDOWNLOADER_TOKEN`；所有 `/api` 请求均使用 `Authorization: Bearer <token>`。`HOTDOWNLOADER_WEB_DIR` 指向前端构建产物，默认 `./dist`。`HOTDOWNLOADER_DOWNLOAD_DIR` 可指定绝对路径，默认使用数据目录下的 `downloads`；服务端以此路径管理下载文件。

数据目录下的 `settings.json`、`qq-credentials.json` 和 `tasks.json` 分别用于下载设置、QQ 登录凭据和任务记录。首次启动使用默认设置和空任务列表。请将整个数据目录持久化并限制其访问权限，妥善保护 QQ 凭据和访问令牌。

任务 API 使用共享核心的 `camelCase` JSON 契约：

- `GET /api/tasks`：当前完整任务快照。
- `POST /api/tasks`：`CreateTaskRequest`；成功返回 `CreateTaskResult`。
- `POST /api/tasks/{id}/pause|resume|retry|cancel|remove`：任务控制；取消和删除可传 `{"deleteFile":true}`。
- `POST /api/tasks/remove`：批量删除，传 `{"taskIds":["..."],"deleteFile":false}`。
- `GET /api/events`：SSE，先发 `task-snapshot`，再发 `task-updated`、`task-removed`；连接恢复时可通过快照同步视图。
- `GET /api/settings`、`PUT /api/settings`、`GET /api/settings/default-download-dir`：设置读取、保存和服务器下载目录查询。
- `POST /api/music/{action}`：歌曲、歌单、专辑、歌手、热搜、建议、封面和歌词查询；参数及结果沿用前端 API 契约。
- `GET /api/login/status`、`POST /api/login/qr`、`GET /api/login/qr/{id}`、`POST /api/login/manual`、`POST /api/login/logout`：QQ 登录会话和凭据操作。

静态前端资源由同一进程提供。Web 端输入访问令牌后，通过 HTTP 操作任务并通过 SSE 接收快照和进度；服务进程独立管理任务生命周期。进程重启后，此前进行中的任务可通过重试接口重新入队。当前部署使用单个访问令牌，适合单管理员使用。公网访问请配置 HTTPS 反向代理以保护访问令牌。

本机联调时分别运行 `cargo run --manifest-path crates/hotdownloader-server/Cargo.toml` 和 `npm run dev`。Vite 将浏览器的 `/api` 请求转发到本机 `8787` 端口；前端生产构建由同一 Rust 进程提供。
