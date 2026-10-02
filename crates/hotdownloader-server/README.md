# HotDownloader 独立服务

此 crate 组装共享 Rust 核心，管理任务队列、下载文件、QQ 凭据和进度广播。
浏览器重新连接时可获取当前任务快照，继续查看进度。

## Docker/Web 部署

### 镜像

- 地址：`ghcr.io/lerdb/hotdownloader`
- 架构：`linux/amd64`
- `latest`：随 `main` 分支发布更新
- 版本标签：`vX.Y.Z`、`X.Y.Z`、`X.Y` 和 `sha-<短提交号>`

### 启动容器

创建 `.env`，配置用户名和密码，以及容器内下载目录：

```dotenv
AUTH_USERNAME=admin
AUTH_PASSWORD=请替换为密码
HOTDOWNLOADER_DOWNLOAD_DIR=/downloads
```

拉取镜像并启动服务：

```bash
docker pull ghcr.io/lerdb/hotdownloader:latest
docker run -d --name hotdownloader --restart unless-stopped \
  --env-file .env -p 127.0.0.1:8787:8787 \
  -v hotdownloader-data:/data \
  -v hotdownloader-downloads:/downloads \
  ghcr.io/lerdb/hotdownloader:latest
```

在服务器本机访问 `http://127.0.0.1:8787`，输入 `.env` 中的用户名和密码。
也可只设置 `HOTDOWNLOADER_TOKEN`（至少 16 个字符），继续使用令牌登录。
若同时设置账号密码和令牌，服务只接受账号密码。
Docker 仅将端口绑定到宿主机回环地址，容器内服务仍监听 `0.0.0.0:8787`，以便端口映射正常工作。

从其他设备访问时，请在宿主机配置 HTTPS 反向代理，将请求转发到 `127.0.0.1:8787`，并通过 HTTPS 域名打开网页。
不要将访问凭据通过公网明文 HTTP 传输。

下载任务由服务进程持续执行。重新打开网页即可查看进度。
页面顶部显示服务连接状态和最近响应时间。

### 数据与下载目录

容器使用两个独立的卷：

- `hotdownloader-data` 挂载到 `/data`，保存设置、QQ 凭据和任务记录。
- `hotdownloader-downloads` 挂载到 `/downloads`，保存新下载的文件。

如需使用宿主机目录，将第二个 `-v` 改为：

```text
-v /srv/music:/downloads
```

`HOTDOWNLOADER_DOWNLOAD_DIR` 应与容器内的下载目录挂载目标一致。

从旧版 Docker 部署升级时，已有下载文件仍位于 `hotdownloader-data` 卷中的 `/data/downloads`，已有任务记录也继续引用该路径。
新任务将写入独立的下载卷。请保留原数据卷，以便访问历史文件。

### 使用 Compose

使用仓库根目录的 [`compose.yaml`](../../compose.yaml)。
在该目录保存 `.env` 后运行：

```bash
docker compose pull
docker compose up -d
```

Compose 默认使用独立的 `hotdownloader-downloads` 卷。
如需改用宿主机目录，在 `.env` 中加入：

```dotenv
HOTDOWNLOADER_DOWNLOAD_MOUNT=/srv/music
```

`HOTDOWNLOADER_DOWNLOAD_MOUNT` 控制挂载源。
如需更改容器内路径，再将 `HOTDOWNLOADER_DOWNLOAD_DIR` 设为绝对路径，Compose 会用它作为挂载目标。

### 更新镜像

Compose 部署：

```bash
docker compose pull
docker compose up -d
```

`docker run` 部署：拉取镜像，删除旧容器，再运行上面的启动命令。

```bash
docker pull ghcr.io/lerdb/hotdownloader:latest
docker rm -f hotdownloader
```

命名卷保留已有数据。

### 任务恢复与访问安全

服务进程重启后，手动创建的未完成任务显示在“已中断”标签中，点击恢复后重新入队。
歌单监控创建的未完成任务会自动恢复；用户主动暂停的监控任务保留暂停决定，可在任务页手动恢复。

远程访问请使用上述 HTTPS 反向代理，保护浏览器与服务之间的访问凭据。
当前部署使用一组账号密码或单个访问令牌，适合单管理员使用。

## 本机运行与配置

从仓库根目录运行：

```bash
cargo run --manifest-path crates/hotdownloader-server/Cargo.toml
```

