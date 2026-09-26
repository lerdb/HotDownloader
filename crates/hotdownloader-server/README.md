# HotDownloader 独立服务

此 crate 组装共享 Rust 核心，独立持有任务队列、下载文件、QQ 凭据和进度广播。浏览器重新连接时可获取当前任务快照并继续查看进度。

## Docker/Web 部署

在仓库根目录创建 `.env`，设置至少 16 个字符的随机访问令牌：

```dotenv
HOTDOWNLOADER_TOKEN=请替换为随机生成的长令牌
```

从仓库根目录启动服务：

```bash
docker compose up --build -d
```

浏览器访问 `http://服务器地址:8787`，输入 `.env` 中的访问令牌。Compose 将容器的 `/data` 挂载到 `hotdownloader-data` 卷，用于保存设置、QQ 凭据、任务记录和下载文件。下载任务由服务进程持续执行；重新打开网页即可查看进度。页面顶部显示服务连接状态和最近响应时间。

服务进程重启后，先前未完成的任务显示在“已中断”标签中，点击恢复后重新入队。公网访问请配置 HTTPS 反向代理，保护浏览器与服务之间的访问令牌。当前部署使用单个访问令牌，适合单管理员使用。

## 本机运行与配置

从仓库根目录运行：

```bash
cargo run --manifest-path crates/hotdownloader-server/Cargo.toml
```

| 环境变量 | 用途与默认值 |
| --- | --- |
| `HOTDOWNLOADER_DATA_DIR` | 数据目录，默认 `./data`。 |
| `HOTDOWNLOADER_BIND` | 监听地址，默认 `127.0.0.1:8787`。 |
| `HOTDOWNLOADER_TOKEN` | 对外监听时设置至少 16 个字符的访问令牌。`/api` 请求使用 `Authorization: Bearer <token>`。 |
| `HOTDOWNLOADER_WEB_DIR` | 前端构建产物目录，默认 `./dist`。 |
| `HOTDOWNLOADER_DOWNLOAD_DIR` | 下载文件的绝对目录，默认数据目录下的 `downloads`。 |
| `HOTDOWNLOADER_LOG_LEVEL` | 日志级别，可设为 `off`、`error`、`warn`、`info`、`debug` 或 `trace`，默认 `info`。 |

数据目录下的 `settings.json`、`qq-credentials.json` 和 `tasks.json` 分别用于下载设置、QQ 登录凭据和任务记录。首次启动使用默认设置和空任务列表。请持久化整个数据目录并限制其访问权限，妥善保护 QQ 凭据和访问令牌。

## 服务接口

任务 API 使用共享核心的 `camelCase` JSON 契约：

- `GET /api/tasks`：当前完整任务快照。
- `POST /api/tasks`：`CreateTaskRequest`；成功返回 `CreateTaskResult`。
- `POST /api/tasks/{id}/pause|resume|retry|cancel|remove`：任务控制；取消和删除可传 `{"deleteFile":true}`。
- `POST /api/tasks/remove`：批量删除，传 `{"taskIds":["..."],"deleteFile":false}`。
- `GET /api/events`：SSE，先发任务与设置快照，再推送 `task-updated`、`task-removed`、`settings-updated` 和心跳；连接恢复时由快照同步视图。
- `GET /api/settings`、`PATCH /api/settings`、`GET /api/settings/default-download-dir`：设置快照、字段级合并更新和服务器下载目录查询。PATCH 提交 `changes` 与各字段的 `expected` 原值；同字段冲突返回 HTTP 409 和最新快照。
- `GET /healthz`：服务健康检查，HTTP 服务可响应时返回 HTTP 200 与 `{"status":"ok"}`。
- `POST /api/music/{action}`：歌曲、歌单、专辑、歌手、热搜、建议、封面和歌词查询；参数及结果沿用前端 API 契约。
- `GET /api/login/status`、`POST /api/login/qr`、`GET /api/login/qr/{id}`、`POST /api/login/manual`、`POST /api/login/logout`：QQ 登录会话和凭据操作。

静态前端资源由同一进程提供。Web 端输入访问令牌后，通过 HTTP 操作任务并通过 SSE 接收快照和进度；服务进程管理任务生命周期。进程重启后，此前未完成的任务进入 `interrupted` 状态，等待用户手动恢复。

## 运行状态与日志

Compose 已配置 `/healthz` 健康检查。查看容器状态与服务日志：

```bash
docker compose ps
docker compose logs -f hotdownloader
```

日志以单行 JSON 写入标准错误输出，包含时间、级别、模块和消息。任务进度与结果也可在网页任务列表中查看。

## 本机前端联调

本机联调时分别运行 `cargo run --manifest-path crates/hotdownloader-server/Cargo.toml` 和 `npm run dev`。Vite 将浏览器的 `/api` 请求转发到本机 `8787` 端口；前端生产构建由同一 Rust 进程提供。
