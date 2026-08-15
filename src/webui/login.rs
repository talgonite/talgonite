use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use bevy::tasks::{IoTaskPool, Task};
use futures_lite::future;
use game_ui::{CoreToUi, LoginError, UiToCore};

use crate::app_state::AppState;
use crate::settings_types::{
    SavedCredential, SavedCredentialPublic, ServerEntry, Settings as SettingsFile,
};
use crate::webui::input::InputBindingResources;
use crate::webui::plugin::{UiInbound, UiOutbound};
use crate::webui::settings::{
    apply_modifier_rows_change, apply_rebind_key, apply_scale_input_change, apply_settings_change,
    apply_unbind_key, apply_volume_change, write_snapshot_and_sync,
};

use super::keyring;

pub(crate) fn handle_ui_inbound_login(
    mut inbound: MessageReader<UiInbound>,
    mut outbound: MessageWriter<UiOutbound>,
    mut settings: ResMut<SettingsFile>,
    mut commands: Commands,
    mut prelogin_state: ResMut<PreLoginConnectionState>,
    storage_config: Res<crate::resources::StorageConfig>,
    bindings: InputBindingResources,
    mut char_preview_state: ResMut<crate::resources::CharacterCreatorPreviewState>,
) {
    let mut input_bindings = bindings.input_bindings;
    let mut unified_bindings = bindings.unified_bindings;

    for UiInbound(msg) in inbound.read() {
        match msg {
            UiToCore::InputKeyboard { .. } | UiToCore::InputPointer { .. } => {}
            UiToCore::ExitApplication => {
                let _ = slint::quit_event_loop();
            }
            UiToCore::ReturnToMainMenu => {}
            UiToCore::RequestSnapshot => {
                write_snapshot_and_sync(&mut outbound, &settings);
                ensure_selected_prelogin_connection(
                    &mut commands,
                    &mut prelogin_state,
                    &settings,
                    false,
                );
            }
            UiToCore::LoginSubmit {
                server_id,
                username,
                password,
                remember,
            } => {
                println!(
                    "[webui] LoginSubmit: server_id={:?} username={}",
                    server_id, username
                );
                // Stay on the login screen and start background login task
                let server = settings
                    .servers
                    .iter()
                    .find(|s| s.id == *server_id)
                    .cloned();
                if let Some(server) = server {
                    let uname = username.clone();
                    let pw = password.clone();
                    let remember = *remember;
                    let cred_id = format!("{}:{}", server.id, uname);
                    let request = PendingLoginRequest {
                        remember,
                        cred_id,
                        server_id: server.id,
                        username: uname,
                        password: pw,
                    };

                    match start_or_queue_login(
                        &mut commands,
                        &mut prelogin_state,
                        server.id,
                        request,
                    ) {
                        Ok(LoginStartOutcome::Started) => {
                            outbound.write(UiOutbound(settings.to_snapshot_message(None)));
                        }
                        Ok(LoginStartOutcome::Queued) => {}
                        Err(err) => {
                            outbound.write(UiOutbound(settings.to_snapshot_message(Some(err))));
                        }
                    }
                } else {
                    println!(
                        "[webui] LoginSubmit: server id {} not found in settings",
                        server_id
                    );
                }
            }
            UiToCore::LoginUseSaved { id } => {
                println!("[webui] LoginUseSaved: id={}", id);
                let mut emitted_snapshot = false;
                let (cred_id, server_id, username) = {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    if let Some(c) = settings.saved_credentials.iter_mut().find(|c| &c.id == id) {
                        c.last_used = now;
                        (c.id.clone(), c.server_id, c.username.clone())
                    } else {
                        println!("[webui] LoginUseSaved: credential id not found");
                        continue;
                    }
                };
                match keyring::get_password(&cred_id) {
                    Ok(password) => {
                        if settings.servers.iter().any(|s| s.id == server_id) {
                            println!(
                                "[webui] LoginUseSaved: starting background login for server {}",
                                server_id
                            );
                            let uname = username.clone();
                            let request = PendingLoginRequest {
                                remember: false,
                                cred_id,
                                server_id,
                                username: uname,
                                password,
                            };

                            match start_or_queue_login(
                                &mut commands,
                                &mut prelogin_state,
                                server_id,
                                request,
                            ) {
                                Ok(LoginStartOutcome::Started | LoginStartOutcome::Queued) => {}
                                Err(err) => {
                                    outbound
                                        .write(UiOutbound(settings.to_snapshot_message(Some(err))));
                                    emitted_snapshot = true;
                                }
                            }
                        } else {
                            println!(
                                "[webui] LoginUseSaved: server {} not found in settings",
                                server_id
                            );
                            outbound.write(UiOutbound(settings.to_snapshot_message(Some(
                                LoginError::Network("Server missing".to_string()),
                            ))));
                            emitted_snapshot = true;
                        }
                    }
                    Err(err) => {
                        println!(
                            "[webui] LoginUseSaved: keyring missing password for id={} ({}). Prompting user to re-enter.",
                            cred_id, err
                        );
                        outbound.write(UiOutbound(settings.to_snapshot_message(Some(
                            LoginError::Network("Missing saved password".to_string()),
                        ))));
                        emitted_snapshot = true;
                    }
                }
                if !emitted_snapshot {
                    let logins_public: Vec<SavedCredentialPublic> =
                        settings.saved_credentials.iter().map(to_public).collect();
                    outbound.write(UiOutbound(CoreToUi::Snapshot {
                        servers: settings.servers.clone(),
                        current_server_id: settings.gameplay.current_server_id,
                        logins: logins_public,
                        login_error: None,
                    }));
                }
            }
            UiToCore::LoginRemoveSaved { id } => {
                let _ = keyring::delete_password(id);
                settings.remove_credential(id, &storage_config);
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
            }
            UiToCore::CharacterCreationSubmit {
                server_id,
                username,
                password,
                save_login,
            } => {
                println!(
                    "[webui] CharacterCreationSubmit: server_id={:?} username={} gender={} hair_style={} hair_color={}",
                    server_id,
                    username,
                    char_preview_state.gender,
                    char_preview_state.hair_style,
                    char_preview_state.hair_color
                );
                let server = settings
                    .servers
                    .iter()
                    .find(|s| s.id == *server_id)
                    .cloned();
                if let Some(server) = server {
                    let uname = username.clone();
                    let pw = password.clone();
                    let uname_task = uname.clone();
                    let pw_task = pw.clone();
                    let gender_task = char_preview_state.gender;
                    let hair_style_task = char_preview_state.hair_style;
                    let hair_color_task = char_preview_state.hair_color;
                    let gender_task = match packets::client::CharGender::try_from(gender_task) {
                        Ok(gender) => gender,
                        Err(_) => {
                            outbound.write(UiOutbound(
                                settings.to_snapshot_message(Some(LoginError::Unknown)),
                            ));
                            continue;
                        }
                    };
                    let cached_session = prelogin_state.session.clone();
                    match prelogin_state.take_session(server.id) {
                        Ok(mut lobby) => {
                            let task = IoTaskPool::get().spawn(async move {
                                let result = lobby
                                    .create_character(
                                        &uname_task,
                                        &pw_task,
                                        hair_style_task,
                                        gender_task,
                                        hair_color_task,
                                    )
                                    .await;

                                if let Ok(mut session) = cached_session.lock() {
                                    *session = Some(lobby);
                                }

                                result
                            });
                            commands.spawn(CharacterCreationTaskEntity(
                                CharacterCreationTaskInner {
                                    task,
                                    save_login: *save_login,
                                    server_id: server.id,
                                    username: uname,
                                    password: Some(pw),
                                },
                            ));
                        }
                        Err(err) => {
                            outbound.write(UiOutbound(settings.to_snapshot_message(Some(err))));
                        }
                    }
                } else {
                    println!(
                        "[webui] CharacterCreationSubmit: server id {} not found",
                        server_id
                    );
                }
            }
            UiToCore::UpdateCharacterCreationPreview {
                gender,
                hair_style,
                hair_color,
                armor_id,
            } => {
                char_preview_state.gender = *gender;
                char_preview_state.hair_style = *hair_style;
                char_preview_state.hair_color = *hair_color;
                char_preview_state.armor_id = *armor_id;
                char_preview_state.dirty = true;
            }
            UiToCore::ServersChangeCurrent { id } => {
                settings.gameplay.current_server_id = Some(*id);
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
                ensure_selected_prelogin_connection(
                    &mut commands,
                    &mut prelogin_state,
                    &settings,
                    true,
                );
            }
            UiToCore::ServersAdd { server } => {
                let new_id = next_id(settings.servers.iter().map(|s| s.id));
                settings.servers.push(ServerEntry {
                    id: new_id,
                    name: server.name.clone(),
                    address: server.address.clone(),
                });
                if settings.gameplay.current_server_id.is_none() {
                    settings.gameplay.current_server_id = Some(new_id);
                }
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
                ensure_selected_prelogin_connection(
                    &mut commands,
                    &mut prelogin_state,
                    &settings,
                    true,
                );
            }
            UiToCore::ServersEdit { server } => {
                if let Some(s) = settings.servers.iter_mut().find(|s| s.id == server.id) {
                    s.name = server.name.clone();
                    s.address = server.address.clone();
                }
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
                ensure_selected_prelogin_connection(
                    &mut commands,
                    &mut prelogin_state,
                    &settings,
                    settings.gameplay.current_server_id == Some(server.id),
                );
            }
            UiToCore::ServersRemove { id } => {
                settings.servers.retain(|s| s.id != *id);
                if settings.gameplay.current_server_id == Some(*id) {
                    settings.gameplay.current_server_id = settings.servers.first().map(|s| s.id);
                }
                outbound.write(UiOutbound(settings.to_snapshot_message(None)));
                ensure_selected_prelogin_connection(
                    &mut commands,
                    &mut prelogin_state,
                    &settings,
                    true,
                );
            }
            UiToCore::SettingsChange { xray_size } => {
                apply_settings_change(*xray_size, &mut settings);
            }
            UiToCore::VolumeChange { sfx, music } => {
                apply_volume_change(*sfx, *music, &mut settings);
            }
            UiToCore::ScaleInputChange { progress } => {
                apply_scale_input_change(*progress, &mut settings);
            }
            UiToCore::ModifierHotbarRowsTargetCustomOnlyChange { enabled } => {
                apply_modifier_rows_change(*enabled, &mut settings);
            }
            UiToCore::RebindKey {
                action,
                new_key,
                index,
            } => {
                apply_rebind_key(
                    action,
                    new_key,
                    *index,
                    &mut settings,
                    &mut input_bindings,
                    &mut unified_bindings,
                );
            }
            UiToCore::UnbindKey { action, index } => {
                apply_unbind_key(
                    action,
                    *index,
                    &mut settings,
                    &mut input_bindings,
                    &mut unified_bindings,
                );
            }
            _ => {}
        }
    }
}

