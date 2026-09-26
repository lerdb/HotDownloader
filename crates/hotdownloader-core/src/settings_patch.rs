//! 设置的字段级更新契约。存储与事件由运行时实现，合并和冲突规则由两端共用。

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const REVISION_KEY: &str = "_settingsRevision";
const CREDENTIAL_KEYS: &[&str] = &[
    "loginUin",
    "authst",
    "refreshToken",
    "refreshKey",
    "accessToken",
    "openid",
    "loginResponseData",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub changes: Map<String, Value>,
    pub expected: Map<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub revision: u64,
    pub settings: Value,
}

#[derive(Clone, Copy, Debug)]
pub enum SettingsScope {
    Tauri,
    Web,
}

#[derive(Debug)]
pub enum SettingsPatchError {
    Invalid(String),
    Conflict {
        fields: Vec<String>,
        snapshot: SettingsSnapshot,
    },
}

pub struct AppliedSettingsPatch {
    pub stored: Value,
    pub snapshot: SettingsSnapshot,
    pub changed_fields: Vec<String>,
}

/// 修订号存放在现有设置对象内，旧设置文件自然以修订号 0 开始。
/// 快照去掉内部字段和登录凭据，普通设置同步不会携带凭据。
pub fn snapshot(stored: &Value) -> SettingsSnapshot {
    let mut visible = stored.as_object().cloned().unwrap_or_default();
    let revision = visible
        .remove(REVISION_KEY)
        .and_then(|value| value.as_u64())
        .unwrap_or(0);
    for key in CREDENTIAL_KEYS {
        visible.remove(*key);
    }
    SettingsSnapshot {
        revision,
        settings: Value::Object(visible),
    }
}

/// 每个字段都带提交者看到的原值。两个页面修改不同字段时分别合并；
/// 修改同一字段时返回最新快照，由页面让用户选择保留哪份值。
pub fn apply_patch(
    stored: &Value,
    patch: SettingsPatch,
    scope: SettingsScope,
) -> Result<AppliedSettingsPatch, SettingsPatchError> {
    let current = stored
        .as_object()
        .ok_or_else(|| SettingsPatchError::Invalid("设置必须是 JSON 对象".into()))?;
    if patch.changes.is_empty() {
        return Err(SettingsPatchError::Invalid("设置更新不能为空".into()));
    }

    let mut conflicts = Vec::new();
    for (field, value) in &patch.changes {
        validate_field(field, value, scope)?;
        let expected = patch
            .expected
            .get(field)
            .ok_or_else(|| SettingsPatchError::Invalid(format!("缺少 {field} 的原值")))?;
        let actual = current.get(field).unwrap_or(&Value::Null);
        if actual != expected {
            conflicts.push(field.clone());
        }
    }
    if !conflicts.is_empty() {
        return Err(SettingsPatchError::Conflict {
            fields: conflicts,
            snapshot: snapshot(stored),
        });
    }

    let mut next = current.clone();
    let mut changed_fields = Vec::new();
    for (field, value) in patch.changes {
        if next.get(&field) != Some(&value) {
            next.insert(field.clone(), value);
            changed_fields.push(field);
        }
    }
    if !changed_fields.is_empty() {
        let revision = snapshot(stored)
            .revision
            .checked_add(1)
            .ok_or_else(|| SettingsPatchError::Invalid("设置修订号已达到上限".into()))?;
        next.insert(REVISION_KEY.into(), Value::from(revision));
    }
    let stored = Value::Object(next);
    let snapshot = snapshot(&stored);
    Ok(AppliedSettingsPatch {
        stored,
        snapshot,
        changed_fields,
    })
}

fn validate_field(
    field: &str,
    value: &Value,
    scope: SettingsScope,
) -> Result<(), SettingsPatchError> {
    let valid = match field {
        "autoDowngrade" | "writeMetadata" | "downloadLrc" => value.is_boolean(),
        "maxConcurrent" => matches!(value.as_u64(), Some(1..=32)),
        "defaultQuality" => value
            .as_str()
            .is_some_and(|text| !text.is_empty() && text.len() <= 512),
        // 空模板和空连接符由现有下载逻辑处理，这里保留用户原有设置语义。
        "namingTemplate" | "artistSeparator" => {
            value.as_str().is_some_and(|text| text.len() <= 512)
        }
        "duplicateStrategy" => matches!(
            value.as_str(),
            Some("ask" | "overwrite" | "rename" | "cancel")
        ),
        "qualityDowngradeOrder" => value.as_array().is_some_and(|items| {
            !items.is_empty()
                && items.len() <= 64
                && items.iter().all(|item| {
                    item.as_str()
                        .is_some_and(|quality| !quality.is_empty() && quality.len() <= 64)
                })
        }),
        "jumpToTask" | "notifyOnComplete" if matches!(scope, SettingsScope::Tauri) => {
            value.is_boolean()
        }
        "downloadDir" | "safFolderUri" | "safFolderName"
            if matches!(scope, SettingsScope::Tauri) =>
        {
            value.is_string()
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(SettingsPatchError::Invalid(format!(
            "设置字段 {field} 的值无效"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn patch(field: &str, value: Value, expected: Value) -> SettingsPatch {
        SettingsPatch {
            changes: Map::from_iter([(field.into(), value)]),
            expected: Map::from_iter([(field.into(), expected)]),
        }
    }

    #[test]
    fn different_fields_merge_without_overwriting_each_other() {
        let start = json!({"maxConcurrent": 3, "namingTemplate": "{song}"});
        let first = apply_patch(
            &start,
            patch("maxConcurrent", json!(5), json!(3)),
            SettingsScope::Web,
        )
        .unwrap();
        let second = apply_patch(
            &first.stored,
            patch("namingTemplate", json!("{artist}"), json!("{song}")),
            SettingsScope::Web,
        )
        .unwrap();
        assert_eq!(second.snapshot.settings["maxConcurrent"], 5);
        assert_eq!(second.snapshot.settings["namingTemplate"], "{artist}");
        assert_eq!(second.snapshot.revision, 2);
    }

    #[test]
    fn same_field_reports_conflict_and_current_snapshot() {
        let current = json!({"maxConcurrent": 5, "_settingsRevision": 4});
        let error = apply_patch(
            &current,
            patch("maxConcurrent", json!(7), json!(3)),
            SettingsScope::Web,
        )
        .err()
        .unwrap();
        match error {
            SettingsPatchError::Conflict { fields, snapshot } => {
                assert_eq!(fields, ["maxConcurrent"]);
                assert_eq!(snapshot.settings["maxConcurrent"], 5);
                assert_eq!(snapshot.revision, 4);
            }
            _ => panic!("应返回字段冲突"),
        }
    }

    #[test]
    fn web_rejects_runtime_path_and_credentials() {
        let current = json!({});
        for field in ["downloadDir", "authst", "jumpToTask"] {
            assert!(matches!(
                apply_patch(
                    &current,
                    patch(field, json!("value"), Value::Null),
                    SettingsScope::Web,
                ),
                Err(SettingsPatchError::Invalid(_))
            ));
        }
    }
}
