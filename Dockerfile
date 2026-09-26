# 前端使用仓库的 npm 锁文件，保证容器构建与本地 npm 依赖一致。
FROM node:22-bookworm-slim AS web-builder
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci
COPY index.html vite.config.ts tsconfig*.json ./
COPY public ./public
COPY src ./src
COPY src-tauri/tauri.conf.json ./src-tauri/tauri.conf.json
RUN npm run build

# Rust 构建需要完整的共享核心和加密库源码。锁文件属于服务 crate。
FROM rust:1-bookworm AS rust-builder
WORKDIR /build
COPY crates ./crates
COPY libs/um_crypto ./libs/um_crypto
RUN cargo build --release --locked --manifest-path crates/hotdownloader-server/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=rust-builder /build/crates/hotdownloader-server/target/release/hotdownloader-server /app/hotdownloader-server
COPY --from=web-builder /build/dist /app/dist

# 任务、凭据、设置和下载文件都放在持久化目录，网页连接中断不影响任务。
ENV HOTDOWNLOADER_BIND=0.0.0.0:8787 \
    HOTDOWNLOADER_DATA_DIR=/data \
    HOTDOWNLOADER_WEB_DIR=/app/dist
VOLUME /data
EXPOSE 8787
CMD ["/app/hotdownloader-server"]