fn next_id(mut iter: impl Iterator<Item = u32>) -> u32 {
    let mut max = 0u32;
    while let Some(v) = iter.next() {
        if v > max {
            max = v;
        }
    }
    max.saturating_add(1)
}

#[derive(Debug, Clone, Default)]
enum PreLoginConnectionStatus {
    #[default]
    Idle,
    Connecting,
    Ready,
    Busy,
    Failed(LoginError),
}

#[derive(Resource, Clone)]
pub(crate) struct PreLoginConnectionState {
    server_id: Option<u32>,
    status: PreLoginConnectionStatus,
    session: Arc<Mutex<Option<crate::session_prelogin::PreLoginSession>>>,
    pending_login: Option<PendingLoginRequest>,
}

#[derive(Clone)]
struct PendingLoginRequest {
    remember: bool,
    cred_id: String,
    server_id: u32,
    username: String,
    password: String,
}

impl Default for PreLoginConnectionState {
    fn default() -> Self {
        Self {
            server_id: None,
            status: PreLoginConnectionStatus::Idle,
            session: Arc::new(Mutex::new(None)),
            pending_login: None,
        }
    }
}

impl PreLoginConnectionState {
    fn clear(&mut self) {
        self.server_id = None;
        self.status = PreLoginConnectionStatus::Idle;
        self.pending_login = None;
        if let Ok(mut session) = self.session.lock() {
            *session = None;
        }
    }

