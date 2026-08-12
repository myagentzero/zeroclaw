use crate::config::EstopConfig;
use crate::security::domain_matcher::DomainMatcher;
use crate::security::otp::OtpValidator;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstopLevel {
    KillAll,
    NetworkKill,
    DomainBlock(Vec<String>),
    ToolFreeze(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeSelector {
    KillAll,
    Network,
    Domains(Vec<String>),
    Tools(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EstopState {
    #[serde(default)]
    pub kill_all: bool,
    #[serde(default)]
    pub network_kill: bool,
    #[serde(default)]
    pub blocked_domains: Vec<String>,
    #[serde(default)]
    pub frozen_tools: Vec<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

impl EstopState {
    pub fn fail_closed() -> Self {
        Self {
            kill_all: true,
            network_kill: false,
            blocked_domains: Vec::new(),
            frozen_tools: Vec::new(),
            updated_at: Some(now_rfc3339()),
        }
    }

    pub fn is_engaged(&self) -> bool {
        self.kill_all
            || self.network_kill
            || !self.blocked_domains.is_empty()
            || !self.frozen_tools.is_empty()
    }

    fn normalize(&mut self) {
        self.blocked_domains = dedup_sort(&self.blocked_domains);
        self.frozen_tools = dedup_sort(&self.frozen_tools);
    }
}

#[derive(Debug, Clone)]
pub struct EstopManager {
    config: EstopConfig,
    state_path: PathBuf,
    state: EstopState,
}

impl EstopManager {
    pub fn load(config: &EstopConfig, config_dir: &Path) -> Result<Self> {
        let state_path = resolve_state_file_path(config_dir, &config.state_file);
        let mut should_fail_closed = false;
        let mut state = if state_path.exists() {
            match fs::read_to_string(&state_path) {
                Ok(raw) => match serde_json::from_str::<EstopState>(&raw) {
                    Ok(mut parsed) => {
                        parsed.normalize();
                        parsed
                    }
                    Err(error) => {
                        tracing::warn!(
                            path = %state_path.display(),
                            "Failed to parse estop state file; entering fail-closed mode: {error}"
                        );
                        should_fail_closed = true;
                        EstopState::fail_closed()
                    }
                },
                Err(error) => {
                    tracing::warn!(
                        path = %state_path.display(),
                        "Failed to read estop state file; entering fail-closed mode: {error}"
                    );
                    should_fail_closed = true;
                    EstopState::fail_closed()
                }
            }
        } else {
            EstopState::default()
        };

        state.normalize();

        let mut manager = Self {
            config: config.clone(),
            state_path,
            state,
        };

        if should_fail_closed {
            let _ = manager.persist_state();
        }

        Ok(manager)
    }

    pub fn state_path(&self) -> &Path {
        &self.state_path
    }

    pub fn status(&self) -> EstopState {
        self.state.clone()
    }

    pub fn engage(&mut self, level: EstopLevel) -> Result<()> {
        match level {
            EstopLevel::KillAll => {
                self.state.kill_all = true;
            }
            EstopLevel::NetworkKill => {
                self.state.network_kill = true;
            }
            EstopLevel::DomainBlock(domains) => {
                for domain in domains {
                    let normalized = domain.trim().to_ascii_lowercase();
                    DomainMatcher::validate_pattern(&normalized)?;
                    self.state.blocked_domains.push(normalized);
                }
            }
            EstopLevel::ToolFreeze(tools) => {
                for tool in tools {
                    let normalized = normalize_tool_name(&tool)?;
                    self.state.frozen_tools.push(normalized);
                }
            }
        }

        self.state.updated_at = Some(now_rfc3339());
        self.state.normalize();
        self.persist_state()
    }

    pub fn resume(
        &mut self,
        selector: ResumeSelector,
        otp_code: Option<&str>,
        otp_validator: Option<&OtpValidator>,
    ) -> Result<()> {
        self.ensure_resume_is_authorized(otp_code, otp_validator)?;

        match selector {
            ResumeSelector::KillAll => {
                self.state.kill_all = false;
            }
            ResumeSelector::Network => {
                self.state.network_kill = false;
            }
            ResumeSelector::Domains(domains) => {
                let normalized = domains
                    .iter()
                    .map(|domain| domain.trim().to_ascii_lowercase())
                    .collect::<Vec<_>>();
                self.state
                    .blocked_domains
                    .retain(|existing| !normalized.iter().any(|target| target == existing));
            }
            ResumeSelector::Tools(tools) => {
                let normalized = tools
                    .iter()
                    .map(|tool| normalize_tool_name(tool))
                    .collect::<Result<Vec<_>>>()?;
                self.state
                    .frozen_tools
                    .retain(|existing| !normalized.iter().any(|target| target == existing));
            }
        }

        self.state.updated_at = Some(now_rfc3339());
        self.state.normalize();
        self.persist_state()
    }

    fn ensure_resume_is_authorized(
        &self,
        otp_code: Option<&str>,
        otp_validator: Option<&OtpValidator>,
    ) -> Result<()> {
        if !self.config.require_otp_to_resume {
            return Ok(());
        }

        let code = otp_code
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("OTP code is required to resume estop state")?;
        let validator = otp_validator
            .context("OTP validator is required to resume estop state with OTP enabled")?;
        let valid = validator.validate(code)?;
        if !valid {
            anyhow::bail!("Invalid OTP code; estop resume denied");
        }
        Ok(())
    }

    fn persist_state(&mut self) -> Result<()> {
        if let Some(parent) = self.state_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create estop state dir {}", parent.display())
            })?;
        }

        let body =
            serde_json::to_string_pretty(&self.state).context("Failed to serialize estop state")?;

        let temp_path = self
            .state_path
            .with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
        fs::write(&temp_path, body).with_context(|| {
            format!(
                "Failed to write temporary estop state file {}",
                temp_path.display()
            )
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600));
        }

        fs::rename(&temp_path, &self.state_path).with_context(|| {
            format!(
                "Failed to atomically replace estop state file {}",
                self.state_path.display()
            )
        })?;

        Ok(())
    }
}

pub fn resolve_state_file_path(config_dir: &Path, state_file: &str) -> PathBuf {
    let expanded = shellexpand::tilde(state_file).into_owned();
    let path = PathBuf::from(expanded);
    if path.is_absolute() {
        path
    } else {
        config_dir.join(path)
    }
}

fn normalize_tool_name(raw: &str) -> Result<String> {
    let value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        anyhow::bail!("Tool name must not be empty");
    }
    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    {
        anyhow::bail!("Tool name '{raw}' contains invalid characters");
    }
    Ok(value)
}

