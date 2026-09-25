use serde_json::Value;

use super::contract::QualityItem;

/// 从设置快照提取的任务策略。创建和重试共用同一套规则，避免前端各自实现。
pub struct TaskRules {
    pub auto_downgrade: bool,
    pub quality_order: Vec<String>,
    pub duplicate_strategy: String,
}

impl TaskRules {
    pub fn from_settings(settings: &Value) -> Self {
        let default_order = [
            "臻品母带",
            "臻品全景声 7.1.4",
            "臻品全景声 5.1",
            "臻品全景声",
            "hires",
            "flac",
            "ape",
            "320kmp3",
            "128kmp3",
            "300kogg",
            "192kogg",
            "100kogg",
            "96kogg",
            "192kaac",
            "96kaac",
            "48kaac",
        ];
        // 用户可能保存了旧版或重复的品质值：只保留已知值，去重后补齐默认顺序。
        // 这样新增品质不会让旧设置失效，也不会改变用户已指定的相对顺序。
        let mut quality_order = Vec::new();
        if let Some(items) = settings["qualityDowngradeOrder"].as_array() {
            for quality in items.iter().filter_map(|v| v.as_str()) {
                if default_order.contains(&quality) && !quality_order.iter().any(|q| q == quality) {
                    quality_order.push(quality.to_string());
                }
            }
        }
        for quality in default_order {
            if !quality_order.iter().any(|q| q == quality) {
                quality_order.push(quality.to_string());
            }
        }
        Self {
            auto_downgrade: settings["autoDowngrade"].as_bool().unwrap_or(true),
            quality_order,
            duplicate_strategy: settings["duplicateStrategy"]
                .as_str()
                .unwrap_or("ask")
                .to_string(),
        }
    }

    pub fn initial_quality<'a>(
        &self,
        desired: &str,
        available: &'a [QualityItem],
    ) -> Option<&'a QualityItem> {
        // 首次创建任务的品质选择：
        // 1. 歌曲提供目标品质时直接使用；
        // 2. 缺失且允许降级时，从用户顺序中目标品质的下一项开始找；
        // 3. 目标不在顺序表、或后续没有歌曲实际提供的品质时返回 None。
        // 因此品质、filename、size 始终来自同一个 QualityItem。
        if let Some(item) = available.iter().find(|q| q.quality == desired) {
            return Some(item);
        }
        if !self.auto_downgrade {
            return None;
        }
        self.next_quality(desired, available)
    }

    pub fn next_quality<'a>(
        &self,
        current: &str,
        available: &'a [QualityItem],
    ) -> Option<&'a QualityItem> {
        // 从当前品质之后查找，跳过歌曲不提供的品质，绝不回头升级。
        // 若当前品质不在规则表中，不猜测顺序，交由调用方返回明确错误。
        let current_index = self.quality_order.iter().position(|q| q == current)?;
        self.quality_order[current_index + 1..]
            .iter()
            .find_map(|quality| available.iter().find(|item| &item.quality == quality))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualities() -> Vec<QualityItem> {
        vec![
            QualityItem {
                quality: "flac".into(),
                filename: "a.flac".into(),
                size: 12,
            },
            QualityItem {
                quality: "320kmp3".into(),
                filename: "a.mp3".into(),
                size: 8,
            },
        ]
    }

    #[test]
    fn initial_selection_only_downgrades_after_requested_quality() {
        let rules = TaskRules::from_settings(&serde_json::json!({}));
        assert_eq!(
            rules
                .initial_quality("hires", &qualities())
                .unwrap()
                .quality,
            "flac"
        );
        assert!(rules.initial_quality("128kmp3", &qualities()).is_none());
    }

    #[test]
    fn retry_uses_matching_filename_and_size() {
        let rules = TaskRules::from_settings(&serde_json::json!({}));
        let available = qualities();
        let next = rules.next_quality("flac", &available).unwrap();
        assert_eq!(
            (&next.quality[..], &next.filename[..], next.size),
            ("320kmp3", "a.mp3", 8)
        );
    }
}
