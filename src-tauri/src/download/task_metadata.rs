//! Android SAF 音频文件的临时文件桥接。标签写入规则位于共享核心。

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use hotdownloader_core::adapters::local::postprocess::write_audio_metadata;
use hotdownloader_core::download::context::TaskContext;
use hotdownloader_core::download::postprocess::PostprocessAssets;
use tauri::{AppHandle, Manager};
use tauri_plugin_android_fs::{AndroidFsExt, FileAccessMode, FsUri};

/// 将 SAF 文件复制到应用缓存目录，在普通路径上调用核心标签处理，然后写回。
/// 任一阶段失败均交给 worker，任务不会被记为成功。
pub(crate) fn write_saf_metadata(
    app: &AppHandle,
    context: &TaskContext,
    audio_path: &str,
    saf_file_uri: Option<&str>,
    assets: &PostprocessAssets,
) -> Result<(), String> {
    write_saf_metadata_inner(app, context, audio_path, saf_file_uri, assets)
}

fn write_saf_metadata_inner(
    app: &AppHandle,
    context: &TaskContext,
    audio_path: &str,
    saf_file_uri: Option<&str>,
    assets: &PostprocessAssets,
) -> Result<(), String> {
    let uri = saf_file_uri.ok_or("SAF 文件 URI 缺失")?;
    let fs_uri = FsUri::from_uri(uri.to_string());
    let api = app.android_fs();

    // lofty 需要普通文件路径和正确扩展名。SAF 只承担文件访问，不再拥有标签业务规则。
    let mut source = api
        .open_file(&fs_uri, FileAccessMode::Read)
        .map_err(|error| format!("打开 SAF 文件读取失败: {error}"))?;
    let mut audio_bytes = Vec::new();
    source
        .read_to_end(&mut audio_bytes)
        .map_err(|error| format!("读取 SAF 文件失败: {error}"))?;

    let extension = Path::new(audio_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("tmp");
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("获取应用缓存目录失败: {error}"))?;
    std::fs::create_dir_all(&cache_dir)
        .map_err(|error| format!("创建应用缓存目录失败: {error}"))?;
    let temp_path = cache_dir.join(format!("{}.{}", rand::random::<u64>(), extension));
    std::fs::write(&temp_path, audio_bytes)
        .map_err(|error| format!("写入临时文件失败: {error}"))?;

    // 即使标签写入或 SAF 回写失败，也清理本次创建的缓存文件。
    let write_result = write_and_copy_back(&temp_path, &fs_uri, app, context, assets);
    if let Err(error) = std::fs::remove_file(&temp_path) {
        log::warn!("删除临时音频文件失败 {}: {error}", temp_path.display());
    }
    write_result
}

fn write_and_copy_back(
    temp_path: &PathBuf,
    fs_uri: &FsUri,
    app: &AppHandle,
    context: &TaskContext,
    assets: &PostprocessAssets,
) -> Result<(), String> {
    write_audio_metadata(temp_path, context, assets)?;
    let data = std::fs::read(temp_path).map_err(|error| format!("读取临时文件失败: {error}"))?;
    let api = app.android_fs();
    let mut target = api
        .open_file(fs_uri, FileAccessMode::ReadWrite)
        .map_err(|error| format!("打开 SAF 文件写入失败: {error}"))?;

    // 先截断再从头写入，避免新标签较短时尾部残留旧数据。
    target
        .set_len(0)
        .map_err(|error| format!("清空 SAF 文件失败: {error}"))?;
    target
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|error| format!("SAF 文件 seek 失败: {error}"))?;
    target
        .write_all(&data)
        .map_err(|error| format!("写入 SAF 文件失败: {error}"))
}
