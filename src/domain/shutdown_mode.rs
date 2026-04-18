use std::time::Instant;

/// デーモンの shutdown 状態を型安全に表現する。
///
/// PollingUseCase の実行時状態として保持される。永続化は不要。
#[derive(Debug, Clone)]
pub enum ShutdownMode {
    /// 通常動作中（shutdown 未要求）
    None,
    /// Graceful shutdown 待機中
    ///
    /// `deadline: None` の場合は無限待機（shutdown_timeout_secs = 0）。
    /// `deadline: Some(t)` の場合は指定時刻までに完了しなければ SIGKILL。
    Graceful { deadline: Option<Instant> },
    /// 強制終了（即時 SIGKILL）
    Force,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn shutdown_mode_none_is_default_state() {
        let mode = ShutdownMode::None;
        assert!(matches!(mode, ShutdownMode::None));
    }

    #[test]
    fn shutdown_mode_graceful_without_deadline() {
        let mode = ShutdownMode::Graceful { deadline: None };
        assert!(matches!(mode, ShutdownMode::Graceful { deadline: None }));
    }

    #[test]
    fn shutdown_mode_graceful_with_deadline() {
        let deadline = Instant::now() + Duration::from_secs(300);
        let mode = ShutdownMode::Graceful {
            deadline: Some(deadline),
        };
        assert!(matches!(
            mode,
            ShutdownMode::Graceful { deadline: Some(_) }
        ));
    }

    #[test]
    fn shutdown_mode_force() {
        let mode = ShutdownMode::Force;
        assert!(matches!(mode, ShutdownMode::Force));
    }
}