    fn has_session(&self) -> bool {
        self.session
            .lock()
            .map(|session| session.is_some())
            .unwrap_or(false)
    }

    fn mark_connecting(&mut self, server_id: u32) {
        if self.server_id != Some(server_id) {
            self.pending_login = None;
        }
        self.server_id = Some(server_id);
        self.status = PreLoginConnectionStatus::Connecting;
        if let Ok(mut session) = self.session.lock() {
            *session = None;
        }
    }

    fn mark_ready(&mut self, server_id: u32) {
        self.server_id = Some(server_id);
        self.status = PreLoginConnectionStatus::Ready;
    }

    fn mark_failed(&mut self, server_id: u32, error: LoginError) {
        self.server_id = Some(server_id);
        self.status = PreLoginConnectionStatus::Failed(error);
        self.pending_login = None;
        if let Ok(mut session) = self.session.lock() {
            *session = None;
        }
    }

    fn set_pending_login(&mut self, request: PendingLoginRequest) {
        self.pending_login = Some(request);
    }

    fn take_pending_login(&mut self, server_id: u32) -> Option<PendingLoginRequest> {
        if self.server_id == Some(server_id) {
            self.pending_login.take()
        } else {
            None
        }
    }

    fn take_session(
        &mut self,
        server_id: u32,
    ) -> Result<crate::session_prelogin::PreLoginSession, LoginError> {
        if self.server_id != Some(server_id) {
            return Err(LoginError::Network(
                "Selected server connection is stale".to_string(),
            ));
        }

        let mut session = self
            .session
            .lock()
            .map_err(|_| LoginError::Network("Prelogin session lock poisoned".to_string()))?;

        match session.take() {
            Some(session) => {
                self.status = PreLoginConnectionStatus::Busy;
                Ok(session)
            }
            None => Err(match &self.status {
                PreLoginConnectionStatus::Connecting => {
                    LoginError::Network("Connecting to login server".to_string())
                }
                PreLoginConnectionStatus::Busy => {
                    LoginError::Network("Login server session is busy".to_string())
                }
                PreLoginConnectionStatus::Failed(error) => error.clone(),
                _ => LoginError::Network("Login server is not connected".to_string()),
            }),
        }
    }
}