| 环境变量 | 用途与默认值 |
| --- | --- |
| `HOTDOWNLOADER_DATA_DIR` | 数据目录，默认 `./data`。 |
| `HOTDOWNLOADER_BIND` | 监听地址，默认 `127.0.0.1:8787`。 |
| `AUTH_USERNAME`、`AUTH_PASSWORD` | 对外监听时可同时设置，启用账号密码认证。`/api` 请求使用 HTTP Basic 认证；两者优先于令牌。不允许只设置其中一个，用户名不能包含冒号。 |
| `HOTDOWNLOADER_TOKEN` | 未设置账号密码时，对外监听需设置至少 16 个字符的访问令牌。`/api` 请求使用 `Authorization: Bearer <token>`。 |
| `HOTDOWNLOADER_WEB_DIR` | 前端构建产物目录，默认 `./dist`。 |
| `HOTDOWNLOADER_DOWNLOAD_DIR` | 下载文件的绝对目录。本机或直接使用镜像时默认是数据目录下的 `downloads`；仓库的 Compose 配置默认是 `/downloads`。 |
| `HOTDOWNLOADER_SCAN_DIRS` | 附加音乐库目录的 JSON 对象数组，默认 `[]`。每项包含容器内绝对 `path`、可选 `template` 和 `artistSeparator`，见下节。下载目录自动纳入。 |
| `HOTDOWNLOADER_LOG_LEVEL` | 日志级别，可设为 `off`、`error`、`warn`、`info`、`debug` 或 `trace`，默认 `info`。 |

数据目录包含：

- `settings.json`：下载设置
- `qq-credentials.json`：QQ 登录凭据
- `tasks.json`：任务记录
- `library.sqlite3`：增量文件索引、监控配置、歌单成员快照和以 MID 为键的处理台账

首次启动使用默认设置和空任务列表。
请持久化整个数据目录并限制其访问权限，妥善保护 QQ 凭据和访问令牌。

## 服务接口

服务接口沿用共享核心的 `camelCase` JSON 契约。

### 任务与事件

- `GET /api/tasks`：当前完整任务快照。
- `POST /api/tasks`：提交 `CreateTaskRequest`，成功后返回 `CreateTaskResult`。
- `POST /api/tasks/{id}/pause|resume|retry|cancel|remove`：控制单个任务。取消和删除可传 `{"deleteFile":true}`。
- `POST /api/tasks/remove`：批量删除，传 `{"taskIds":["..."],"deleteFile":false}`。
- `GET /api/events`：SSE。连接后先发送任务与设置快照，再推送任务更新、设置更新和心跳。重连时通过快照同步视图。

事件类型包括 `task-updated`、`task-removed` 和 `settings-updated`。

### 设置与健康检查

- `GET /api/settings`：设置快照。
- `PATCH /api/settings`：字段级合并更新。提交 `changes` 与各字段的 `expected` 原值；同字段冲突返回 HTTP 409 和最新快照。
- `GET /api/settings/default-download-dir`：服务器下载目录。
- `GET /healthz`：服务健康检查，HTTP 服务可响应时返回 HTTP 200 与 `{"status":"ok"}`。

### 音乐与登录

- `POST /api/music/{action}`：查询歌曲、歌单、专辑、歌手、热搜、建议、封面和歌词。参数及结果沿用前端 API 契约。
- QQ 登录状态与凭据操作：
  - `GET /api/login/status`
  - `POST /api/login/qr`
  - `GET /api/login/qr/{id}`
  - `POST /api/login/manual`
  - `POST /api/login/logout`

静态前端资源由同一进程提供。
Web 端输入访问令牌后，通过 HTTP 操作任务，通过 SSE 接收快照和进度。
服务进程管理任务生命周期；进程重启后，手动任务等待恢复，监控拥有的未完成任务自动恢复。

## 音乐库与歌单监控

Web 版打开「歌单 → 歌单监控与自动补齐」。添加 QQ 公开歌单（数字 ID）、个人歌单或「我喜欢」，选定自动下载音质和检查间隔后启用。个人歌单可通过「读取我的歌单」选择，保留目录 ID。需要在设置页保存可用的 QQ 登录凭据。

监控音质保存在服务端，浏览器默认音质仅用于新建表单预填；选择「每次询问」时必须在表单中另选固定音质。检查间隔为 5–10080 分钟，停用后仍可「立即检查」执行一次补齐。停用不取消已入队下载，任务页可以继续暂停、取消和恢复。

### 挂载已有音乐库

下载目录自动扫描，默认按下载设置中的命名模板和歌手分隔符解析。其他目录在 `.env` 中配置，例如：

```dotenv
HOTDOWNLOADER_SCAN_DIRS='[{"path":"/music/archive","template":"{song} - {artist}","artistSeparator":"、"},{"path":"/music/albums"}]'
```

在 `compose.yaml` 的 `volumes` 中增加对应只读挂载：

```yaml
- /srv/music/archive:/music/archive:ro
- /srv/music/albums:/music/albums:ro
```

`path` 是容器内绝对路径。未设置 `template` 时只读取音频标签；模板要求各包含一次 `{song}`、`{artist}`，可加入 `{album}`、`{quality}`，变量之间必须有分隔文字。歌手字符串按该目录的 `artistSeparator` 拆分，默认 `、`；多值音频歌手标签也会合并为集合。下载目录的命名模板若不足以解析标题和歌手，则只使用标签。符号链接不递归扫描。

