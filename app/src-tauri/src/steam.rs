//! Steam lobbies, invites and messages for online games.
//!
//! Steam runs on its own thread. The window sends it commands; it reports the lobby and
//! incoming messages back as events:
//!
//! - `steam-lobby`: the current [`LobbyState`] (or `null` after leaving).
//! - `steam-message`: `{ from, data }`, a message another player sent.
//!
//! The game itself is host-authoritative: the lobby owner runs the rules engine, the others
//! send their actions to the host and get their own view of the table back. That protocol lives
//! in the UI; this module only moves strings between players.
//!
//! When Steam isn't running (or the game wasn't started with Steam available), everything else
//! still works and `steam_status` says why online play is off.

use serde::Serialize;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use steamworks::{
    networking_types::{NetworkingIdentity, SendFlags},
    Client, GameLobbyJoinRequested, LobbyChatUpdate, LobbyDataUpdate, LobbyId, LobbyType, SteamId,
};
use tauri::{AppHandle, Emitter};

/// Steam's shared test app ("Spacewar"). Replace with the game's own app ID once it has one.
pub const APP_ID: u32 = 480;
const CHANNEL: u32 = 0;
const MAX_PLAYERS: u32 = 4;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Member {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct LobbyState {
    pub lobby: String,
    pub me: String,
    pub host: String,
    pub is_host: bool,
    pub members: Vec<Member>,
}

#[derive(Clone, Debug, Serialize)]
pub struct SteamStatus {
    pub available: bool,
    pub name: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct Incoming {
    from: String,
    data: String,
}

enum Cmd {
    Create,
    Invite,
    Join(u64),
    Leave,
    /// `None` sends to everyone else in the lobby.
    Send(Option<u64>, String),
}

enum Internal {
    Entered(LobbyId),
    Failed(String),
    Refresh,
    JoinRequested(LobbyId),
}

pub struct Steam {
    tx: Mutex<Option<Sender<Cmd>>>,
    status: Arc<Mutex<SteamStatus>>,
}

impl Steam {
    /// Start Steam on its own thread. Never fails: without Steam, online play is just off.
    pub fn start(app: AppHandle) -> Steam {
        let status = Arc::new(Mutex::new(SteamStatus {
            available: false,
            name: None,
            error: Some("Starting Steam…".into()),
        }));
        let (tx, rx) = channel::<Cmd>();
        let st = status.clone();
        let launch_lobby = launch_lobby_arg();
        std::thread::Builder::new()
            .name("steam".into())
            .spawn(move || match Client::init_app(APP_ID) {
                Ok(client) => {
                    *st.lock().unwrap() = SteamStatus {
                        available: true,
                        name: Some(client.friends().name()),
                        error: None,
                    };
                    run(client, rx, app, launch_lobby);
                }
                Err(e) => {
                    *st.lock().unwrap() = SteamStatus {
                        available: false,
                        name: None,
                        error: Some(format!(
                            "Steam isn't running, so online play is off ({e}). Start Steam and reopen the game."
                        )),
                    };
                }
            })
            .expect("spawn steam thread");
        Steam {
            tx: Mutex::new(Some(tx)),
            status,
        }
    }

    pub fn status(&self) -> SteamStatus {
        self.status.lock().unwrap().clone()
    }

    fn send(&self, cmd: Cmd) -> Result<(), String> {
        if !self.status().available {
            return Err(self
                .status()
                .error
                .unwrap_or_else(|| "Steam isn't available".into()));
        }
        self.tx
            .lock()
            .unwrap()
            .as_ref()
            .ok_or("Steam has stopped")?
            .send(cmd)
            .map_err(|_| "Steam has stopped".to_string())
    }

    pub fn create(&self) -> Result<(), String> {
        self.send(Cmd::Create)
    }
    pub fn invite(&self) -> Result<(), String> {
        self.send(Cmd::Invite)
    }
    pub fn join(&self, lobby: &str) -> Result<(), String> {
        let id = lobby.parse().map_err(|_| "not a lobby id".to_string())?;
        self.send(Cmd::Join(id))
    }
    pub fn leave(&self) -> Result<(), String> {
        self.send(Cmd::Leave)
    }
    pub fn message(&self, to: Option<&str>, data: String) -> Result<(), String> {
        let to = match to {
            Some(s) => Some(s.parse().map_err(|_| "not a player id".to_string())?),
            None => None,
        };
        self.send(Cmd::Send(to, data))
    }
}

/// Steam starts the game with `+connect_lobby <id>` when you accept an invite while it's closed.
fn launch_lobby_arg() -> Option<u64> {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == "+connect_lobby")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
}

fn run(client: Client, rx: Receiver<Cmd>, app: AppHandle, launch_lobby: Option<u64>) {
    let (itx, irx) = channel::<Internal>();
    let mm = client.matchmaking();
    let net = client.networking_messages();
    let me = client.user().steam_id();
    let mut lobby: Option<LobbyId> = None;
    let mut last_sent: Option<LobbyState> = None;

    // Accept message sessions only from people in our lobby.
    let session_lobby = Arc::new(Mutex::new(None::<LobbyId>));
    {
        let client = client.clone();
        let cur = session_lobby.clone();
        net.session_request_callback(move |req| {
            let ok = req
                .remote()
                .steam_id()
                .zip(*cur.lock().unwrap())
                .is_some_and(|(who, l)| client.matchmaking().lobby_members(l).contains(&who));
            if ok {
                req.accept();
            } else {
                req.reject();
            }
        });
    }
    let set_session_lobby = |l: Option<LobbyId>| *session_lobby.lock().unwrap() = l;

    let t = itx.clone();
    let _join_cb = client.register_callback(move |e: GameLobbyJoinRequested| {
        let _ = t.send(Internal::JoinRequested(e.lobby_steam_id));
    });
    let t = itx.clone();
    let _chat_cb = client.register_callback(move |_: LobbyChatUpdate| {
        let _ = t.send(Internal::Refresh);
    });
    let t = itx.clone();
    let _data_cb = client.register_callback(move |_: LobbyDataUpdate| {
        let _ = t.send(Internal::Refresh);
    });

    if let Some(id) = launch_lobby {
        let _ = itx.send(Internal::JoinRequested(LobbyId::from_raw(id)));
    }

    let join = |id: LobbyId, itx: &Sender<Internal>| {
        let t = itx.clone();
        client.matchmaking().join_lobby(id, move |r| {
            let _ = t.send(match r {
                Ok(l) => Internal::Entered(l),
                Err(()) => {
                    Internal::Failed("Couldn't join that game. It may be full or closed.".into())
                }
            });
        });
    };

    loop {
        client.run_callbacks();

        // Commands from the window.
        loop {
            match rx.try_recv() {
                Ok(Cmd::Create) => {
                    if let Some(l) = lobby.take() {
                        mm.leave_lobby(l);
                    }
                    let t = itx.clone();
                    mm.create_lobby(LobbyType::FriendsOnly, MAX_PLAYERS, move |r| {
                        let _ = t.send(match r {
                            Ok(l) => Internal::Entered(l),
                            Err(e) => {
                                Internal::Failed(format!("Couldn't create a game on Steam: {e}"))
                            }
                        });
                    });
                }
                Ok(Cmd::Invite) => {
                    if let Some(l) = lobby {
                        client.friends().activate_invite_dialog(l);
                    }
                }
                Ok(Cmd::Join(id)) => join(LobbyId::from_raw(id), &itx),
                Ok(Cmd::Leave) => {
                    if let Some(l) = lobby.take() {
                        mm.leave_lobby(l);
                    }
                    set_session_lobby(None);
                    client.friends().clear_rich_presence();
                    last_sent = None;
                    let _ = app.emit("steam-lobby", Option::<LobbyState>::None);
                }
                Ok(Cmd::Send(to, data)) => {
                    if let Some(l) = lobby {
                        let targets: Vec<SteamId> = match to {
                            Some(id) => vec![SteamId::from_raw(id)],
                            None => mm
                                .lobby_members(l)
                                .into_iter()
                                .filter(|m| *m != me)
                                .collect(),
                        };
                        for who in targets {
                            let _ = net.send_message_to_user(
                                NetworkingIdentity::new_steam_id(who),
                                SendFlags::RELIABLE | SendFlags::AUTO_RESTART_BROKEN_SESSION,
                                data.as_bytes(),
                                CHANNEL,
                            );
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return,
            }
        }

        // Results of Steam calls.
        let mut refresh = false;
        while let Ok(ev) = irx.try_recv() {
            match ev {
                Internal::Entered(l) => {
                    if let Some(old) = lobby.replace(l) {
                        if old != l {
                            mm.leave_lobby(old);
                        }
                    }
                    set_session_lobby(Some(l));
                    if mm.lobby_owner(l) == me {
                        mm.set_lobby_data(l, "game", "commander-table");
                    }
                    client.friends().set_rich_presence("steam_display", None);
                    client
                        .friends()
                        .set_rich_presence("connect", Some(&format!("+connect_lobby {}", l.raw())));
                    refresh = true;
                }
                Internal::Failed(msg) => {
                    let _ = app.emit("steam-error", msg);
                }
                Internal::Refresh => refresh = true,
                Internal::JoinRequested(l) => join(l, &itx),
            }
        }

        if refresh {
            if let Some(l) = lobby {
                let state = LobbyState {
                    lobby: l.raw().to_string(),
                    me: me.raw().to_string(),
                    host: mm.lobby_owner(l).raw().to_string(),
                    is_host: mm.lobby_owner(l) == me,
                    members: mm
                        .lobby_members(l)
                        .into_iter()
                        .map(|m| Member {
                            id: m.raw().to_string(),
                            name: client.friends().get_friend(m).name(),
                        })
                        .collect(),
                };
                if last_sent.as_ref() != Some(&state) {
                    let _ = app.emit("steam-lobby", Some(&state));
                    last_sent = Some(state);
                }
            }
        }

        // Messages from other players.
        for m in net.receive_messages_on_channel(CHANNEL, 64) {
            let from = m
                .identity_peer()
                .steam_id()
                .map(|s| s.raw().to_string())
                .unwrap_or_default();
            let data = String::from_utf8_lossy(m.data()).into_owned();
            let _ = app.emit("steam-message", Incoming { from, data });
        }

        std::thread::sleep(Duration::from_millis(15));
    }
}
