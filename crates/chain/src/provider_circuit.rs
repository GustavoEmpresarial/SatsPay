//! Shared, process-local circuit state. Clients are recreated for each wallet,
//! so state must live beyond an individual HTTP client.
use crate::types::ChainError;
use std::collections::HashMap;
use std::future::Future;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

const FAILURE_LIMIT: u8 = 3;
const OPEN_FOR: Duration = Duration::from_secs(60);

#[derive(Default)]
struct State {
    failures: u8,
    opened_at: Option<Instant>,
    probing: bool,
}

impl State {
    fn allow(&mut self, now: Instant) -> bool {
        match self.opened_at {
            None => true,
            Some(opened) if now.duration_since(opened) >= OPEN_FOR && !self.probing => {
                self.probing = true;
                true
            }
            _ => false,
        }
    }

    fn complete(&mut self, now: Instant, success: bool) {
        self.probing = false;
        if success {
            self.failures = 0;
            self.opened_at = None;
        } else {
            self.failures = self.failures.saturating_add(1);
            if self.failures >= FAILURE_LIMIT {
                self.opened_at = Some(now);
            }
        }
    }
}

static CIRCUITS: OnceLock<Mutex<HashMap<String, State>>> = OnceLock::new();

fn circuits() -> &'static Mutex<HashMap<String, State>> {
    CIRCUITS.get_or_init(|| Mutex::new(HashMap::new()))
}

struct ProbeReset<'a> { provider: &'a str, armed: bool }

impl Drop for ProbeReset<'_> {
    fn drop(&mut self) {
        if self.armed {
            let mut all = circuits().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(state) = all.get_mut(self.provider) { state.probing = false; }
        }
    }
}

pub async fn call<T, F, Fut>(provider: &str, operation: F) -> Result<T, ChainError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<T, ChainError>>,
{
    let was_probe = {
        let mut all = circuits()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let state = all.entry(provider.to_owned()).or_default();
        if !state.allow(Instant::now()) {
            return Err(ChainError { message: format!("{provider} circuit open") });
        }
        state.probing
    };
    let mut reset = ProbeReset { provider, armed: was_probe };
    let result = operation().await;
    let mut all = circuits()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let state = all.entry(provider.to_owned()).or_default();
    let newly_opened = result.is_err() && state.opened_at.is_none() && state.failures.saturating_add(1) >= FAILURE_LIMIT;
    state.complete(Instant::now(), result.is_ok());
    reset.armed = false;
    if newly_opened {
        tracing::warn!(provider, "chain provider circuit opened");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn opens_after_three_failures_and_skips_fourth_call() {
        let key = format!("test-{}", uuid::Uuid::new_v4());
        for _ in 0..3 {
            let _: Result<(), _> = call(&key, || async {
                Err(ChainError {
                    message: "down".into(),
                })
            })
            .await;
        }
        let result: Result<(), _> = call(&key, || async {
            panic!("must not probe open circuit");
        })
        .await;
        assert!(result.unwrap_err().message.contains("circuit open"));
    }

    #[test]
    fn one_half_open_probe_closes_on_success_and_reopens_on_failure() {
        let now = Instant::now();
        let mut state = State::default();
        for _ in 0..3 { state.complete(now, false); }
        assert!(!state.allow(now + Duration::from_secs(59)));
        assert!(state.allow(now + Duration::from_secs(60)));
        assert!(!state.allow(now + Duration::from_secs(60)));
        state.complete(now + Duration::from_secs(60), false);
        assert!(!state.allow(now + Duration::from_secs(61)));
        assert!(state.allow(now + Duration::from_secs(120)));
        state.complete(now + Duration::from_secs(120), true);
        assert!(state.allow(now + Duration::from_secs(120)));
    }
}