fn dedup_sort(values: &[String]) -> Vec<String> {
    let mut deduped = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    deduped.sort_unstable();
    deduped.dedup();
    deduped
}

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0)
        .unwrap_or(chrono::DateTime::<chrono::Utc>::UNIX_EPOCH)
        .to_rfc3339()
}

/// Tool names treated as network-capable for `network_kill` enforcement.
/// Best-effort static allowlist; MCP-routed tools use dynamic
/// `<server>__<tool>` names and are not covered by this classification.
const NETWORK_TOOL_NAMES: &[&str] = &[
    "web_fetch",
    "http_request",
    "browser",
    "web_search_tool",
    "github",
    "jira",
    "confluence",
    "servicenow",
    "notion",
    "weather",
    "composio",
    "ess_query",
    "delegate",
    "subagent_spawn",
];

fn is_network_tool(normalized_name: &str) -> bool {
    NETWORK_TOOL_NAMES.contains(&normalized_name)
}

fn file_mtime(path: &Path) -> Option<SystemTime> {
    fs::metadata(path).and_then(|meta| meta.modified()).ok()
}

struct GuardState {
    manager: EstopManager,
    last_mtime: Option<SystemTime>,
}

/// Live, shareable handle onto an [`EstopManager`] for use by the running
/// agent runtime (tool loop, gateway, channels, daemon).
///
/// Unlike `EstopManager` — which is loaded fresh for each `agentzero estop`
/// CLI invocation and then dropped — `EstopGuard` is constructed once per
/// long-lived process and cheaply re-reads the on-disk state whenever its
/// mtime changes. This lets a separate `agentzero estop` CLI invocation (a
/// different OS process) take effect in an already-running gateway/daemon
/// without a restart, and without a file-watcher dependency.
pub struct EstopGuard {
    config: EstopConfig,
    config_dir: PathBuf,
    inner: Mutex<GuardState>,
}

impl EstopGuard {
    pub fn load(config: &EstopConfig, config_dir: &Path) -> Result<Self> {
        let manager = EstopManager::load(config, config_dir)?;
        let last_mtime = file_mtime(manager.state_path());
        Ok(Self {
            config: config.clone(),
            config_dir: config_dir.to_path_buf(),
            inner: Mutex::new(GuardState {
                manager,
                last_mtime,
            }),
        })
    }