#[derive(Component)]
pub(crate) struct PreLoginConnectTaskEntity {
    server_id: u32,
    task: Task<Result<crate::session_prelogin::PreLoginSession, LoginError>>,
}

fn selected_server(settings: &SettingsFile) -> Option<ServerEntry> {
    let effective_server_id = settings
        .gameplay
        .current_server_id
        .or_else(|| settings.servers.first().map(|server| server.id));

    effective_server_id.and_then(|server_id| {
        settings
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .cloned()
    })
}

fn spawn_prelogin_connect_task(
    commands: &mut Commands,
    prelogin_state: &mut PreLoginConnectionState,
    server: &ServerEntry,
    force: bool,
) {
    let already_ready = prelogin_state.server_id == Some(server.id)
        && matches!(
            prelogin_state.status,
            PreLoginConnectionStatus::Connecting | PreLoginConnectionStatus::Ready
        )
        && (prelogin_state.has_session()
            || matches!(prelogin_state.status, PreLoginConnectionStatus::Connecting));

    if already_ready && !force {
        return;
    }

    let (host, port) = parse_host_port(&server.address).unwrap_or((server.address.clone(), 2610));
    prelogin_state.mark_connecting(server.id);
    let server_id = server.id;
    let task = IoTaskPool::get().spawn(async move {
        crate::session_prelogin::PreLoginSession::new(&host, port)
            .await
            .map_err(|error| LoginError::Network(error.to_string()))
    });

    commands.spawn(PreLoginConnectTaskEntity { server_id, task });
}

