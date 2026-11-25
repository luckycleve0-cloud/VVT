//! # vtt-server — авторитарное состояние игры
//!
//! Комната = один [`GameServer`]. Он хранит [`Campaign`], знает подключённых
//! участников, проверяет права и решает, что и кому разослать.
//!
//! Транспорт не входит в этот крейт: сервер возвращает [`Outgoing`], а
//! `vtt-net` доставляет их по TCP (или, в будущем, по QUIC/iroh).
//!
//! ```no_run
//! use vtt_server::{Campaign, GameServer};
//! use vtt_proto::{ClientMsg, PROTOCOL_VERSION};
//!
//! let mut server = GameServer::new(Campaign::new("Моя кампания"));
//! let host = server.add_host("ГМ");
//! let reply = server.handle(0, ClientMsg::Ping { nonce: 1 });
//! assert_eq!(reply.len(), 1);
//! # let _ = host;
//! ```

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod campaign;
pub mod persist;
pub mod server;

pub use campaign::Campaign;
pub use persist::{campaign_exists, load_campaign, load_pack_file, save_campaign, CAMPAIGN_FILE};
pub use server::{room_code, ClientSession, GameServer, Outgoing, HOST_CLIENT};
