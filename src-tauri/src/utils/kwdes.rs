//! 酷我音乐自定义 DES（kwDES）加解密模块。
//!
//! 酷我音乐客户端使用一种非标准的 DES 变体（`kwDES`）来加解密：
//! - 使用 LSB-first 位序，与标准 DES 相反。
//! - S-box 采用字节对齐分组。
//! - 因此标准 `des` crate 无法兼容此算法。
//!
//! # 加密与解密模式
//! - `des_crypt(msg, key, CryptMode::Encrypt)`: 加密
//! - `des_crypt(msg, key, CryptMode::Decrypt)`: 解密（DES 是对称算法，使用反转子密钥）
//!
//! `base64_encrypt` / `base64_decrypt` 是酷我实际使用的封装：
//! 先 kwDES 加解密，再做 Base64 编/解码。

use base64::{engine::general_purpose::STANDARD, Engine as _};

// ============================================================================
// 常量表
// ============================================================================

/// E 扩展置换表（48 位输出，但表中保留 -1 占位，共 64 长度）
const EXPANSION: [i16; 64] = [
    31, 0, 1, 2, 3, 4, -1, -1, 3, 4, 5, 6, 7, 8, -1, -1, 7, 8, 9, 10, 11, 12, -1, -1, 11, 12, 13,
    14, 15, 16, -1, -1, 15, 16, 17, 18, 19, 20, -1, -1, 19, 20, 21, 22, 23, 24, -1, -1, 23, 24, 25,
    26, 27, 28, -1, -1, 27, 28, 29, 30, 31, 30, -1, -1,
];

/// 初始置换 IP
const INITIAL_PERMUTATION: [i16; 64] = [
    57, 49, 41, 33, 25, 17, 9, 1, 59, 51, 43, 35, 27, 19, 11, 3, 61, 53, 45, 37, 29, 21, 13, 5, 63,
    55, 47, 39, 31, 23, 15, 7, 56, 48, 40, 32, 24, 16, 8, 0, 58, 50, 42, 34, 26, 18, 10, 2, 60, 52,
    44, 36, 28, 20, 12, 4, 62, 54, 46, 38, 30, 22, 14, 6,
];

/// 末置换 IP-1
const INVERSE_PERMUTATION: [i16; 64] = [
    39, 7, 47, 15, 55, 23, 63, 31, 38, 6, 46, 14, 54, 22, 62, 30, 37, 5, 45, 13, 53, 21, 61, 29,
    36, 4, 44, 12, 52, 20, 60, 28, 35, 3, 43, 11, 51, 19, 59, 27, 34, 2, 42, 10, 50, 18, 58, 26,
    33, 1, 41, 9, 49, 17, 57, 25, 32, 0, 40, 8, 48, 16, 56, 24,
];

