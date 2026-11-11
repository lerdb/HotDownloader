//! 平台标识和媒体标识用于识别同一录音，标题、专辑和文件名只用于展示。
use super::contract::{SongInput, TaskRecord};

#[derive(Clone, Copy)]
pub struct RecordingIdentity<'a> {
    platform: &'a str,
    id: u64,
    mid: &'a str,
    media_mid: &'a str,
}

impl<'a> From<&'a SongInput> for RecordingIdentity<'a> {
    fn from(song: &'a SongInput) -> Self {
        Self {
            platform: &song.platform,
            id: song.id,
            mid: &song.mid,
            media_mid: &song.media_mid,
        }
    }
}

impl<'a> From<&'a TaskRecord> for RecordingIdentity<'a> {
    fn from(task: &'a TaskRecord) -> Self {
        Self {
            platform: &task.platform,
            id: task.song_id,
            mid: &task.song_mid,
            media_mid: &task.media_mid,
        }
    }
}

impl RecordingIdentity<'_> {
    pub fn matches(self, other: Self) -> bool {
        self.platform == other.platform
            && ((!self.mid.is_empty() && self.mid == other.mid)
                || (self.mid.is_empty()
                    && other.mid.is_empty()
                    && self.id != 0
                    && self.id == other.id)
                || (!self.media_mid.is_empty() && self.media_mid == other.media_mid))
    }
}
