#!/usr/bin/env node
/**
 * 生成第三方许可证声明。
 *
 * 输出（三份文件都需要提交到 git）：
 *   - src/data/licenses.ts          前端 TypeScript 模块
 *   - NOTICE                        简表（打包资源）
 *   - THIRD_PARTY_LICENSES.txt      合并全文（打包资源）
 *
 * 用法：npm run generate:licenses
 *
 * 设计原则：
 *   - 许可证全文统一取自 SPDX 官方数据（spdx-license-list/full）
 *   - 无法解析的许可证直接报错，由 OVERRIDES 或 MANUAL_TEXTS 表补全
 */

import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const licenseChecker = require('license-checker-rseidelsohn');
// /full 子入口在每条 SPDX 条目上附带 licenseText 字段。
const spdx = require('spdx-license-list/full');

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '..');
const srcTauri = path.join(projectRoot, 'src-tauri');
const dataDir = path.join(projectRoot, 'src', 'data');

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------
const NOTICE_MIN_NAME_WIDTH = 20;
const NOTICE_MIN_VERSION_WIDTH = 8;
const LICENSE_TABLE_COLUMN_SEPARATOR = 2;
const LICENSE_RULE_WIDTH = 78;
const LICENSE_TABLE_RULE_WIDTH = 30;

// ---------------------------------------------------------------------------
// 手动覆盖表
// ---------------------------------------------------------------------------
// 补齐依赖元数据里缺失或写法不规范的许可证。
// npm 表以含 scope 的包名为键，cargo 表以 crate 名为键。
const NPM_OVERRIDES = {
    '@sahil-vartak/tauri-plugin-safe-area-insets-css-api': 'MIT',
};

const CARGO_OVERRIDES = {
    'umc_qmc': 'Apache-2.0 OR MIT',
    'umc_utils': 'Apache-2.0 OR MIT',
};

// ---------------------------------------------------------------------------
// 手动许可证全文表
// ---------------------------------------------------------------------------
// 存放 spdx-license-list/full 未收录的许可证全文。
// 值为 { name, text }。
const MANUAL_TEXTS = {
    // 目前为空。
};

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

function hasOwn(obj, key) {
    return Object.prototype.hasOwnProperty.call(obj, key);
}

function normalizeLicense(lic) {
    if (!lic) return 'Unknown';
    if (Array.isArray(lic)) {
        return lic.map(normalizeLicense).filter(Boolean).join(' OR ');
    }
    if (typeof lic === 'object' && lic.type) {
        return normalizeLicense(lic.type);
    }
    return String(lic)
        .trim()
        // SPDX 早期语法用 "/" 表示 OR，许多老 crate 沿用此写法。
        .replace(/\s*\/\s*/g, ' OR ')
        .replace(/\s+/g, ' ');
}

/**
 * 解析 SPDX 表达式，返回其中的 ID 列表。
 * 支持嵌套表达式，例如 "(MIT OR Apache-2.0) AND BSD-3-Clause"。
 * 跳过 WITH 后面的 exception 名称。
 */
function parseSpdxIds(expr) {
    if (!expr || expr === 'Unknown') return [];
    const tokens = expr
        .replace(/[()]/g, ' ')
        .split(/\s+/)
        .filter(Boolean);

    const ids = [];
    for (let i = 0; i < tokens.length; i++) {
        const tok = tokens[i];
        if (tok === 'OR' || tok === 'AND') continue;
        if (tok === 'WITH') { i++; continue; }
        ids.push(tok);
    }
    return [...new Set(ids)];
}

function sortByName(arr) {
    return arr.slice().sort((a, b) => a.name.localeCompare(b.name, 'en'));
}

/**
 * 取出 SPDX 条目里的许可证正文。
 * 优先读 licenseText 字段，缺失时读 text 字段。
 */
function extractText(entry) {
    if (!entry) return '';
    return (entry.licenseText || entry.text || '').trim();
}

/**
 * 计算 name 和 version 两列的宽度。
 * 每列宽度取组件字段最大长度与保底宽度的较大值，再加列间距。
 */
function computeColumnWidths(components, minName, minVersion) {
    const nameWidth = components.reduce(
        (m, c) => Math.max(m, c.name.length),
        minName
    ) + LICENSE_TABLE_COLUMN_SEPARATOR;
    const versionWidth = components.reduce(
        (m, c) => Math.max(m, c.version.length),
        minVersion
    ) + LICENSE_TABLE_COLUMN_SEPARATOR;
    return { nameWidth, versionWidth };
}

