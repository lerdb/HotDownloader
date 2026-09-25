use umc_qmc::QMCv2Cipher;

/// 流式 QMC 解密上下文。密钥无效时沿用原有行为，继续写入未解密数据并记录错误。
pub struct DecryptContext {
    cipher: Option<QMCv2Cipher>,
    pub enabled: bool,
}

/// 解密只适用于加密扩展名且平台提供了 ekey 的歌曲。
pub fn init_decryption(key: &str, enabled: bool) -> DecryptContext {
    if !enabled || key.is_empty() {
        return DecryptContext {
            cipher: None,
            enabled: false,
        };
    }

    match QMCv2Cipher::new_from_ekey(key.as_bytes()) {
        Ok(cipher) => {
            log::info!("解密器初始化成功");
            DecryptContext {
                cipher: Some(cipher),
                enabled: true,
            }
        }
        Err(error) => {
            log::error!("解密器初始化失败: {}", error);
            DecryptContext {
                cipher: None,
                enabled: false,
            }
        }
    }
}

/// 根据文件名选择是否解密，避免不同 worker 重复实现扩展名判断。
pub fn init_for_filename(filename: &str, key: &str) -> DecryptContext {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let encrypted = matches!(extension, "mgg" | "mflac");
    init_decryption(key, encrypted && !key.is_empty())
}

/// 使用文件的绝对字节偏移原地解密，续传后仍能从正确位置继续。
pub fn decrypt_chunk(ctx: &DecryptContext, data: &mut [u8], offset: u64) {
    if !ctx.enabled {
        return;
    }
    if let Some(cipher) = &ctx.cipher {
        cipher.decrypt(data, offset as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::init_for_filename;

    #[test]
    fn regular_audio_never_uses_qmc_cipher() {
        assert!(!init_for_filename("song.mp3", "bad-key").enabled);
        assert!(!init_for_filename("song.mflac", "").enabled);
    }
}