扫描支持 MP3、FLAC、M4A/MP4、AAC、OGG、Opus、APE、WAV、AIFF 和 WavPack 的常见扩展名。首次读取标签，此后仅对路径、大小、修改时间或解析配置变化的文件重新读取。正在下载和未完成任务的目标文件不参与匹配。

匹配采用 Unicode NFKC、大小写和空白规范化后的标题及无序歌手集合，不比较音质、扩展名或专辑，保留 Live、Remix 等版本文字。标签优先，缺失字段才从文件名补充。标签和文件名冲突、多个候选或信息不完整会进入「待确认」，可逐曲关联、下载或忽略。无法读取标签的文件保留警告；库中存在完全无法识别的文件时，不可靠的缺失判断同样等待确认。

任一配置目录或子目录不可访问时，整轮索引更新和自动补齐跳过，保留上次完整索引。恢复挂载后会再次检查。索引不会修改已有音频文件。

### 补齐、台账与恢复

- 首次检查补齐当前缺失歌曲，之后只为新 MID 建立处理记录。相同 MID 在多个歌单中共用状态和任务；歌单移除歌曲不删除文件。
- 每轮最多派发 20 首，服务循环每 10 秒运行一次，下载并发继续受设置页的并发限制约束。
- 已关联、已下载、已忽略的决定独立于任务历史。移动或删除文件、清除下载任务都不触发自动重下。逐曲「清除决定并重新下载」会清除共享决定并保留已有文件，重名时另存一份。
- 无可用音质直接记录状态；下载失败或凭据失效最多自动重试 3 次，等待 5、15、60 分钟。下载失败复用原任务，耗尽后由用户重新处理。未监控的手动失败任务不由监控接管重试。
- 服务重启后继续未完成的分批补齐、定时检查和重试，并恢复监控拥有的中断下载。若任务记录被移除或无法确认派发是否成功，进入待确认。
- 监控音质在首次发现歌曲时记录到歌曲台账；调整监控音质影响后续新歌。手动重新下载使用当前监控音质。

请持久化 `/data`。`library.sqlite3` 无需独立数据库服务；备份时停止服务后复制数据目录，保留任务与台账的一致状态。数据库写入失败会停止新的自动派发，修复存储后重启服务。

### 监控 API

接口与现有 `/api` 共用访问认证：

- `GET /api/library`、`POST /api/library/scan`：索引状态及立即扫描。
- `GET /api/monitors`、`POST /api/monitors`：读取及新建监控。
- `PATCH /api/monitors/{id}`：修改名称、音质、间隔和启停状态；请求完整配置。
- `POST /api/monitors/{id}/check`：请求立即检查，在服务循环执行。
- `GET /api/monitors/{id}/songs`：逐曲状态、候选文件、任务 ID 和重试时间。
- `POST /api/library/songs/{mid}`：传 `{"action":"link","path":"/music/example.flac"}`、`{"action":"download"}`、`{"action":"ignore"}` 或 `{"action":"reset"}`。

监控配置示例：

```json
{"name":"示例歌单","source":"public","playlistId":"123456","dirid":"","quality":"320kmp3","intervalMinutes":60,"enabled":true}
```

`source` 可取 `public`、`created`、`liked`；个人歌单需要 `dirid`，「我喜欢」使用服务端当前 QQ 账号返回的目录。歌单读取失败或响应不完整时保留上次成员快照。

### 验证

```bash
cargo test --locked --manifest-path crates/hotdownloader-server/Cargo.toml
cargo clippy --locked --manifest-path crates/hotdownloader-server/Cargo.toml --all-targets -- -D warnings
npm run build
cargo build --locked --manifest-path crates/hotdownloader-server/Cargo.toml
npm run test:monitor-http
```

监控测试使用合成静音 WAV、虚构歌单和禁止联网的下载执行器，覆盖增量扫描、冲突、批次入队、跨歌单去重、处理台账、有限重试与重启恢复。
HTTP 冒烟测试需要 Node.js 24 或更高版本（内置 SQLite），会启动独立的临时服务并自动清理其合成数据。

## 运行状态与日志

Compose 已配置 `/healthz` 健康检查。查看 Compose 服务状态与日志：

```bash
docker compose ps
docker compose logs -f hotdownloader
```

使用 `docker run` 部署时查看容器与日志：

```bash
docker ps
docker logs -f hotdownloader
```

日志以单行 JSON 写入标准错误输出，包含时间、级别、模块和消息。
任务进度与结果也可在网页任务列表中查看。

## 本机前端联调

本机联调时，在两个终端分别运行服务与前端。

终端一：

```bash
cargo run --manifest-path crates/hotdownloader-server/Cargo.toml
```

终端二：

```bash
npm run dev
```

Vite 将浏览器的 `/api` 请求转发到本机 `8787` 端口。
前端生产构建由同一 Rust 进程提供。