// ---------------------------------------------------------------------------
// 收集 npm 依赖（含传递依赖）
// ---------------------------------------------------------------------------

function collectNpmDeps() {
    console.log('📦 收集前端依赖（license-checker-rseidelsohn）...');
    return new Promise((resolve, reject) => {
        licenseChecker.init(
            {
                start: projectRoot,
                production: true,
                excludePrivatePackages: true,
            },
            (err, packages) => {
                if (err) {
                    return reject(
                        new Error(`license-checker-rseidelsohn 初始化失败：${err.message}`)
                    );
                }

                const result = [];
                for (const [key, info] of Object.entries(packages)) {
                    const atIdx = key.lastIndexOf('@');
                    const name = key.slice(0, atIdx);
                    const version = key.slice(atIdx + 1);

                    let license = normalizeLicense(info.licenses);
                    if (hasOwn(NPM_OVERRIDES, name)) license = NPM_OVERRIDES[name];

                    result.push({ name, version, license });
                }
                resolve(sortByName(result));
            }
        );
    });
}

// ---------------------------------------------------------------------------
// 收集 cargo 依赖（含传递依赖，排除 workspace 自身）
// ---------------------------------------------------------------------------

function collectCargoDeps() {
    console.log('📦 收集 Rust 依赖（cargo metadata）...');
    const output = execFileSync(
        'cargo',
        ['metadata', '--format-version', '1'],
        {
            cwd: srcTauri,
            maxBuffer: 64 * 1024 * 1024,
            encoding: 'utf8',
            stdio: ['ignore', 'pipe', 'inherit'],
        }
    );
    const metadata = JSON.parse(output);
    const workspaceMembers = new Set(metadata.workspace_members);

    const result = [];
    for (const pkg of metadata.packages) {
        if (workspaceMembers.has(pkg.id)) continue;

        let license = normalizeLicense(pkg.license);
        if (hasOwn(CARGO_OVERRIDES, pkg.name)) license = CARGO_OVERRIDES[pkg.name];

        result.push({ name: pkg.name, version: pkg.version, license });
    }
    return sortByName(result);
}

// ---------------------------------------------------------------------------
// 校验并收集唯一 SPDX 全文
// ---------------------------------------------------------------------------

function collectLicenseTexts(allComponents) {
    const missingMetadata = [];
    const missingText = [];
    const ids = new Set();

    for (const comp of allComponents) {
        const lic = comp.license;
        if (!lic || lic === 'Unknown' || lic === 'UNLICENSED') {
            missingMetadata.push(`${comp.name} (${lic || 'missing'})`);
            continue;
        }
        const parsed = parseSpdxIds(lic);
        if (parsed.length === 0) {
            missingMetadata.push(`${comp.name} (${lic})`);
            continue;
        }
        for (const id of parsed) ids.add(id);
    }

    const texts = [];
    for (const id of [...ids].sort()) {
        const fromPkg = hasOwn(spdx, id) ? spdx[id] : undefined;
        const fromManual = hasOwn(MANUAL_TEXTS, id) ? MANUAL_TEXTS[id] : undefined;

        const text = extractText(fromPkg) || (fromManual?.text || '').trim();
        const name = fromPkg?.name || fromManual?.name || id;

        if (!text) {
            missingText.push(id);
            continue;
        }
        texts.push({ id, name, text });
    }

    if (missingMetadata.length > 0 || missingText.length > 0) {
        console.error('\n❌ 生成失败：\n');

        if (missingMetadata.length > 0) {
            console.error('  组件许可证元数据缺失或不可解析。');
            console.error('  请在 NPM_OVERRIDES 或 CARGO_OVERRIDES 表中补全：\n');
            for (const u of missingMetadata.slice().sort()) console.error('     - ' + u);
            console.error('');
        }

        if (missingText.length > 0) {
            console.error('  以下 SPDX ID 无法从 spdx-license-list/full 取到全文。');
            console.error('  ID 拼写有误时请修正 NPM_OVERRIDES 或 CARGO_OVERRIDES 中的条目；');
            console.error('  ID 合法但未收录时请把全文加入 MANUAL_TEXTS：\n');
            for (const u of missingText.slice().sort()) console.error('     - ' + u);
            console.error('');
        }

        process.exit(1);
    }
    return texts;
}

// ---------------------------------------------------------------------------
// 输出 1：src/data/licenses.ts
// ---------------------------------------------------------------------------