    fn refresh_locked(&self, guard: &mut GuardState) {
        let current_mtime = file_mtime(guard.manager.state_path());
        if current_mtime == guard.last_mtime {
            return;
        }
        if let Ok(reloaded) = EstopManager::load(&self.config, &self.config_dir) {
            guard.manager = reloaded;
        }
        guard.last_mtime = current_mtime;
    }

    pub fn status(&self) -> EstopState {
        let mut guard = self.inner.lock();
        self.refresh_locked(&mut guard);
        guard.manager.status()
    }

    pub fn engage(&self, level: EstopLevel) -> Result<()> {
        let mut guard = self.inner.lock();
        self.refresh_locked(&mut guard);
        guard.manager.engage(level)?;
        guard.last_mtime = file_mtime(guard.manager.state_path());
        Ok(())
    }

    pub fn resume(
        &self,
        selector: ResumeSelector,
        otp_code: Option<&str>,
        otp_validator: Option<&OtpValidator>,
    ) -> Result<()> {
        let mut guard = self.inner.lock();
        self.refresh_locked(&mut guard);
        guard.manager.resume(selector, otp_code, otp_validator)?;
        guard.last_mtime = file_mtime(guard.manager.state_path());
        Ok(())
    }

    /// Returns a refusal reason if a new agent turn must not start
    /// (kill-all engaged), or `None` if the turn may proceed.
    pub fn check_turn_allowed(&self) -> Option<String> {
        let state = self.status();
        if state.kill_all {
            Some(
                "Emergency stop engaged (kill-all). This agent turn was aborted before \
                 contacting the model provider. Resume via `agentzero estop resume`, the \
                 WebUI, or the Android app."
                    .to_string(),
            )
        } else {
            None
        }
    }

    /// Returns a refusal reason if the named tool must not execute, or
    /// `None` if it may proceed.
    pub fn check_tool(&self, tool_name: &str) -> Option<String> {
        let state = self.status();
        let normalized = tool_name.trim().to_ascii_lowercase();
        if state.kill_all {
            return Some(format!(
                "Emergency stop engaged (kill-all); tool '{tool_name}' was not executed."
            ));
        }
        if state
            .frozen_tools
            .iter()
            .any(|frozen| frozen == &normalized)
        {
            return Some(format!(
                "Tool '{tool_name}' is frozen by emergency stop (tool-freeze)."
            ));
        }
        if state.network_kill && is_network_tool(&normalized) {
            return Some(format!(
                "Tool '{tool_name}' requires network access, which is disabled by \
                 emergency stop (network-kill)."
            ));
        }
        None
    }

    /// Returns a refusal reason if an outbound request to `host` must not
    /// proceed, or `None` if it may proceed.
    pub fn check_domain(&self, host: &str) -> Option<String> {
        let state = self.status();
        if state.network_kill {
            return Some(
                "Network access is disabled by emergency stop (network-kill).".to_string(),
            );
        }
        if state.blocked_domains.is_empty() {
            return None;
        }
        match DomainMatcher::new(&state.blocked_domains, &[]) {
            Ok(matcher) if matcher.is_gated(host) => Some(format!(
                "Host '{host}' is blocked by emergency stop (domain-block)."
            )),
            _ => None,
        }
    }
}

static GLOBAL_ESTOP_GUARD: OnceLock<Arc<EstopGuard>> = OnceLock::new();

/// Registers the process-wide [`EstopGuard`]. Called once at startup (CLI
/// `main`, before subcommand dispatch) when `[security.estop].enabled` is
/// `true`, so the agent tool loop, gateway API, and URL validators can all
/// enforce a live emergency-stop state without threading it through every
/// call site. Only the first call takes effect; later calls are ignored.
pub fn init_global_guard(guard: Arc<EstopGuard>) {
    let _ = GLOBAL_ESTOP_GUARD.set(guard);
}

/// Returns the process-wide [`EstopGuard`] if one was registered via
/// [`init_global_guard`]. Returns `None` when `[security.estop].enabled` is
/// `false` (the default), so callers should treat `None` as "no restriction."
///
/// In test builds this first consults a per-thread override (see
/// [`test_support`]) so tests that engage estop levels only affect code
/// running on their own thread — `cargo test` runs many unrelated tests
/// concurrently on other threads, and those must keep seeing "not engaged".
pub fn global_guard() -> Option<Arc<EstopGuard>> {
    #[cfg(test)]
    {
        if let Some(guard) = test_support::thread_override() {
            return Some(guard);
        }
    }
    GLOBAL_ESTOP_GUARD.get().cloned()
}