fn ensure_selected_prelogin_connection(
    commands: &mut Commands,
    prelogin_state: &mut PreLoginConnectionState,
    settings: &SettingsFile,
    force: bool,
) {
    let Some(server) = selected_server(settings) else {
        prelogin_state.clear();
        return;
    };

    spawn_prelogin_connect_task(commands, prelogin_state, &server, force);
}

fn spawn_login_task(
    commands: &mut Commands,
    lobby: crate::session_prelogin::PreLoginSession,
    request: PendingLoginRequest,
) {
    let uname_task = request.username.clone();
    let pw_task = request.password.clone();
    let task: Task<Result<(network::DecryptedReceiver, network::EncryptedSender), LoginError>> =
        IoTaskPool::get().spawn(async move {
            match lobby.login(&uname_task, &pw_task).await {
                Ok((rx, tx)) => Ok((rx, tx)),
                Err(code) => Err(code),
            }
        });

    commands.spawn(LoginTaskEntity(LoginTaskInner {
        task,
        remember: request.remember,
        cred_id: request.cred_id,
        server_id: request.server_id,
        username: request.username,
        password: Some(request.password),
    }));
}

enum LoginStartOutcome {
    Started,
    Queued,
}

fn start_or_queue_login(
    commands: &mut Commands,
    prelogin_state: &mut PreLoginConnectionState,
    server_id: u32,
    request: PendingLoginRequest,
) -> Result<LoginStartOutcome, LoginError> {
    if prelogin_state.server_id != Some(server_id) {
        return Err(LoginError::Network(
            "Selected server connection is stale".to_string(),
        ));
    }

    if matches!(prelogin_state.status, PreLoginConnectionStatus::Connecting) {
        prelogin_state.set_pending_login(request);
        return Ok(LoginStartOutcome::Queued);
    }

    let lobby = prelogin_state.take_session(server_id)?;
    spawn_login_task(commands, lobby, request);
    Ok(LoginStartOutcome::Started)
}

pub(crate) fn handle_prelogin_connect_tasks(
    mut commands: Commands,
    mut prelogin_state: ResMut<PreLoginConnectionState>,
    settings: Res<SettingsFile>,
    mut outbound: MessageWriter<UiOutbound>,
    mut q: Query<(Entity, &mut PreLoginConnectTaskEntity)>,
) {
    for (entity, mut task_wrap) in &mut q {
        if let Some(result) = future::block_on(future::poll_once(&mut task_wrap.task)) {
            match result {
                Ok(session) => {
                    if prelogin_state.server_id == Some(task_wrap.server_id) {
                        if let Ok(mut cached) = prelogin_state.session.lock() {
                            *cached = Some(session);
                        }
                        prelogin_state.mark_ready(task_wrap.server_id);
                        if let Some(request) =
                            prelogin_state.take_pending_login(task_wrap.server_id)
                        {
                            if let Ok(lobby) = prelogin_state.take_session(task_wrap.server_id) {
                                spawn_login_task(&mut commands, lobby, request);
                            }
                        }
                    }
                }
                Err(error) => {
                    if prelogin_state.server_id == Some(task_wrap.server_id) {
                        let had_pending_login = prelogin_state.pending_login.is_some();
                        prelogin_state.mark_failed(task_wrap.server_id, error.clone());
                        if had_pending_login {
                            let logins_public: Vec<SavedCredentialPublic> =
                                settings.saved_credentials.iter().map(to_public).collect();
                            outbound.write(UiOutbound(CoreToUi::Snapshot {
                                servers: settings.servers.clone(),
                                current_server_id: settings.gameplay.current_server_id,
                                logins: logins_public,
                                login_error: Some(error),
                            }));
                        }
                    }
                }
            }
            commands.entity(entity).despawn();
        }
    }
}