/// 每轮左移位数
const SHIFT_SCHEDULE: [u8; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

/// 左移掩码辅助
const SHIFT_MASKS: [u64; 3] = [0, 0x100001, 0x300003];

/// P 置换
const PERMUTATION: [i16; 32] = [
    15, 6, 19, 20, 28, 11, 27, 16, 0, 14, 22, 25, 4, 17, 30, 9, 1, 7, 23, 13, 31, 26, 2, 8, 18, 12,
    29, 5, 21, 10, 3, 24,
];

/// PC-1 置换
const PC1: [i16; 56] = [
    56, 48, 40, 32, 24, 16, 8, 0, 57, 49, 41, 33, 25, 17, 9, 1, 58, 50, 42, 34, 26, 18, 10, 2, 59,
    51, 43, 35, 62, 54, 46, 38, 30, 22, 14, 6, 61, 53, 45, 37, 29, 21, 13, 5, 60, 52, 44, 36, 28,
    20, 12, 4, 27, 19, 11, 3,
];

/// PC-2 置换
const PC2: [i16; 64] = [
    13, 16, 10, 23, 0, 4, -1, -1, 2, 27, 14, 5, 20, 9, -1, -1, 22, 18, 11, 3, 25, 7, -1, -1, 15, 6,
    26, 19, 12, 1, -1, -1, 40, 51, 30, 36, 46, 54, -1, -1, 29, 39, 50, 44, 32, 47, -1, -1, 43, 48,
    38, 55, 33, 52, -1, -1, 45, 41, 49, 35, 28, 31, -1, -1,
];

/// S-box 矩阵（8 组 × 64 字节）
const S_BOXES: [[u8; 64]; 8] = [
    [
        14, 4, 3, 15, 2, 13, 5, 3, 13, 14, 6, 9, 11, 2, 0, 5, 4, 1, 10, 12, 15, 6, 9, 10, 1, 8, 12,
        7, 8, 11, 7, 0, 0, 15, 10, 5, 14, 4, 9, 10, 7, 8, 12, 3, 13, 1, 3, 6, 15, 12, 6, 11, 2, 9,
        5, 0, 4, 2, 11, 14, 1, 7, 8, 13,
    ],
    [
        15, 0, 9, 5, 6, 10, 12, 9, 8, 7, 2, 12, 3, 13, 5, 2, 1, 14, 7, 8, 11, 4, 0, 3, 14, 11, 13,
        6, 4, 1, 10, 15, 3, 13, 12, 11, 15, 3, 6, 0, 4, 10, 1, 7, 8, 4, 11, 14, 13, 8, 0, 6, 2, 15,
        9, 5, 7, 1, 10, 12, 14, 2, 5, 9,
    ],
    [
        10, 13, 1, 11, 6, 8, 11, 5, 9, 4, 12, 2, 15, 3, 2, 14, 0, 6, 13, 1, 3, 15, 4, 10, 14, 9, 7,
        12, 5, 0, 8, 7, 13, 1, 2, 4, 3, 6, 12, 11, 0, 13, 5, 14, 6, 8, 15, 2, 7, 10, 8, 15, 4, 9,
        11, 5, 9, 0, 14, 3, 10, 7, 1, 12,
    ],
    [
        7, 10, 1, 15, 0, 12, 11, 5, 14, 9, 8, 3, 9, 7, 4, 8, 13, 6, 2, 1, 6, 11, 12, 2, 3, 0, 5,
        14, 10, 13, 15, 4, 13, 3, 4, 9, 6, 10, 1, 12, 11, 0, 2, 5, 0, 13, 14, 2, 8, 15, 7, 4, 15,
        1, 10, 7, 5, 6, 12, 11, 3, 8, 9, 14,
    ],
    [
        2, 4, 8, 15, 7, 10, 13, 6, 4, 1, 3, 12, 11, 7, 14, 0, 12, 2, 5, 9, 10, 13, 0, 3, 1, 11, 15,
        5, 6, 8, 9, 14, 14, 11, 5, 6, 4, 1, 3, 10, 2, 12, 15, 0, 13, 2, 8, 5, 11, 8, 0, 15, 7, 14,
        9, 4, 12, 7, 10, 9, 1, 13, 6, 3,
    ],
    [
        12, 9, 0, 7, 9, 2, 14, 1, 10, 15, 3, 4, 6, 12, 5, 11, 1, 14, 13, 0, 2, 8, 7, 13, 15, 5, 4,
        10, 8, 3, 11, 6, 10, 4, 6, 11, 7, 9, 0, 6, 4, 2, 13, 1, 9, 15, 3, 8, 15, 3, 1, 14, 12, 5,
        11, 0, 2, 12, 14, 7, 5, 10, 8, 13,
    ],
    [
        4, 1, 3, 10, 15, 12, 5, 0, 2, 11, 9, 6, 8, 7, 6, 9, 11, 4, 12, 15, 0, 3, 10, 5, 14, 13, 7,
        8, 13, 14, 1, 2, 13, 6, 14, 9, 4, 1, 2, 14, 11, 13, 5, 0, 1, 10, 8, 3, 0, 11, 3, 5, 9, 4,
        15, 2, 7, 8, 12, 15, 10, 7, 6, 12,
    ],
    [
        13, 7, 10, 0, 6, 9, 5, 15, 8, 4, 3, 10, 11, 14, 12, 5, 2, 11, 9, 6, 15, 12, 0, 3, 4, 1, 14,
        13, 1, 2, 7, 8, 1, 2, 12, 15, 10, 4, 0, 3, 13, 14, 6, 9, 7, 8, 9, 6, 15, 1, 5, 12, 3, 10,
        14, 5, 8, 7, 11, 0, 4, 13, 2, 11,
    ],
];

/// 默认密钥
pub const DEFAULT_KEY: &[u8; 8] = b"ylzsxkwm";

// ============================================================================
// 基础变换
// ============================================================================

/// 加解密模式
pub enum CryptMode {
    #[allow(dead_code)]
    Encrypt,
    Decrypt,
}

/// 按位变换：根据映射表重新排列输入整数的位。
///
/// # 参数
/// - `data`: 输入整数（位表示）
/// - `mapper`: 映射表，长度等于输出位数，每个元素表示输出位在输入中的位置索引。
///             若元素值为 -1，则该输出位固定为 0。
///
/// # 返回
/// 置换后的整数（输出位数与 mapper 长度一致）
#[inline]
fn permute(data: u64, mapper: &[i16]) -> u64 {
    let mut ret = 0u64;
    for (i, &v) in mapper.iter().enumerate() {
        if v < 0 {
            continue;
        }
        // 检查输入位 v 是否为 1，若是则设置输出位 i 为 1
        if (data & (1u64 << v)) != 0 {
            ret |= 1u64 << i;
        }
    }
    ret
}

/// 根据 64 位密钥生成 16 个 48 位子密钥。
///
/// # 参数
/// - `key_int`: 64 位密钥整数
/// - `round_keys`: 输出数组，长度 16，存储生成的子密钥
#[inline]
fn derive_keys(key_int: u64, round_keys: &mut [u64; 16]) {
    // PC1 置换：将 64 位密钥压缩为 56 位
    let mut key56 = permute(key_int, &PC1);

    // 循环 16 轮，每轮根据 SHIFT_SCHEDULE 对左右两部分分别循环左移
    for i in 0..16 {
        let shift = SHIFT_SCHEDULE[i];
        let mask = SHIFT_MASKS[shift as usize];
        // 28 位循环左移（同时作用于左右两部分）
        key56 = ((key56 & mask) << (28 - shift as u32)) | ((key56 & !mask) >> shift as u32);
        // PC2 置换：从移位后的 56 位中选出 48 位作为子密钥
        round_keys[i] = permute(key56, &PC2);
    }
}

/// Feistel 轮函数：对 32 位半块进行扩展、异或、S 盒替换和 P 置换。
///
/// # 参数
/// - `half_block`: 32 位输入
/// - `subkey`: 48 位子密钥
///
/// # 返回
/// 32 位输出
#[inline]
fn f(half_block: u64, subkey: u64) -> u64 {
    // 扩展置换：32 位 -> 48 位（自定义表中有 -1 导致某些位为 0）
    let expanded = permute(half_block, &EXPANSION);
    // 与子密钥异或
    let mixed = expanded ^ subkey;

    // S 盒替换：将 48 位分成 8 组 6 位，每组通过对应 S 盒映射为 4 位
    let mut s_output = 0u64;
    // 从最高字节到最低字节处理（对应 8 个 S 盒）
    for i in (0..8u8).rev() {
        // 提取第 i 个 6 位组（从高位算起），注意使用 0x3F 掩码
        let six_bits = ((mixed >> (i * 8)) & 0x3F) as usize;
        // 查 S 盒并追加到结果（左移 4 位）
        s_output = (s_output << 4) | u64::from(S_BOXES[i as usize][six_bits]);
    }

    // P 置换：将 S 盒输出的 32 位重新排列
    permute(s_output, &PERMUTATION)
}

/// 加密一个 64 位数据块（使用给定的轮密钥）。
///
/// # 参数
/// - `block`: 64 位输入块
/// - `round_keys`: 16 个轮子密钥（整数数组）
///
/// # 返回
/// 64 位加密结果
#[inline]
fn encrypt_block(block: u64, round_keys: &[u64; 16]) -> u64 {
    // 初始置换
    let permuted = permute(block, &INITIAL_PERMUTATION);

    // 拆分为左右各 32 位
    let mut left = permuted & 0xFFFF_FFFF;
    let mut right = (permuted >> 32) & 0xFFFF_FFFF;

    // 16 轮 Feistel 运算
    for &subkey in round_keys.iter() {
        let new_right = left ^ f(right, subkey);
        left = right;
        right = new_right;
    }

    // 合并左右半（注意交换后 left 是原 right，right 是原 left ^ f）
    let combined = (left << 32) | right;

    // 逆初始置换
    permute(combined, &INVERSE_PERMUTATION)
}

/// 对数据进行 DES 加/解密（ECB 模式，零填充）。
///
/// # 参数
/// - `data`: 明文或密文字节串
/// - `key`: 8 字节密钥
/// - `mode`: 加密或解密模式
///
/// # 返回
/// 加密或解密后的字节串（长度总是 8 的倍数）
pub fn des_crypt(data: &[u8], key: &[u8; 8], mode: CryptMode) -> Vec<u8> {
    // 将密钥转换为整数并生成轮密钥
    let key_int = u64::from_le_bytes(*key);
    let mut round_keys = [0u64; 16];
    derive_keys(key_int, &mut round_keys);

    // 解密时逆序使用轮密钥
    if matches!(mode, CryptMode::Decrypt) {
        round_keys.reverse();
    }

    // 按 8 字节块处理
    let block_count = data.len().div_ceil(8);
    let mut output = vec![0u8; block_count * 8];

    for (i, chunk) in data.chunks(8).enumerate() {
        let mut block_bytes = [0u8; 8];
        block_bytes[..chunk.len()].copy_from_slice(chunk);
        let block_int = u64::from_le_bytes(block_bytes);
        let processed = encrypt_block(block_int, &round_keys);
        output[i * 8..(i + 1) * 8].copy_from_slice(&processed.to_le_bytes());
    }

    output
}

// ============================================================================
// Base64 包装
// ============================================================================

/// kwDES 加密 + Base64 编码。
///
/// # 参数
/// - `msg`: 待加密的字符串（将被编码为 UTF-8 字节）
/// - `key`: 8 字节密钥，默认 `b"ylzsxkwm"`
///
/// # 返回
/// Base64 编码的密文字符串（无换行）
#[allow(dead_code)]
pub fn base64_encrypt(msg: &str, key: &[u8; 8]) -> String {
    let encrypted = des_crypt(msg.as_bytes(), key, CryptMode::Encrypt);
    STANDARD.encode(encrypted)
}

/// kwDES 解密 + Base64 解码。
///
/// # 参数
/// - `msg`: Base64 编码的密文
/// - `key`: 8 字节密钥，默认 `b"ylzsxkwm"`
///
/// # 返回
/// 解密后的 UTF-8 字符串（已去除尾部 `\0` 填充）
pub fn base64_decrypt(msg: &str, key: &[u8; 8]) -> Result<String, String> {
    let decoded = STANDARD
        .decode(msg)
        .map_err(|e| format!("Base64 解码失败: {}", e))?;
    let decrypted = des_crypt(&decoded, key, CryptMode::Decrypt);
    let end = decrypted
        .iter()
        .rposition(|&b| b != 0)
        .map(|p| p + 1)
        .unwrap_or(0);
    String::from_utf8(decrypted[..end].to_vec()).map_err(|e| format!("UTF-8 解码失败: {}", e))
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 permute 的基本性质：全 0 输入得到全 0，全 1 输入得到全 1
    #[test]
    fn test_permute_basic() {
        let all_zero = permute(0, &INITIAL_PERMUTATION);
        assert_eq!(all_zero, 0);
        let all_one = permute(u64::MAX, &INITIAL_PERMUTATION);
        assert_eq!(all_one, u64::MAX);
    }

    /// 测试 permute 的位映射：输入 bit 0 = 1 应映射到输出 bit 39
    #[test]
    fn test_permute_one_bit() {
        let out = permute(1, &INITIAL_PERMUTATION);
        assert_eq!(out, 1u64 << 39);
    }

    /// 测试密钥转换
    #[test]
    fn test_key_conversion() {
        let key_int = u64::from_le_bytes(*DEFAULT_KEY);
        assert_eq!(key_int, 0x6d776b78737a6c79u64);
    }

    /// 验证加密后再解密能还原原文
    #[test]
    fn test_roundtrip() {
        let plain = "12345";
        let cipher = base64_encrypt(plain, DEFAULT_KEY);
        let decrypted = base64_decrypt(&cipher, DEFAULT_KEY).expect("解密失败");
        assert_eq!(decrypted, plain);
    }

    /// 测试简单加密用例
    #[test]
    fn test_simple_vector() {
        let cipher = base64_encrypt("12345", DEFAULT_KEY);
        assert_eq!(cipher, "6kgYe3imgc4=");
    }
}