/// Test-only helpers for exercising [`EstopGuard`] enforcement from unit
/// tests in other modules (agent loop, tool execution, URL validators,
/// gateway) without leaking engaged state to unrelated tests.
///
/// `cargo test` runs the whole crate's tests in one binary, many
/// concurrently on different threads, and most of those tests exercise the
/// very choke points (`run_tool_call_loop`, `execute_one_tool`, URL
/// validators) this guard feeds into. A single process-wide `OnceLock`
/// guard — engaged by one test — would therefore intermittently break
/// unrelated tests running on other threads at the same moment.
///
/// Instead, [`reset_and_get`] installs a fresh [`EstopGuard`] into a
/// *thread-local* override consulted by [`global_guard`]. Each test's
/// `#[tokio::test]` body (default: single-threaded runtime) and everything
/// it calls run on one OS thread, so the override is only ever visible to
/// that test — never to other tests running concurrently on other threads.
/// The returned [`ScopedGuard`] clears the override on drop so a later,
/// unrelated test reusing the same pooled thread doesn't inherit it.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static TEST_GUARD_OVERRIDE: RefCell<Option<Arc<EstopGuard>>> = const { RefCell::new(None) };
    }

    pub(crate) fn thread_override() -> Option<Arc<EstopGuard>> {
        TEST_GUARD_OVERRIDE.with(|cell| cell.borrow().clone())
    }

    /// RAII handle onto this thread's [`EstopGuard`] override that clears it
    /// when dropped, regardless of test outcome.
    pub(crate) struct ScopedGuard(Arc<EstopGuard>);

    impl std::ops::Deref for ScopedGuard {
        type Target = EstopGuard;
        fn deref(&self) -> &EstopGuard {
            &self.0
        }
    }

    impl Drop for ScopedGuard {
        fn drop(&mut self) {
            TEST_GUARD_OVERRIDE.with(|cell| *cell.borrow_mut() = None);
        }
    }

    /// Installs a fresh, unengaged [`EstopGuard`] as this thread's override
    /// and returns it. `require_otp_to_resume: true` matches the
    /// secure-by-default config so gateway tests can exercise real OTP
    /// enforcement.
    pub(crate) fn reset_and_get() -> ScopedGuard {
        let dir = tempfile::tempdir().expect("create estop test tempdir");
        let dir_path = dir.path().to_path_buf();
        // Leaked deliberately: the guard installed below must outlive this
        // function; it's cleaned up (along with the override) when the
        // returned `ScopedGuard` drops at the end of the test.
        std::mem::forget(dir);
        let cfg = EstopConfig {
            enabled: true,
            state_file: dir_path.join("estop-state.json").display().to_string(),
            require_otp_to_resume: true,
        };
        let guard = Arc::new(EstopGuard::load(&cfg, &dir_path).expect("load test estop guard"));
        TEST_GUARD_OVERRIDE.with(|cell| *cell.borrow_mut() = Some(Arc::clone(&guard)));
        ScopedGuard(guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OtpConfig;
    use crate::security::SecretStore;
    use crate::security::otp::OtpValidator;
    use tempfile::tempdir;

    fn estop_config(path: &Path) -> EstopConfig {
        EstopConfig {
            enabled: true,
            state_file: path.display().to_string(),
            require_otp_to_resume: false,
        }
    }

    #[test]
    fn estop_levels_compose_and_resume() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);
        let mut manager = EstopManager::load(&cfg, dir.path()).unwrap();

        manager
            .engage(EstopLevel::DomainBlock(vec!["*.chase.com".into()]))
            .unwrap();
        manager
            .engage(EstopLevel::ToolFreeze(vec!["shell".into()]))
            .unwrap();
        manager.engage(EstopLevel::NetworkKill).unwrap();
        assert!(manager.status().network_kill);
        assert_eq!(manager.status().blocked_domains, vec!["*.chase.com"]);
        assert_eq!(manager.status().frozen_tools, vec!["shell"]);

        manager
            .resume(
                ResumeSelector::Domains(vec!["*.chase.com".into()]),
                None,
                None,
            )
            .unwrap();
        assert!(manager.status().blocked_domains.is_empty());
        assert!(manager.status().network_kill);

        manager
            .resume(ResumeSelector::Tools(vec!["shell".into()]), None, None)
            .unwrap();
        assert!(manager.status().frozen_tools.is_empty());
    }

    #[test]
    fn estop_state_survives_reload() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);

        {
            let mut manager = EstopManager::load(&cfg, dir.path()).unwrap();
            manager.engage(EstopLevel::KillAll).unwrap();
            manager
                .engage(EstopLevel::DomainBlock(vec!["*.paypal.com".into()]))
                .unwrap();
        }

        let reloaded = EstopManager::load(&cfg, dir.path()).unwrap();
        let state = reloaded.status();
        assert!(state.kill_all);
        assert_eq!(state.blocked_domains, vec!["*.paypal.com"]);
    }

    #[test]
    fn corrupted_state_defaults_to_fail_closed_kill_all() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        fs::write(&state_path, "{not-valid-json").unwrap();
        let cfg = estop_config(&state_path);
        let manager = EstopManager::load(&cfg, dir.path()).unwrap();
        assert!(manager.status().kill_all);
    }

    #[test]
    fn resume_requires_valid_otp_when_enabled() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let mut cfg = estop_config(&state_path);
        cfg.require_otp_to_resume = true;

        let mut manager = EstopManager::load(&cfg, dir.path()).unwrap();
        manager.engage(EstopLevel::KillAll).unwrap();

        let err = manager
            .resume(ResumeSelector::KillAll, None, None)
            .expect_err("resume should require OTP");
        assert!(err.to_string().contains("OTP code is required"));
    }

    #[test]
    fn resume_accepts_valid_otp_code() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let mut cfg = estop_config(&state_path);
        cfg.require_otp_to_resume = true;

        let otp_cfg = OtpConfig {
            enabled: true,
            ..OtpConfig::default()
        };
        let store = SecretStore::new(dir.path(), true);
        let (validator, _) = OtpValidator::from_config(&otp_cfg, dir.path(), &store).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let code = validator.code_for_timestamp(now);

        let mut manager = EstopManager::load(&cfg, dir.path()).unwrap();
        manager.engage(EstopLevel::KillAll).unwrap();
        manager
            .resume(ResumeSelector::KillAll, Some(&code), Some(&validator))
            .unwrap();
        assert!(!manager.status().kill_all);
    }

    #[test]
    fn guard_allows_when_not_engaged() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);
        let guard = EstopGuard::load(&cfg, dir.path()).unwrap();

        assert!(guard.check_turn_allowed().is_none());
        assert!(guard.check_tool("shell").is_none());
        assert!(guard.check_domain("example.com").is_none());
    }

    #[test]
    fn guard_check_turn_allowed_blocks_on_kill_all() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);
        let guard = EstopGuard::load(&cfg, dir.path()).unwrap();

        guard.engage(EstopLevel::KillAll).unwrap();
        let reason = guard
            .check_turn_allowed()
            .expect("turn should be blocked by kill-all");
        assert!(reason.contains("kill-all"));
    }

    #[test]
    fn guard_check_tool_blocks_frozen_tool_only() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);
        let guard = EstopGuard::load(&cfg, dir.path()).unwrap();

        guard
            .engage(EstopLevel::ToolFreeze(vec!["shell".into()]))
            .unwrap();
        assert!(guard.check_tool("shell").is_some());
        assert!(guard.check_tool("file_read").is_none());
    }

    #[test]
    fn guard_check_tool_blocks_network_tools_on_network_kill() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);
        let guard = EstopGuard::load(&cfg, dir.path()).unwrap();

        guard.engage(EstopLevel::NetworkKill).unwrap();
        assert!(guard.check_tool("web_fetch").is_some());
        assert!(guard.check_tool("http_request").is_some());
        assert!(guard.check_tool("file_read").is_none());
    }

    #[test]
    fn guard_check_domain_blocks_matching_pattern() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);
        let guard = EstopGuard::load(&cfg, dir.path()).unwrap();

        guard
            .engage(EstopLevel::DomainBlock(vec!["*.chase.com".into()]))
            .unwrap();
        assert!(guard.check_domain("secure.chase.com").is_some());
        assert!(guard.check_domain("example.com").is_none());
    }

    #[test]
    fn guard_observes_out_of_process_state_file_changes() {
        let dir = tempdir().unwrap();
        let state_path = dir.path().join("estop-state.json");
        let cfg = estop_config(&state_path);
        let guard = EstopGuard::load(&cfg, dir.path()).unwrap();
        assert!(guard.check_turn_allowed().is_none());

        // Simulate a separate `agentzero estop` CLI process mutating the
        // shared state file directly, independent of this guard's Arc.
        // Sleep past filesystems with coarse (1s) mtime resolution so the
        // guard's mtime-change detection reliably observes the write.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let mut other_process_manager = EstopManager::load(&cfg, dir.path()).unwrap();
        other_process_manager.engage(EstopLevel::KillAll).unwrap();

        assert!(guard.check_turn_allowed().is_some());
    }
}