fn to_public(c: &SavedCredential) -> SavedCredentialPublic {
    SavedCredentialPublic {
        id: c.id.clone(),
        server_id: c.server_id,
        username: c.username.clone(),
        last_used: c.last_used,
        preview: c.preview.clone(),
    }
}

#[derive(Component)]
pub(crate) struct CharacterCreationTaskEntity(CharacterCreationTaskInner);

struct CharacterCreationTaskInner {
    task: Task<Result<(), LoginError>>,
    save_login: bool,
    server_id: u32,
    username: String,
    password: Option<String>,
}

#[derive(Component)]
pub(crate) struct CharacterCreationResultComp(Option<CharacterCreationTaskInner>);

#[derive(Component)]
pub(crate) struct CharacterCreationErrorComp(LoginError);

#[derive(Component)]
pub(crate) struct LoginTaskEntity(LoginTaskInner);

struct LoginTaskInner {
    task: Task<Result<(network::DecryptedReceiver, network::EncryptedSender), LoginError>>,
    remember: bool,
    cred_id: String,
    server_id: u32,
    username: String,
    password: Option<String>,
}

#[derive(Component)]
pub(crate) struct LoginResultComp(
    Option<network::DecryptedReceiver>,
    Option<network::EncryptedSender>,
    Option<LoginTaskInner>,
);

#[derive(Component)]
pub(crate) struct LoginErrorComp(LoginError, LoginTaskInner);

pub(crate) fn handle_login_tasks(
    mut commands: Commands,
    mut q: Query<(Entity, &mut LoginTaskEntity)>,
) {
    for (e, mut task_wrap) in &mut q {
        if let Some(res) = future::block_on(future::poll_once(&mut task_wrap.0.task)) {
            println!("[webui] LoginTask completed: success={}.", res.is_ok());
            let inner = std::mem::replace(
                &mut task_wrap.0,
                LoginTaskInner {
                    task: IoTaskPool::get().spawn(async { Err(LoginError::Unknown) }),
                    remember: false,
                    cred_id: String::new(),
                    server_id: 0,
                    username: String::new(),
                    password: None,
                },
            );

            match res {
                Ok((rx, tx)) => {
                    commands.spawn(LoginResultComp(Some(rx), Some(tx), Some(inner)));
                }
                Err(err) => {
                    commands.spawn(LoginErrorComp(err, inner));
                }
            }
            commands.entity(e).despawn();
        }
    }
}

pub(crate) fn handle_character_creation_tasks(
    mut commands: Commands,
    mut q: Query<(Entity, &mut CharacterCreationTaskEntity)>,
) {
    for (e, mut task_wrap) in &mut q {
        if let Some(res) = future::block_on(future::poll_once(&mut task_wrap.0.task)) {
            println!(
                "[webui] CharacterCreationTask completed: success={}.",
                res.is_ok()
            );
            let inner = std::mem::replace(
                &mut task_wrap.0,
                CharacterCreationTaskInner {
                    task: IoTaskPool::get().spawn(async { Err(LoginError::Unknown) }),
                    save_login: false,
                    server_id: 0,
                    username: String::new(),
                    password: None,
                },
            );

            match res {
                Ok(_) => {
                    commands.spawn(CharacterCreationResultComp(Some(inner)));
                }
                Err(err) => {
                    commands.spawn(CharacterCreationErrorComp(err));
                }
            }
            commands.entity(e).despawn();
        }
    }
}

