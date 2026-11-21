// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Free Domain Name contributors

//! Community chat service for the Free Domain Browser.
//!
//! Chat is opt-in per tab. The first enabled panel creates an iroh identity and
//! endpoint; hiding the last panel drops every room and stops traffic. This
//! module keeps all state in memory except the identity (keychain) and the
//! settings file (nickname, profile, ignore list, UI preferences).

pub mod config;
pub mod identity;
pub mod policy;
pub mod protocol;
pub mod room;
pub mod settings;
pub mod transport;

use std::{
    collections::{BTreeMap, BTreeSet, HashSet, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering},
    },
    time::Duration,
};

use iroh::EndpointId;
use iroh_gossip::api::{Event, GossipSender};
use serde::Serialize;
use std::sync::Arc as StdArc;

/// Event sink installed by the CEF chrome. Keeps the transport layer free of
/// platform UI dependencies.
pub type EventCallback = StdArc<dyn Fn(&str, String) + Send + Sync>;

pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("chat runtime")
    })
}

pub fn service() -> &'static StdArc<ChatService> {
    static SERVICE: std::sync::OnceLock<StdArc<ChatService>> = std::sync::OnceLock::new();
    SERVICE.get_or_init(|| StdArc::new(ChatService::new(AppHandle::new(StdArc::new(|_, _| {})))))
}

pub struct AppHandle {
    callback: std::sync::RwLock<EventCallback>,
}

// The sink is swapped at runtime (CEF installs the bridge after startup), so
// clone it from behind the read lock instead of deriving Clone.
impl Clone for AppHandle {
    fn clone(&self) -> Self {
        let callback = self
            .callback
            .read()
            .map(|slot| slot.clone())
            .unwrap_or_else(|_| StdArc::new(|_, _| {}));
        Self {
            callback: std::sync::RwLock::new(callback),
        }
    }
}

impl AppHandle {
    pub fn new(callback: EventCallback) -> Self {
        Self {
            callback: std::sync::RwLock::new(callback),
        }
    }

    /// Swap the event sink after construction. The CEF browser installs the
    /// bridge on first use because `SimpleHandler::instance()` does not exist
    /// yet when the chat singleton is created; until then the default sink is
    /// a no-op and the UI keeps its 3 s snapshot poll as a fallback.
    pub fn set_callback(&self, callback: EventCallback) {
        if let Ok(mut slot) = self.callback.write() {
            *slot = callback;
        }
    }

    fn emit<T: Serialize>(&self, event: &str, payload: T) -> Result<(), ()> {
        let body = serde_json::to_string(&payload).map_err(|_| ())?;
        let sink = self.callback.read().map_err(|_| ())?;
        (sink)(event, body);
        Ok(())
    }
}
use tokio::task::JoinHandle;

use self::{
    config::BootstrapConfig,
    identity::Identity,
    policy::{LimitError, Policy},
    protocol::{
        MAX_TEXT_CHARS, Message, Presence, ProfileCard, ProtocolError, Wire, fingerprint, now_ms,
    },
    settings::{Profile, Settings, SettingsStore},
    transport::Transport,
};

pub const EVENT_STATE: &str = "chat://state";
pub const EVENT_MESSAGE: &str = "chat://message";
pub const EVENT_PRESENCE: &str = "chat://presence";
pub const EVENT_ERROR: &str = "chat://error";
pub const EVENT_ROOMS: &str = "chat://rooms";

const HEARTBEAT: Duration = Duration::from_secs(15);
const PROFILE_COOLDOWN: Duration = Duration::from_secs(5 * 60);
const PRESENCE_TTL_MS: i64 = 45_000;
const MESSAGE_RING: usize = 200;

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error("18+ confirmation is required before enabling chat")]
    AgeGate,
    #[error("chat bootstrap is not configured (missing lobby node ID)")]
    NotConfigured,
    #[error("chat is not enabled")]
    NotEnabled,
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("profile updates are limited; try again in {0} ms")]
    ProfileCooldown(u64),
    #[error("chat transport error: {0}")]
    Transport(String),
    #[error("chat protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("{0}")]
    Limit(#[from] LimitError),
    #[error("chat storage error: {0}")]
    Storage(String),
}

impl Serialize for ChatError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MessageView {
    pub id: String,
    pub sender: String,
    pub nickname: String,
    pub fingerprint: String,
    pub text: String,
    pub timestamp: i64,
    pub mine: bool,
    pub ignored: bool,
    pub profile_version: u32,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct ProfileView {
    pub description: String,
    pub gender: String,
    pub age_band: String,
    pub country: String,
    pub city: String,
}

impl ProfileView {
    fn from_profile(profile: &Profile) -> Self {
        Self {
            description: profile.description.clone(),
            gender: profile.gender.clone(),
            age_band: profile.age_band.clone(),
            country: profile.country.clone(),
            city: profile.city.clone(),
        }
    }