function writeLicensesTs(rust, frontend, texts) {
    const content = `// AUTO-GENERATED by scripts/generate-licenses.mjs
// Do not edit manually. After changing dependencies, run: npm run generate:licenses

export interface ComponentInfo {
  name: string;
  version: string;
  license: string;
}

export interface LicenseText {
  id: string;
  name: string;
  text: string;
}

export const rustComponents: ComponentInfo[] = ${JSON.stringify(rust, null, 2)};

export const frontendComponents: ComponentInfo[] = ${JSON.stringify(frontend, null, 2)};

export const licenseTexts: LicenseText[] = ${JSON.stringify(texts, null, 2)};
`;
    fs.mkdirSync(dataDir, { recursive: true });
    fs.writeFileSync(path.join(dataDir, 'licenses.ts'), content, 'utf8');
    console.log('  ✓ src/data/licenses.ts');
}

// ---------------------------------------------------------------------------
// 输出 2：NOTICE（简表）
// ---------------------------------------------------------------------------

function writeNotice(rust, frontend) {
    const all = [...rust, ...frontend];
    const { nameWidth, versionWidth } = computeColumnWidths(
        all,
        NOTICE_MIN_NAME_WIDTH,
        NOTICE_MIN_VERSION_WIDTH
    );

    const line = (c) =>
        `  ${c.name.padEnd(nameWidth)}${c.version.padEnd(versionWidth)}${c.license}`;

    let out = `HotDownloader
Copyright 2026 lerd

This product includes software developed by independent third parties.
Each third-party component is governed by its own license, as indicated
below. For full license texts, see THIRD_PARTY_LICENSES.txt.

Third-party components:

Rust:
`;
    for (const c of rust) out += line(c) + '\n';
    out += `\nFrontend:\n`;
    for (const c of frontend) out += line(c) + '\n';

    fs.writeFileSync(path.join(projectRoot, 'NOTICE'), out, 'utf8');
    console.log('  ✓ NOTICE');
}

// ---------------------------------------------------------------------------
// 输出 3：THIRD_PARTY_LICENSES.txt（合并全文）
// ---------------------------------------------------------------------------

function writeThirdPartyLicenses(rust, frontend, texts) {
    const header = (title) =>
        `\n${'='.repeat(LICENSE_RULE_WIDTH)}\n${title}\n${'='.repeat(LICENSE_RULE_WIDTH)}\n\n`;

    const all = [...rust, ...frontend];
    const { nameWidth, versionWidth } = computeColumnWidths(
        all,
        'Component'.length,
        'Version'.length
    );

    const tableHeader =
        `${'Component'.padEnd(nameWidth)}${'Version'.padEnd(versionWidth)}License\n` +
        `${'-'.repeat(nameWidth)}${'-'.repeat(versionWidth)}${'-'.repeat(LICENSE_TABLE_RULE_WIDTH)}\n`;

    const tableRow = (c) =>
        `${c.name.padEnd(nameWidth)}${c.version.padEnd(versionWidth)}${c.license}\n`;

    let out = `HotDownloader — Third-Party License Notices\n`;
    out += `\n`;
    out += `This file lists all third-party components distributed with this\n`;
    out += `software, along with the full text of each license used.\n`;

    out += header('Rust Components');
    out += tableHeader;
    for (const c of rust) out += tableRow(c);

    out += header('Frontend Components');
    out += tableHeader;
    for (const c of frontend) out += tableRow(c);

    out += header('License Texts');
    for (const t of texts) {
        out += `\n${'-'.repeat(LICENSE_RULE_WIDTH)}\n${t.name} (${t.id})\n${'-'.repeat(LICENSE_RULE_WIDTH)}\n\n`;
        out += t.text + '\n';
    }

    fs.writeFileSync(
        path.join(projectRoot, 'THIRD_PARTY_LICENSES.txt'),
        out,
        'utf8'
    );
    console.log('  ✓ THIRD_PARTY_LICENSES.txt');
}

// ---------------------------------------------------------------------------
// 主流程
// ---------------------------------------------------------------------------

async function main() {
    const rust = collectCargoDeps();
    const frontend = await collectNpmDeps();

    console.log('📝 生成许可证文件...');
    const texts = collectLicenseTexts([...rust, ...frontend]);

    writeLicensesTs(rust, frontend, texts);
    writeNotice(rust, frontend);
    writeThirdPartyLicenses(rust, frontend, texts);

    console.log(
        `\n✅ 完成：${rust.length} 个 Rust 组件 + ${frontend.length} 个前端组件，共 ${texts.length} 种许可证。\n`
    );
}

main().catch((err) => {
    console.error('❌ 生成失败：', err);
    process.exit(1);
});