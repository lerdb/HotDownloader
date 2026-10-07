use super::*;
use futures_util::{stream, StreamExt};

struct Running<'a>(&'a AtomicUsize);
impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl MonitorService {
    pub async fn tick(&self, runtime: &ServerRuntime) -> Result<()> {
        if self.persistence_failed.load(Ordering::Relaxed) {
            return Err("台账持久化失败，停止自动下载；修复存储后重启服务".into());
        }
        if let Ok(_guard) = self.reconcile_lock.try_lock() {
            self.reconcile(runtime).await?;
        }
        let monitors = self.store.all::<Monitor>("monitor")?;
        let budget = AtomicUsize::new(BATCH_SIZE);
        let results = stream::iter(
            monitors
                .into_iter()
                .map(|monitor| self.run_monitor(runtime, monitor, &budget)),
        )
        .buffer_unordered(4)
        .collect::<Vec<_>>()
        .await;
        let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }

    async fn run_monitor(
        &self,
        runtime: &ServerRuntime,
        mut monitor: Monitor,
        budget: &AtomicUsize,
    ) -> Result<()> {
        let lock = {
            let mut locks = self.monitor_locks.lock().unwrap();
            locks.retain(|_, lock| lock.strong_count() > 0);
            if let Some(lock) = locks.get(&monitor.id).and_then(std::sync::Weak::upgrade) {
                lock
            } else {
                let lock = Arc::new(Mutex::new(()));
                locks.insert(monitor.id.clone(), Arc::downgrade(&lock));
                lock
            }
        };
        let Ok(_run_guard) = lock.try_lock() else {
            return Ok(());
        };
        let due = monitor.requested || (monitor.config.enabled && monitor.next_check <= now());
        let pending = self
            .store
            .pending_entries(&monitor)?
            .iter()
            .any(|e| participates(&monitor, e));
        if !due && !pending {
            return Ok(());
        }
        let mut record = CheckRecord {
            started_at: now(),
            status: "running".into(),
            trigger: if monitor.requested {
                "manual"
            } else if due {
                "scheduled"
            } else {
                "backfill"
            }
            .into(),
            message: "正在扫描、检查与派发".into(),
            ..Default::default()
        };
        {
            let _guard = self.operation.lock().await;
            if !self.current_revision(&monitor)? {
                return Ok(());
            }
            let mut history = self
                .store
                .get::<Vec<CheckRecord>>("history", &monitor.id)?
                .unwrap_or_default();
            history.push(record.clone());
            if history.len() > HISTORY_LIMIT {
                history.drain(..history.len() - HISTORY_LIMIT);
            }
            self.store.put("history", &monitor.id, &history)?;
        }
        self.running.fetch_add(1, Ordering::Relaxed);
        let _running = Running(&self.running);
        let result = self
            .monitor_work(runtime, &mut monitor, due, budget, &mut record)
            .await;
        record.finished_at = now();
        if let Err(error) = &result {
            record.status = "failed".into();
            record.message = error.clone();
        } else if record.status == "running" {
            record.status = if record.failed > 0 {
                "warning"
            } else {
                "completed"
            }
            .into();
            record.message = if record.failed > 0 {
                "本轮派发存在失败，请查看逐曲状态"
            } else {
                "本轮处理完成，下载结果见逐曲状态"
            }
            .into();
        }
        self.store
            .change::<Vec<CheckRecord>>("history", &monitor.id, |history| {
                if let Some(last) = history.last_mut() {
                    *last = record;
                }
            })?;
        result
    }

    fn current_revision(&self, monitor: &Monitor) -> Result<bool> {
        Ok(self
            .store
            .get::<Monitor>("monitor", &monitor.id)?
            .is_some_and(|m| m.revision == monitor.revision))
    }

    async fn monitor_work(
        &self,
        runtime: &ServerRuntime,
        monitor: &mut Monitor,
        due: bool,
        budget: &AtomicUsize,
        record: &mut CheckRecord,
    ) -> Result<()> {
        if let Err(error) = self.scan_cached(runtime, monitor.requested).await {
            let _guard = self.operation.lock().await;
            if self.current_revision(monitor)? {
                self.store.change::<Monitor>("monitor", &monitor.id, |m| {
                    m.last_state = "scan_failed".into();
                    m.last_result = format!("扫描失败，本轮补齐已跳过: {error}");
                })?;
            }
            return Err(error);
        }
        if due {
            let result = tokio::time::timeout(
                Duration::from_secs(120),
                self.remote.fetch(monitor, runtime),
            )
            .await
            .map_err(|_| "读取歌单网络超时".to_string())
            .and_then(|r| r);
            let _guard = self.operation.lock().await;
            if !self.current_revision(monitor)? {
                record.status = "interrupted".into();
                record.message = "监控已修改或删除，本轮结果已丢弃".into();
                return Ok(());
            }
            let previous: HashSet<_> = monitor.members.iter().cloned().collect();
            let result = result.and_then(|songs| {
                self.ingest(monitor, songs, &runtime.environment.artist_separator())
            });
            if let Err(error) = result {
                monitor.last_check = now();
                monitor.next_check = now() + monitor.config.interval_minutes * 60;
                monitor.requested = false;
                monitor.last_result = format!("歌单检查失败: {error}");
                monitor.last_state = match failure_state(&error) {
                    State::NetworkFailed => "network_failed",
                    State::CredentialInvalid => "credential_invalid",
                    _ => "check_failed",
                }
                .into();
                self.store.put("monitor", &monitor.id, monitor)?;
                record.status = "failed".into();
                record.message = error;
                return Ok(());
            }
            record.added = monitor
                .members
                .iter()
                .filter(|mid| !previous.contains(*mid))
                .count();
        }
        let entries = self.store.pending_entries(monitor)?;
        let mut auth = None;
        for mut entry in entries {
            if !participates(monitor, &entry) || !eligible(&entry) {
                continue;
            }
            let matched = if entry.state == State::Pending
                || (entry.state == State::Ready && !entry.force_download)
            {
                let store = self.store.clone();
                let identity = entry.identity.clone();
                Some(
                    tokio::task::spawn_blocking(move || store.match_indexed(&identity))
                        .await
                        .map_err(|e| e.to_string())??,
                )
            } else {
                None
            };
            {
                let _guard = self.operation.lock().await;
                let _task_guard = runtime.tasks.creation_lock.lock().await;
                if !self.current_revision(monitor)? {
                    return Ok(());
                }
                let previous = entry.clone();
                if !entry.force_download && entry.task_id.is_none() {
                    let order =
                        hotdownloader_core::task::rules::TaskRules::from_settings(&json!({}))
                            .quality_order;
                    if let Some(quality) = self
                        .store
                        .all::<Monitor>("monitor")?
                        .iter()
                        .filter(|m| {
                            (m.config.enabled || m.draining)
                                && m.members.iter().any(|mid| mid == entry.mid())
                        })
                        .map(|m| m.config.quality.clone())
                        .min_by_key(|q| {
                            order
                                .iter()
                                .position(|quality| quality == q)
                                .unwrap_or(usize::MAX)
                        })
                    {
                        entry.quality = quality;
                    }
                }
                if let Some(matched) = matched {
                    match matched {
                        Match::Found(path) if runtime.tasks.path_reserved(&path) => {
                            entry.state = State::PendingConfirmation;
                            entry.message =
                                "候选文件正在被下载任务使用，请待任务结束后刷新候选".into();
                        }
                        Match::Found(path) => {
                            entry.state = State::Matched;
                            entry.path = Some(path);
                            entry.message = "已匹配本地文件".into();
                        }
                        Match::Missing => {
                            entry.state = State::Ready;
                            entry.message = "等待分批入队".into();
                        }
                        Match::Confirm(candidates, reason) => {
                            entry.state = State::PendingConfirmation;
                            entry.candidates = candidates;
                            entry.message = reason;
                        }
                    }
                }
                if !self
                    .store
                    .compare_and_put("entry", entry.mid(), &previous, &entry)?
                {
                    continue;
                }
                record.linked += usize::from(entry.state == State::Matched);
            }
            if !eligible(&entry) || budget.load(Ordering::Relaxed) == 0 {
                continue;
            }
            if auth.is_none() {
                auth = Some(
                    tokio::time::timeout(
                        Duration::from_secs(45),
                        self.remote.authenticate(runtime),
                    )
                    .await
                    .map_err(|_| "QQ 凭据校验网络超时".to_string())
                    .and_then(|r| r),
                );
            }
            // 耗时查询结束后重新核对监控与歌曲；启停、删除、逐曲操作不等待网络。
            let _guard = self.operation.lock().await;
            if !self.current_revision(monitor)? {
                return Ok(());
            }
            if !self
                .store
                .get::<Entry>("entry", entry.mid())?
                .is_some_and(|e| json!(e) == json!(entry))
            {
                continue;
            }
            if budget
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_sub(1))
                .is_err()
            {
                break;
            }
            let mid = entry.mid().to_string();
            let enqueued = self
                .dispatch(runtime, entry, auth.as_ref().unwrap())
                .await?;
            record.enqueued += usize::from(enqueued);
            record.failed += usize::from(
                !enqueued
                    && self.store.get::<Entry>("entry", &mid)?.is_some_and(|e| {
                        matches!(
                            e.state,
                            State::NoQuality
                                | State::NetworkFailed
                                | State::DownloadFailed
                                | State::CredentialInvalid
                        )
                    }),
            );
        }
        Ok(())
    }
}

fn participates(monitor: &Monitor, entry: &Entry) -> bool {
    monitor.config.enabled || monitor.draining || entry.requested_by.as_deref() == Some(&monitor.id)
}