    fn is_empty(&self) -> bool {
        self.description.trim().is_empty()
            && self.gender.trim().is_empty()
            && self.age_band.trim().is_empty()
            && self.country.trim().is_empty()
            && self.city.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserView {
    pub sender: String,
    pub nickname: String,
    pub fingerprint: String,
    pub ignored: bool,
    pub is_me: bool,
    pub last_seen_ms: i64,
    pub profile: Option<ProfileView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub enabled: bool,
    pub connected: bool,
    pub node_id: Option<String>,
    pub fingerprint: Option<String>,
    pub room: String,
    pub online: usize,
    pub users: Vec<UserView>,
    pub messages: Vec<MessageView>,
    pub nickname: String,
    pub cooldown_ms: u64,
    pub profile_cooldown_ms: u64,
    pub age_confirmed: bool,
    pub error: Option<String>,
    pub relay_url: String,
    pub profile: ProfileView,
    pub show_profile_in_rooms: bool,
    /// The signing identity lives in memory only this session because the OS
    /// keychain refused persistence; the UI should say so.
    pub identity_ephemeral: bool,
    /// Monotonic capture counter; responses with a lower seq are stale and
    /// must not overwrite newer state in the UI.
    pub seq: u64,
}

struct Shared {
    node_id: EndpointId,
    room: String,
    relay_url: String,
    nickname: Mutex<String>,
    profile: Mutex<Option<ProfileView>>,
    identity_ephemeral: AtomicBool,
    /// Clone of the service capture counter, so pushed state events get the
    /// same monotonic stamp as command/poll responses.
    next_seq: StdArc<std::sync::atomic::AtomicU64>,
    show_profile: AtomicBool,
    profile_version: AtomicU32,
    connected: AtomicBool,
    error: Mutex<Option<String>>,
    users: Mutex<BTreeMap<String, UserView>>,
    messages: Mutex<VecDeque<MessageView>>,
    policy: Mutex<Policy>,
    last_noise_ms: AtomicI64,
    last_profile_ms: AtomicI64,
}

struct Session {
    transport: Transport,
    sender: GossipSender,
    identity: Arc<Identity>,
    tasks: Vec<JoinHandle<()>>,
    shared: Arc<Shared>,
}

struct Inner {
    enabled_tabs: BTreeSet<String>,
    active_tab: Option<String>,
    session: Option<Session>,
    error: Option<String>,
}

pub struct ChatService {
    app: AppHandle,
    inner: tokio::sync::Mutex<Inner>,
    lifecycle: tokio::sync::Mutex<()>,
    settings: SettingsStore,
    bootstrap: BootstrapConfig,
    /// True when the OS keychain refused persistence this process and the
    /// session fell back to a memory-only identity.
    identity_ephemeral: std::sync::atomic::AtomicBool,
    /// Monotonic capture counter for every snapshot produced by this service.
    /// Responses (poll + command results) and pushed state events carry it so
    /// the UI can drop stale snapshots that would otherwise revert newer
    /// state (last-mutation wins, not last-response).
    next_seq: StdArc<std::sync::atomic::AtomicU64>,
}

impl ChatService {
    /// Swap the event sink after construction (used by the CEF handler).
    pub fn set_callback(&self, callback: EventCallback) {
        self.app.set_callback(callback);
    }

    pub fn new(app: AppHandle) -> Self {
        let config_dir = std::env::var_os("FDN_CHAT_CONFIG_DIR")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                let primary = crate::settings::config_dir();
                let probe = primary.join(".chat-write-probe");
                if std::fs::create_dir_all(&primary).is_ok() && std::fs::write(&probe, b"1").is_ok()
                {
                    let _ = std::fs::remove_file(probe);
                    Some(primary)
                } else {
                    Some(std::env::temp_dir().join("free-domain-browser-cef-chat"))
                }
            });
        let settings = SettingsStore::load(config_dir.as_deref());
        let bootstrap = BootstrapConfig::load(config_dir.as_deref());
        Self {
            app,
            inner: tokio::sync::Mutex::new(Inner {
                enabled_tabs: BTreeSet::new(),
                active_tab: None,
                session: None,
                error: None,
            }),
            lifecycle: tokio::sync::Mutex::new(()),
            settings,
            bootstrap,
            identity_ephemeral: std::sync::atomic::AtomicBool::new(false),
            next_seq: StdArc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub async fn set_enabled(&self, tab_id: String, enabled: bool) -> Result<Snapshot, ChatError> {
        if enabled {
            let settings = self.settings.get();
            if !settings.age_confirmed {
                return Err(ChatError::AgeGate);
            }
            if !self.bootstrap.is_configured() {
                return Err(ChatError::NotConfigured);
            }
        }

        let _lifecycle = self.lifecycle.lock().await;
        {
            let mut inner = self.inner.lock().await;
            if enabled {
                inner.enabled_tabs.insert(tab_id.clone());
                inner.active_tab = Some(tab_id.clone());
            } else {
                inner.enabled_tabs.remove(&tab_id);
                if inner.active_tab.as_deref() == Some(tab_id.as_str()) {
                    inner.active_tab = inner.enabled_tabs.iter().next().cloned();
                }
            }
        }

        let should_run = {
            let inner = self.inner.lock().await;
            !inner.enabled_tabs.is_empty()
        };
        if should_run {
            if let Err(error) = self.start_session().await {
                let mut inner = self.inner.lock().await;
                if inner.session.is_none() {
                    inner.enabled_tabs.remove(&tab_id);
                    inner.error = Some(error.to_string());
                }
                drop(inner);
                let snapshot = self.snapshot().await;
                let _ = self.app.emit(EVENT_STATE, snapshot);
                return Err(error);
            }
        } else {
            self.stop_session().await;
        }

        let _ = self.settings.update(|settings| {
            settings.panel_enabled = should_run;
        });

        Ok(self.snapshot().await)
    }

    async fn start_session(&self) -> Result<(), ChatError> {
        {
            let inner = self.inner.lock().await;
            if inner.session.is_some() {
                return Ok(());
            }
        }

        let settings = self.settings.get();
        let bootstrap = self.bootstrap.clone();
        let identity = if ephemeral_identity() {
            Identity::generate()
        } else {
            // Plain file identity (chat-identity.json next to the settings):
            // the OS keychain caused per-binary authorization prompts on macOS
            // and made both browser builds share one signing identity, so the
            // relay treated the two browsers as the same user.
            let path = identity::identity_file_path(self.settings.dir());
            let loaded = match tokio::task::spawn_blocking(move || {
                Identity::load_or_create(Some(path.as_path()))
            })
            .await
            {
                Ok(result) => result.map_err(|error| ChatError::Storage(error.to_string())),
                Err(error) => Err(ChatError::Storage(error.to_string())),
            };
            match loaded {
                Ok(identity) => identity,
                Err(_error) => {
                    // Unwritable config dir: degrade to a session-local
                    // identity instead of failing chat entirely; the snapshot
                    // flags it as ephemeral so the UI can tell.
                    self.identity_ephemeral
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    Identity::generate()
                }
            }
        };
        let identity = Arc::new(identity);

        let transport = Transport::start(&identity, &bootstrap)
            .await
            .map_err(|error| ChatError::Transport(format!("{error:#}")))?;
        let topic = transport
            .subscribe_lobby()
            .await
            .map_err(|error| ChatError::Transport(format!("{error:#}")))?;
        let (sender, receiver) = topic.split();

        let shared = Arc::new(Shared {
            node_id: identity.node_id,
            room: bootstrap.lobby_room.clone(),
            relay_url: bootstrap.relay_url.clone(),
            nickname: Mutex::new(settings.nickname.clone()),
            profile: Mutex::new(if settings.show_profile_in_rooms {
                Some(ProfileView::from_profile(&settings.profile))
            } else {
                None
            }),
            show_profile: AtomicBool::new(settings.show_profile_in_rooms),
            profile_version: AtomicU32::new(settings.profile_version),
            connected: AtomicBool::new(false),
            error: Mutex::new(None),
            users: Mutex::new(BTreeMap::new()),
            messages: Mutex::new(VecDeque::new()),
            policy: Mutex::new(Policy::new(settings.ignored.clone())),
            last_noise_ms: AtomicI64::new(0),
            last_profile_ms: AtomicI64::new(0),
            identity_ephemeral: AtomicBool::new(
                self.identity_ephemeral
                    .load(std::sync::atomic::Ordering::Relaxed),
            ),
            next_seq: StdArc::clone(&self.next_seq),
        });

        let me = UserView {
            sender: identity.node_id.to_string(),
            nickname: settings.nickname.clone(),
            fingerprint: identity.fingerprint.clone(),
            ignored: false,
            is_me: true,
            last_seen_ms: now_ms(),
            profile: if settings.show_profile_in_rooms {
                Some(ProfileView::from_profile(&settings.profile))
            } else {
                None
            },
        };
        shared
            .users
            .lock()
            .expect("users lock")
            .insert(me.sender.clone(), me);

        let receiver_task = {
            let app = self.app.clone();
            let shared = Arc::clone(&shared);
            let settings = settings.clone();
            tokio::spawn(async move {
                run_receiver(app, shared, receiver, settings).await;
            })
        };
        let heartbeat_task = {
            let app = self.app.clone();
            let shared = Arc::clone(&shared);
            let sender = sender.clone();
            let key = Arc::clone(&identity);
            tokio::spawn(async move {
                run_heartbeat(app, shared, sender, key).await;
            })
        };

        let mut inner = self.inner.lock().await;
        inner.session = Some(Session {
            transport,
            sender,
            identity: Arc::clone(&identity),
            tasks: vec![receiver_task, heartbeat_task],
            shared: Arc::clone(&shared),
        });
        inner.error = None;
        drop(inner);

        emit_state(&self.app, &shared, &settings, true);
        Ok(())
    }

    async fn stop_session(&self) {
        let session = {
            let mut inner = self.inner.lock().await;
            inner.session.take()
        };
        let Some(session) = session else {
            return;
        };
        for task in &session.tasks {
            task.abort();
        }
        for task in session.tasks {
            let _ = task.await;
        }
        drop(session.sender);
        let _ = session.transport.shutdown().await;
        let snapshot = self.snapshot().await;
        let _ = self.app.emit(EVENT_STATE, snapshot);
    }

    pub async fn snapshot(&self) -> Snapshot {
        let settings = self.settings.get();
        let inner = self.inner.lock().await;
        // Stamp while holding the session lock so a snapshot captured after a
        // mutation always carries a higher seq than one captured before it.
        let seq = self
            .next_seq
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let enabled = !inner.enabled_tabs.is_empty();
        match &inner.session {
            Some(session) => snapshot_from(&session.shared, &settings, enabled, seq),
            None => Snapshot {
                enabled,
                connected: false,
                node_id: None,
                fingerprint: None,
                room: self.bootstrap.lobby_room.clone(),
                online: 0,
                users: Vec::new(),
                messages: Vec::new(),
                nickname: settings.nickname.clone(),
                cooldown_ms: 0,
                profile_cooldown_ms: 0,
                age_confirmed: settings.age_confirmed,
                error: inner.error.clone(),
                relay_url: self.bootstrap.relay_url.clone(),
                profile: ProfileView::from_profile(&settings.profile),
                show_profile_in_rooms: settings.show_profile_in_rooms,
                identity_ephemeral: self
                    .identity_ephemeral
                    .load(std::sync::atomic::Ordering::Relaxed),
                seq,
            },
        }
    }

    pub async fn send(&self, text: String) -> Result<(), ChatError> {
        let text = text.trim().to_string();
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(ChatError::Invalid(format!(
                "message is longer than {MAX_TEXT_CHARS} characters"
            )));
        }

        let (sender, shared, key, nickname) = {
            let inner = self.inner.lock().await;
            let session = inner.session.as_ref().ok_or(ChatError::NotEnabled)?;
            let shared = Arc::clone(&session.shared);
            let sender = session.sender.clone();
            let key = Arc::clone(&session.identity);
            let nickname = shared.nickname.lock().expect("nickname lock").clone();
            (sender, shared, key, nickname)
        };

        {
            let mut policy = shared.policy.lock().expect("policy lock");
            policy.check_send(&text)?;
        }

        let mut message = Message::create(
            &shared.room,
            shared.node_id,
            &nickname,
            shared.profile_version.load(Ordering::Relaxed),
            &text,
            None,
        )?;
        message.sign(&key.key);
        let bytes = Wire::Message(message.clone()).encode()?;
        sender
            .broadcast(bytes.into())
            .await
            .map_err(|error| ChatError::Transport(error.to_string()))?;

        let view = MessageView {
            id: message.message_id.clone(),
            sender: message.sender.clone(),
            nickname: message.nickname.clone(),
            fingerprint: fingerprint(&shared.node_id),
            text: message.text.clone(),
            timestamp: message.timestamp,
            mine: true,
            ignored: false,
            profile_version: message.profile_version,
        };
        shared
            .policy
            .lock()
            .expect("policy lock")
            .mark_seen(message.message_id.clone());
        push_message(&self.app, &shared, view);
        emit_state(&self.app, &shared, &self.settings.get(), true);
        Ok(())
    }

    pub async fn set_nickname(&self, nickname: String) -> Result<Snapshot, ChatError> {
        let nickname = nickname.trim().to_string();
        if nickname.is_empty() || nickname.chars().count() > 32 {
            return Err(ChatError::Invalid(
                "nickname must be 1–32 characters".to_string(),
            ));
        }
        self.settings
            .update(|settings| settings.nickname = nickname.clone())
            .map_err(ChatError::Storage)?;

        let shared = {
            let inner = self.inner.lock().await;
            inner
                .session
                .as_ref()
                .map(|session| Arc::clone(&session.shared))
        };
        if let Some(shared) = shared {
            *shared.nickname.lock().expect("nickname lock") = nickname.clone();
            let node_id = shared.node_id;
            if let Some(user) = shared
                .users
                .lock()
                .expect("users lock")
                .get_mut(&node_id.to_string())
            {
                user.nickname = nickname;
                user.last_seen_ms = now_ms();
            }
            let settings = self.settings.get();
            emit_state(&self.app, &shared, &settings, true);
        }
        Ok(self.snapshot().await)
    }

    pub async fn set_profile(
        &self,
        profile: Profile,
        show_in_rooms: bool,
    ) -> Result<Snapshot, ChatError> {
        let profile = profile.normalized();
        profile.validate().map_err(ChatError::Invalid)?;
        if show_in_rooms
            && !profile.is_empty()
            && let Some(shared) = self.session_shared().await
        {
            let left = profile_cooldown_ms(&shared);
            if left > 0 {
                return Err(ChatError::ProfileCooldown(left));
            }
        }
        let next_version = self.settings.get().profile_version.wrapping_add(1);
        self.settings
            .update(|settings| {
                settings.profile = profile.clone();
                settings.show_profile_in_rooms = show_in_rooms;
                settings.profile_version = next_version;
            })
            .map_err(ChatError::Storage)?;

        let session = {
            let inner = self.inner.lock().await;
            inner.session.as_ref().map(|session| {
                (
                    session.sender.clone(),
                    Arc::clone(&session.identity),
                    Arc::clone(&session.shared),
                )
            })
        };
        if let Some((sender, identity, shared)) = session {
            *shared.profile.lock().expect("profile lock") =
                Some(ProfileView::from_profile(&profile));
            shared.show_profile.store(show_in_rooms, Ordering::Relaxed);
            shared
                .profile_version
                .store(next_version, Ordering::Relaxed);
            let nickname = shared.nickname.lock().expect("nickname lock").clone();
            if let Some(user) = shared
                .users
                .lock()
                .expect("users lock")
                .get_mut(&shared.node_id.to_string())
            {
                user.profile = show_in_rooms.then(|| ProfileView::from_profile(&profile));
                user.last_seen_ms = now_ms();
            }
            if show_in_rooms {
                shared.last_profile_ms.store(now_ms(), Ordering::Relaxed);
                if let Ok(mut card) = ProfileCard::create(
                    &shared.room,
                    shared.node_id,
                    &nickname,
                    next_version,
                    &profile.description,
                    &profile.gender,
                    &profile.age_band,
                    &profile.country,
                    &profile.city,
                ) {
                    card.sign(&identity.key);
                    if let Ok(bytes) = Wire::Profile(card).encode() {
                        let _ = sender.broadcast(bytes.into()).await;
                    }
                }
            }
            let settings = self.settings.get();
            emit_state(&self.app, &shared, &settings, true);
        }
        Ok(self.snapshot().await)
    }

    pub async fn confirm_age(&self) -> Result<Snapshot, ChatError> {
        self.settings
            .update(|settings| settings.age_confirmed = true)
            .map_err(ChatError::Storage)?;
        Ok(self.snapshot().await)
    }

    pub async fn set_active_tab(&self, tab_id: Option<String>) -> Snapshot {
        {
            let mut inner = self.inner.lock().await;
            inner.active_tab = tab_id;
        }
        self.snapshot().await
    }

    pub async fn ignore(&self, sender: String) -> Result<Snapshot, ChatError> {
        let sender: EndpointId = sender
            .parse()
            .map_err(|_| ChatError::Invalid("invalid sender ID".to_string()))?;
        let shared = self.session_shared().await.ok_or(ChatError::NotEnabled)?;
        shared.policy.lock().expect("policy lock").ignore(&sender);
        self.settings
            .update(|settings| {
                let value = sender.to_string();
                if !settings.ignored.contains(&value) {
                    settings.ignored.push(value);
                }
            })
            .map_err(ChatError::Storage)?;
        {
            let mut messages = shared.messages.lock().expect("messages lock");
            messages.retain(|message| message.sender != sender.to_string());
        }
        {
            let mut users = shared.users.lock().expect("users lock");
            if let Some(user) = users.get_mut(&sender.to_string()) {
                user.ignored = true;
            }
        }
        emit_presence(&self.app, &shared);
        emit_state(&self.app, &shared, &self.settings.get(), true);
        Ok(self.snapshot().await)
    }

    pub async fn unignore(&self, sender: String) -> Result<Snapshot, ChatError> {
        let sender: EndpointId = sender
            .parse()
            .map_err(|_| ChatError::Invalid("invalid sender ID".to_string()))?;
        let shared = self.session_shared().await.ok_or(ChatError::NotEnabled)?;
        shared.policy.lock().expect("policy lock").unignore(&sender);
        self.settings
            .update(|settings| {
                settings
                    .ignored
                    .retain(|value| value != &sender.to_string())
            })
            .map_err(ChatError::Storage)?;
        {
            let mut users = shared.users.lock().expect("users lock");
            if let Some(user) = users.get_mut(&sender.to_string()) {
                user.ignored = false;
            }
        }
        emit_presence(&self.app, &shared);
        Ok(self.snapshot().await)
    }

    pub async fn clear_local_data(&self) -> Result<Snapshot, ChatError> {
        let _lifecycle = self.lifecycle.lock().await;
        self.stop_session().await;
        // Nothing was written to disk when the session ran with a fallback
        // identity, so skip the delete entirely in that case.
        if !ephemeral_identity()
            && !self
                .identity_ephemeral
                .load(std::sync::atomic::Ordering::Relaxed)
        {
            let path = identity::identity_file_path(self.settings.dir());
            tokio::task::spawn_blocking(move || identity::delete(Some(path.as_path())))
                .await
                .map_err(|error| ChatError::Storage(error.to_string()))?
                .map_err(|error| ChatError::Storage(error.to_string()))?;
        }
        self.settings
            .update(|settings| *settings = Settings::default())
            .map_err(ChatError::Storage)?;
        {
            let mut inner = self.inner.lock().await;
            inner.enabled_tabs.clear();
            inner.active_tab = None;
        }
        let snapshot = self.snapshot().await;
        let _ = self.app.emit(EVENT_STATE, snapshot.clone());
        Ok(snapshot)
    }

    async fn session_shared(&self) -> Option<Arc<Shared>> {
        let inner = self.inner.lock().await;
        inner
            .session
            .as_ref()
            .map(|session| Arc::clone(&session.shared))
    }
}

fn ephemeral_identity() -> bool {
    cfg!(debug_assertions) && std::env::var_os("FDN_CHAT_EPHEMERAL_IDENTITY").is_some()
}

async fn run_receiver(
    app: AppHandle,
    shared: Arc<Shared>,
    mut receiver: iroh_gossip::api::GossipReceiver,
    settings: Settings,
) {
    use n0_future::StreamExt;

    let mut neighbors: HashSet<EndpointId> = HashSet::new();
    let mut warned = false;
    let mut ticker = tokio::time::interval(Duration::from_secs(5));
    ticker.tick().await;

    loop {
        tokio::select! {
            event = receiver.next() => {
                let Some(event) = event else { break };
                match event {
                    Ok(Event::NeighborUp(peer)) => {
                        neighbors.insert(peer);
                        shared.connected.store(true, Ordering::Relaxed);
                        *shared.error.lock().expect("error lock") = None;
                        warned = false;
                        emit_state(&app, &shared, &settings, true);
                    }
                    Ok(Event::NeighborDown(peer)) => {
                        neighbors.remove(&peer);
                        if neighbors.is_empty() {
                            shared.connected.store(false, Ordering::Relaxed);
                        }
                        emit_state(&app, &shared, &settings, true);
                    }
                    Ok(Event::Received(message)) => {
                        handle_payload(&app, &shared, &message.content, &settings);
                    }
                    Ok(Event::Lagged) => {
                        emit_error(&app, "chat receiver fell behind; presence may be stale");
                    }
                    Ok(_) => {}
                    Err(error) => {
                        *shared.error.lock().expect("error lock") =
                            Some(format!("chat stream ended: {error}"));
                        emit_error(&app, &format!("chat stream ended: {error}"));
                        break;
                    }
                }
            }
            _ = ticker.tick() => {
                if neighbors.is_empty() && !warned {
                    warned = true;
                    let message = "connecting… relay has not answered yet";
                    *shared.error.lock().expect("error lock") = Some(message.to_string());
                    emit_error(&app, message);
                    emit_state(&app, &shared, &settings, true);
                }
            }
        }
    }
    shared.connected.store(false, Ordering::Relaxed);
    emit_state(&app, &shared, &settings, true);
}

fn handle_payload(app: &AppHandle, shared: &Arc<Shared>, content: &[u8], settings: &Settings) {
    let wire = match Wire::decode(content) {
        Ok(wire) => wire,
        Err(error) => {
            emit_error(app, &format!("dropped malformed chat envelope: {error}"));
            return;
        }
    };
    let now = now_ms();

    match wire {
        Wire::Message(message) => {
            if message.room != shared.room {
                return;
            }
            let sender = match message.verify(now) {
                Ok(sender) => sender,
                Err(error) => {
                    emit_error(app, &format!("dropped unsigned message: {error}"));
                    return;
                }
            };
            {
                let mut policy = shared.policy.lock().expect("policy lock");
                if policy.is_ignored(&sender) {
                    return;
                }
                if let Err(error) = policy.check_receive(sender, &message.message_id, &message.text)
                {
                    if matches!(error, LimitError::RoomNoisy) {
                        let last = shared.last_noise_ms.load(Ordering::Relaxed);
                        if now - last > 5_000 {
                            shared.last_noise_ms.store(now, Ordering::Relaxed);
                            emit_error(app, "room is noisy; some messages were dropped");
                        }
                    }
                    return;
                }
            }

            let view = MessageView {
                id: message.message_id.clone(),
                sender: message.sender.clone(),
                nickname: message.nickname.clone(),
                fingerprint: fingerprint(&sender),
                text: message.text.clone(),
                timestamp: message.timestamp,
                mine: sender == shared.node_id,
                ignored: false,
                profile_version: message.profile_version,
            };
            touch_user(shared, sender, &message.nickname, None);
            push_message(app, shared, view);
            emit_state(app, shared, settings, true);
        }
        Wire::Presence(presence) => {
            if presence.room != shared.room {
                return;
            }
            let sender = match presence.verify(now) {
                Ok(sender) => sender,
                Err(error) => {
                    emit_error(app, &format!("dropped unsigned presence: {error}"));
                    return;
                }
            };
            let ignored = shared
                .policy
                .lock()
                .expect("policy lock")
                .is_ignored(&sender);
            touch_user(shared, sender, &presence.nickname, Some(ignored));
            emit_presence(app, shared);
            emit_state(app, shared, settings, true);
        }
        Wire::Profile(card) => {
            if card.room != shared.room {
                return;
            }
            let sender = match card.verify(now) {
                Ok(sender) => sender,
                Err(error) => {
                    emit_error(app, &format!("dropped unsigned profile: {error}"));
                    return;
                }
            };
            let profile = ProfileView {
                description: card.description.clone(),
                gender: card.gender.clone(),
                age_band: card.age_band.clone(),
                country: card.country.clone(),
                city: card.city.clone(),
            };
            touch_user(shared, sender, &card.nickname, None);
            if let Some(user) = shared
                .users
                .lock()
                .expect("users lock")
                .get_mut(&sender.to_string())
            {
                user.nickname = card.nickname.clone();
                user.profile = Some(profile);
                user.last_seen_ms = now;
            }
            emit_presence(app, shared);
            emit_state(app, shared, settings, true);
        }
    }
}

fn touch_user(shared: &Shared, sender: EndpointId, nickname: &str, ignored: Option<bool>) {
    let now = now_ms();
    let mut users = shared.users.lock().expect("users lock");
    let entry = users.entry(sender.to_string()).or_insert_with(|| UserView {
        sender: sender.to_string(),
        nickname: nickname.to_string(),
        fingerprint: fingerprint(&sender),
        ignored: false,
        is_me: sender == shared.node_id,
        last_seen_ms: now,
        profile: None,
    });
    entry.nickname = nickname.to_string();
    entry.last_seen_ms = now;
    if let Some(ignored) = ignored {
        entry.ignored = ignored;
    }
}

async fn run_heartbeat(
    app: AppHandle,
    shared: Arc<Shared>,
    sender: GossipSender,
    identity: Arc<Identity>,
) {
    loop {
        let nickname = shared.nickname.lock().expect("nickname lock").clone();
        let presence = Presence::create(
            &shared.room,
            shared.node_id,
            &nickname,
            shared.profile_version.load(Ordering::Relaxed),
        );
        match presence {
            Ok(mut presence) => {
                presence.sign(&identity.key);
                match Wire::Presence(presence).encode() {
                    Ok(bytes) => {
                        if let Err(error) = sender.broadcast(bytes.into()).await {
                            let message = format!("presence send failed: {error}");
                            *shared.error.lock().expect("error lock") = Some(message.clone());
                            emit_error(&app, &message);
                        }
                    }
                    Err(error) => emit_error(&app, &format!("presence encode failed: {error}")),
                }
            }
            Err(error) => emit_error(&app, &format!("presence create failed: {error}")),
        }

        if shared.show_profile.load(Ordering::Relaxed) {
            let nickname = shared.nickname.lock().expect("nickname lock").clone();
            let profile = shared.profile.lock().expect("profile lock").clone();
            if let Some(profile) = profile.filter(|profile| !profile.is_empty()) {
                match ProfileCard::create(
                    &shared.room,
                    shared.node_id,
                    &nickname,
                    shared.profile_version.load(Ordering::Relaxed),
                    &profile.description,
                    &profile.gender,
                    &profile.age_band,
                    &profile.country,
                    &profile.city,
                ) {
                    Ok(mut card) => {
                        card.sign(&identity.key);
                        match Wire::Profile(card).encode() {
                            Ok(bytes) => {
                                if let Err(error) = sender.broadcast(bytes.into()).await {
                                    emit_error(&app, &format!("profile send failed: {error}"));
                                }
                            }
                            Err(error) => {
                                emit_error(&app, &format!("profile encode failed: {error}"));
                            }
                        }
                    }
                    Err(error) => emit_error(&app, &format!("profile create failed: {error}")),
                }
            }
        }

        let now = now_ms();
        let changed = {
            let mut users = shared.users.lock().expect("users lock");
            let before = users.len();
            users.retain(|_, user| {
                user.is_me || now.saturating_sub(user.last_seen_ms) <= PRESENCE_TTL_MS
            });
            users.len() != before
        };
        if changed {
            emit_presence(&app, &shared);
        }

        tokio::time::sleep(HEARTBEAT).await;
    }
}

fn push_message(app: &AppHandle, shared: &Shared, view: MessageView) {
    {
        let mut messages = shared.messages.lock().expect("messages lock");
        messages.push_back(view.clone());
        while messages.len() > MESSAGE_RING {
            messages.pop_front();
        }
    }
    let _ = app.emit(EVENT_MESSAGE, view);
}

fn emit_presence(app: &AppHandle, shared: &Shared) {
    let users = sorted_users(shared);
    let _ = app.emit(
        EVENT_PRESENCE,
        serde_json::json!({
            "room": shared.room,
            "users": users,
        }),
    );
}

fn profile_cooldown_ms(shared: &Shared) -> u64 {
    let last = shared.last_profile_ms.load(Ordering::Relaxed);
    if last <= 0 {
        return 0;
    }
    let elapsed = now_ms().saturating_sub(last).max(0) as u64;
    (PROFILE_COOLDOWN.as_millis() as u64).saturating_sub(elapsed)
}

fn emit_state(app: &AppHandle, shared: &Shared, settings: &Settings, enabled: bool) {
    let seq = shared.next_seq.fetch_add(1, Ordering::Relaxed);
    let snapshot = snapshot_from(shared, settings, enabled, seq);
    let _ = app.emit(EVENT_STATE, snapshot);
}

fn emit_error(app: &AppHandle, message: &str) {
    let _ = app.emit(EVENT_ERROR, serde_json::json!({ "message": message }));
}

fn snapshot_from(shared: &Shared, settings: &Settings, enabled: bool, seq: u64) -> Snapshot {
    let users = sorted_users(shared);
    let online = users.iter().filter(|user| !user.is_me).count();
    let messages = shared
        .messages
        .lock()
        .expect("messages lock")
        .iter()
        .cloned()
        .collect();
    Snapshot {
        enabled,
        connected: shared.connected.load(Ordering::Relaxed),
        node_id: Some(shared.node_id.to_string()),
        fingerprint: Some(fingerprint(&shared.node_id)),
        room: shared.room.clone(),
        online,
        users,
        messages,
        nickname: shared.nickname.lock().expect("nickname lock").clone(),
        cooldown_ms: shared
            .policy
            .lock()
            .expect("policy lock")
            .send_cooldown_ms(),
        profile_cooldown_ms: profile_cooldown_ms(shared),
        age_confirmed: settings.age_confirmed,
        error: shared.error.lock().expect("error lock").clone(),
        relay_url: shared.relay_url.clone(),
        profile: ProfileView::from_profile(&settings.profile),
        show_profile_in_rooms: settings.show_profile_in_rooms,
        identity_ephemeral: shared.identity_ephemeral.load(Ordering::Relaxed),
        seq,
    }
}

fn sorted_users(shared: &Shared) -> Vec<UserView> {
    let mut users: Vec<UserView> = shared
        .users
        .lock()
        .expect("users lock")
        .values()
        .cloned()
        .collect();
    users.sort_by(|a, b| {
        b.is_me
            .cmp(&a.is_me)
            .then_with(|| a.nickname.to_lowercase().cmp(&b.nickname.to_lowercase()))
            .then_with(|| a.sender.cmp(&b.sender))
    });
    users
}
