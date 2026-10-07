# 合成音频测试样本

`silence.mp3` 和 `silence.flac` 均由 FFmpeg 的 `anullsrc` 生成，输入为 44.1 kHz、单声道、0.12 秒静音，不含真实歌曲或账号数据。

测试将样本复制到临时目录，再写入虚构的标题和歌手。MP3 使用 ID3v2.4 的 NUL 分隔多值 TPE1，FLAC 使用重复的 Vorbis `ARTIST` 字段；写入后先验证确实读出两个歌手值，再验证服务端索引和匹配。

日常 Rust 测试直接读取已保存的样本，无需 FFmpeg。需要重新生成时，使用本机已有 FFmpeg，在仓库根目录执行：

```bash
node scripts/generate-monitor-audio-fixtures.mjs
```

默认从 PATH 查找 FFmpeg，也可以用 `FFMPEG_PATH` 指定可执行文件。脚本打印实际版本，不安装工具或下载音频。
