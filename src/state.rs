use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineAction {
    Disable,
    Enable,
}

#[derive(Debug, Clone)]
pub struct AwakeState {
    enabled: bool,
    keep_display_on: bool,
    deadline: Option<Instant>,
    deadline_action: DeadlineAction,
}

impl AwakeState {
    pub fn new(enabled: bool, keep_display_on: bool) -> Self {
        Self {
            enabled,
            keep_display_on,
            deadline: None,
            deadline_action: DeadlineAction::Disable,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn keep_display_on(&self) -> bool {
        self.keep_display_on
    }

    pub fn is_timed(&self) -> bool {
        self.deadline.is_some()
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.deadline = None;
    }

    pub fn toggle(&mut self) {
        self.set_enabled(!self.enabled);
    }

    pub fn set_keep_display_on(&mut self, enabled: bool) {
        self.keep_display_on = enabled;
    }

    pub fn activate_for(&mut self, now: Instant, duration: Duration) -> bool {
        let Some(deadline) = now.checked_add(duration) else {
            return false;
        };
        self.enabled = true;
        self.deadline = Some(deadline);
        self.deadline_action = DeadlineAction::Disable;
        true
    }

    pub fn pause_for(&mut self, now: Instant, duration: Duration) -> bool {
        let Some(deadline) = now.checked_add(duration) else {
            return false;
        };
        self.enabled = false;
        self.deadline = Some(deadline);
        self.deadline_action = DeadlineAction::Enable;
        true
    }

    /// Advances a timed session. Returns true when the visible state changed.
    pub fn tick(&mut self, now: Instant) -> bool {
        let Some(deadline) = self.deadline else {
            return false;
        };
        if now < deadline {
            return false;
        }

        self.enabled = self.deadline_action == DeadlineAction::Enable;
        self.deadline = None;
        true
    }

    pub fn remaining(&self, now: Instant) -> Option<Duration> {
        self.deadline
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    pub fn mode_name(&self) -> &'static str {
        match (self.enabled, self.deadline_action, self.deadline.is_some()) {
            (true, DeadlineAction::Disable, true) => "active_for",
            (false, DeadlineAction::Enable, true) => "paused_for",
            (true, _, _) => "active",
            (false, _, _) => "inactive",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_timer_disables_at_deadline() {
        let now = Instant::now();
        let mut state = AwakeState::new(false, false);
        assert!(state.activate_for(now, Duration::from_secs(60)));

        assert!(state.enabled());
        assert!(!state.tick(now + Duration::from_secs(59)));
        assert!(state.tick(now + Duration::from_secs(60)));
        assert!(!state.enabled());
        assert!(!state.is_timed());
    }

    #[test]
    fn pause_timer_reenables_at_deadline() {
        let now = Instant::now();
        let mut state = AwakeState::new(true, false);
        assert!(state.pause_for(now, Duration::from_secs(10)));

        assert!(!state.enabled());
        assert!(state.tick(now + Duration::from_secs(10)));
        assert!(state.enabled());
    }

    #[test]
    fn manual_change_cancels_timer() {
        let now = Instant::now();
        let mut state = AwakeState::new(false, false);
        assert!(state.activate_for(now, Duration::from_secs(60)));
        state.set_enabled(false);

        assert!(!state.is_timed());
        assert!(!state.tick(now + Duration::from_secs(60)));
    }
}