pub(crate) fn handle_character_creation_results(
    mut commands: Commands,
    mut success_q: Query<(Entity, &mut CharacterCreationResultComp)>,
    mut error_q: Query<(Entity, &mut CharacterCreationErrorComp)>,
    mut outbound: MessageWriter<UiOutbound>,
    mut settings: ResMut<SettingsFile>,
    mut prelogin_state: ResMut<PreLoginConnectionState>,
    storage_config: Res<crate::resources::StorageConfig>,
) {
    for (e, mut res) in &mut success_q {
        let inner = res.0.take();
        if inner.is_none() {
            continue;
        }
        let inner = inner.unwrap();

        commands.entity(e).despawn();

        println!(
            "[webui] CharacterCreationResult: success for user {}",
            inner.username
        );

        let cred_id = format!("{}:{}", inner.server_id, inner.username);

        if inner.save_login {
            if let Some(pw) = inner.password {
                let _ = keyring::set_password(&cred_id, &pw);
            }
            settings.add_credential(
                inner.server_id,
                &inner.username,
                &storage_config,
                None, // We can't save preview here, it will be fetched when logging in.
            );
        }

        // Return to login screen
        if prelogin_state.server_id == Some(inner.server_id) && prelogin_state.has_session() {
            prelogin_state.mark_ready(inner.server_id);
        }
        outbound.write(UiOutbound(settings.to_snapshot_message(None)));
    }

    for (e, err) in &mut error_q {
        println!("[webui] CharacterCreationResult: error {:?}", err.0);
        commands.entity(e).despawn();
        if let Some(server) = selected_server(&settings) {
            if prelogin_state.server_id == Some(server.id) && prelogin_state.has_session() {
                prelogin_state.mark_ready(server.id);
            }
        }
        outbound.write(UiOutbound(
            settings.to_snapshot_message(Some(err.0.clone())),
        ));
    }
}

fn parse_host_port(address: &str) -> Option<(String, u16)> {
    let mut parts = address.split(':');
    let host = parts.next()?.to_string();
    let port = parts
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(2610);
    Some((host, port))
}

pub(crate) fn handle_login_results(
    mut commands: Commands,
    mut success_q: Query<(Entity, &mut LoginResultComp)>,
    mut error_q: Query<(Entity, &mut LoginErrorComp)>,
    mut outbound: MessageWriter<UiOutbound>,
    mut settings: ResMut<SettingsFile>,
    mut prelogin_state: ResMut<PreLoginConnectionState>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    for (e, mut res) in &mut success_q {
        let (receiver, sender, inner) = {
            let LoginResultComp(rx, tx, inner) = &mut *res;
            (rx.take(), tx.take(), inner.take())
        };

        if receiver.is_none() || sender.is_none() || inner.is_none() {
            continue;
        }
        let receiver: network::DecryptedReceiver = receiver.unwrap();
        let sender: network::EncryptedSender = sender.unwrap();
        let inner: LoginTaskInner = inner.unwrap();

        commands.entity(e).despawn();

        println!(
            "[webui] LoginResult: success for user {} on server {}",
            inner.username, inner.server_id
        );
        // Spawn the background receiver task piping into NetEventRx
        use crate::session::runtime::{NetBgTask, NetEventRx};
        let (tx, rx) = crossbeam_channel::unbounded::<crate::events::NetworkEvent>();
        commands.insert_resource(NetEventRx(rx));

        let (outbox_tx, outbox_rx) = async_channel::unbounded::<Vec<u8>>();
        commands.insert_resource(crate::network::PacketOutbox(outbox_tx.clone()));

        let tx_for_task = tx.clone();
        let mut rx_loop = receiver;

        let reader_task = IoTaskPool::get().spawn(async move {
            loop {
                match rx_loop.receive().await {
                    Ok((packet_id, packet_data)) => {
                        use packets::server;
                        if let Ok(code) = server::Codes::try_from(packet_id) {
                            let _ = tx_for_task
                                .send(crate::events::NetworkEvent::Packet(code, packet_data));
                        }
                    }
                    Err(_) => {
                        let _ = tx_for_task.send(crate::events::NetworkEvent::Disconnected);
                        break;
                    }
                }
            }
        });

        // Spawn the background writer task on the IoTaskPool
        let mut tx_loop = sender;
        let writer_task = IoTaskPool::get().spawn(async move {
            while let Ok(packet) = outbox_rx.recv().await {
                if let Err(_) = tx_loop.send(&packet).await {
                    break;
                }
                while let Ok(extra_packet) = outbox_rx.try_recv() {
                    if let Err(_) = tx_loop.send(&extra_packet).await {
                        return;
                    }
                }
                let _ = tx_loop.flush().await;
            }
        });

        commands.spawn(NetBgTask(reader_task));
        commands.spawn(NetBgTask(writer_task));

        // Emit connected event to seed tick timers
        let _ = tx.send(crate::events::NetworkEvent::Connected);
        // On success, if remember was requested, persist cred and password
        if inner.remember {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            if let Some(pw) = &inner.password {
                let _ = keyring::set_password(&inner.cred_id, pw);
            }
            // Upsert saved credential record
            if let Some(existing) = settings
                .saved_credentials
                .iter_mut()
                .find(|c| c.id == inner.cred_id)
            {
                existing.last_used = now;
                existing.username = inner.username.clone();
                existing.server_id = inner.server_id;
            } else {
                settings.saved_credentials.push(SavedCredential {
                    id: inner.cred_id.clone(),
                    server_id: inner.server_id,
                    username: inner.username.clone(),
                    last_used: now,
                    preview: None,
                });
            }
        }
        let server_url = settings
            .servers
            .iter()
            .find(|s| s.id == inner.server_id)
            .map(|s| s.address.clone())
            .unwrap_or_default();

        commands.insert_resource(crate::CurrentSession {
            username: inner.username.clone(),
            server_id: inner.server_id,
            server_url,
        });

        let mut hotbars = settings.get_hotbars(inner.server_id, &inner.username);
        let mut macros = settings.get_macros(inner.server_id, &inner.username);

        if macros.is_empty() {
            // Populate default emote macros for ids 9-44.
            for i in 9..=44 {
                let Ok(body_anim) = packets::types::BodyAnimationKind::try_from(i) else {
                    continue;
                };
                let name = crate::ecs::macros::get_animation_name_by_id(body_anim);
                if !name.is_empty() {
                    let id = format!("MC{:04}{}", i, name);
                    macros.insert(id.clone(), format!("emote(\"{}\");", name));
                }
            }
            settings.set_macros(inner.server_id, &inner.username, macros.clone());
        }

        if hotbars.is_blank() {
            hotbars.apply_default_emote_layout();
            settings.set_hotbars(inner.server_id, &inner.username, hotbars.clone());
        }

        let mut hotbar_state = crate::ecs::hotbar::HotbarState::new();
        hotbar_state.config = hotbars;
        commands.insert_resource(hotbar_state);

        // Load the saved hotbar panel selection
        let saved_panel = settings.get_current_hotbar_panel(inner.server_id, &inner.username);
        let saved_row_count = settings.get_hotbar_row_count(inner.server_id, &inner.username);
        let mut hotbar_panel_state = crate::ecs::hotbar::HotbarPanelState::default();
        hotbar_panel_state.current_panel =
            crate::ecs::hotbar::HotbarPanel::from_u8(saved_panel as u8);
        hotbar_panel_state.rows = crate::ecs::hotbar::HotbarRows::from_i32(saved_row_count);
        commands.insert_resource(hotbar_panel_state);

        next_state.set(AppState::InGame);
        outbound.write(UiOutbound(CoreToUi::EnteredGame));
    }

    for (e, err) in &mut error_q {
        println!(
            "[webui] LoginResult: failed with code {:?} for user {} on server {}",
            err.0, err.1.username, err.1.server_id
        );
        if let Some(server) = settings
            .servers
            .iter()
            .find(|s| s.id == err.1.server_id)
            .cloned()
        {
            spawn_prelogin_connect_task(&mut commands, &mut prelogin_state, &server, true);
        }
        // Login failed: keep user on the current screen (login) and emit error
        let logins_public: Vec<SavedCredentialPublic> =
            settings.saved_credentials.iter().map(to_public).collect();
        outbound.write(UiOutbound(CoreToUi::Snapshot {
            servers: settings.servers.clone(),
            current_server_id: settings.gameplay.current_server_id,
            logins: logins_public,
            login_error: Some(err.0.clone()),
        }));
        commands.entity(e).despawn();
    }
}
