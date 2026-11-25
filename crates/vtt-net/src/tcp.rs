//! TCP-транспорт для игры в локальной сети.
//!
//! Роли:
//!
//! - [`TcpHub`] живёт на ПК ГМа: слушает порт, принимает подключения,
//!   раздаёт `ClientId` и умеет отправить сообщение одному или всем.
//! - [`TcpClient`] живёт у игрока (и у ГМа, если он смотрит на себя как игрок):
//!   подключается, шлёт и принимает сообщения.
//!
//! Каждое соединение обслуживают два потока (чтение и запись), сообщения ходят
//! через каналы `std::sync::mpsc`. Никаких `async`/Tokio: для LAN-сценария и
//! 5–8 игроков этого более чем достаточно, а зависимостей — ноль.
//!
//! Ограничение: `TcpHub::poll` нужно вызывать регулярно (он не блокируется).

use crate::codec::{decode, encode, read_frame};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use vtt_proto::{ClientId, ClientMsg, ServerMsg};

/// Серверная сторона: принимает подключения и рассылает сообщения.
pub struct TcpHub {
    local_addr: SocketAddr,
    incoming_rx: Arc<Mutex<mpsc::Receiver<(ClientId, ClientMsg)>>>,
    writers: Arc<Mutex<HashMap<ClientId, mpsc::Sender<Vec<u8>>>>>,
    shutdown: Arc<AtomicBool>,
}

impl fmt::Debug for TcpHub {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpHub")
            .field("local_addr", &self.local_addr)
            .field("clients", &self.client_count())
            .finish()
    }
}

impl TcpHub {
    /// Запустить сервер на адресе. `127.0.0.1:0` — случайный свободный порт.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let local_addr = listener.local_addr()?;
        let (incoming_tx, incoming_rx) = mpsc::channel();
        let incoming_rx = Arc::new(Mutex::new(incoming_rx));
        let writers: Arc<Mutex<HashMap<ClientId, mpsc::Sender<Vec<u8>>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let next_client = Arc::new(AtomicU64::new(1));
        let shutdown = Arc::new(AtomicBool::new(false));

        let accept_writers = Arc::clone(&writers);
        let accept_next = Arc::clone(&next_client);
        let accept_shutdown = Arc::clone(&shutdown);
        thread::Builder::new()
            .name("vtt-accept".to_string())
            .spawn(move || {
                accept_loop(
                    listener,
                    incoming_tx,
                    accept_writers,
                    accept_next,
                    accept_shutdown,
                );
            })?;

        Ok(Self {
            local_addr,
            incoming_rx,
            writers,
            shutdown,
        })
    }

    /// Адрес, на котором слушает сервер.
    #[must_use]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// Забрать все накопившиеся входящие сообщения (не блокируется).
    #[must_use]
    pub fn poll(&self) -> Vec<(ClientId, ClientMsg)> {
        let mut out = Vec::new();
        let Ok(receiver) = self.incoming_rx.lock() else {
            return out;
        };
        while let Ok(item) = receiver.try_recv() {
            out.push(item);
        }
        out
    }

    /// Отправить сообщение одному клиенту.
    pub fn send_to(&self, client: ClientId, msg: &ServerMsg) -> io::Result<()> {
        let payload = encode(msg)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let writers = self
            .writers
            .lock()
            .map_err(|_| io::Error::other("список клиентов повреждён"))?;
        match writers.get(&client) {
            Some(tx) => tx
                .send(payload)
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "клиент уже отключился")),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                "неизвестный клиент",
            )),
        }
    }

    /// Отправить сообщение всем. Возвращает, скольким доставлено.
    pub fn broadcast(&self, msg: &ServerMsg) -> io::Result<usize> {
        let payload = encode(msg)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        let writers = self
            .writers
            .lock()
            .map_err(|_| io::Error::other("список клиентов повреждён"))?;
        let mut sent = 0;
        for tx in writers.values() {
            if tx.send(payload.clone()).is_ok() {
                sent += 1;
            }
        }
        Ok(sent)
    }

    /// Сколько клиентов подключено.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.writers.lock().map(|w| w.len()).unwrap_or(0)
    }

    /// Остановить приём новых подключений.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        // «Будильник» для accept(): пустое подключение заставляет цикл проверить флаг.
        let _ = TcpStream::connect(self.local_addr);
    }

    /// Отключить клиента (после уведомления): поток записи завершится и закроет сокет.
    pub fn drop_client(&self, client: ClientId) {
        if let Ok(mut writers) = self.writers.lock() {
            writers.remove(&client);
        }
    }
}

impl Drop for TcpHub {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Клиентская сторона.
pub struct TcpClient {
    peer: SocketAddr,
    writer_tx: mpsc::Sender<Vec<u8>>,
    incoming_rx: mpsc::Receiver<ServerMsg>,
    alive: Arc<AtomicBool>,
}

impl fmt::Debug for TcpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TcpClient")
            .field("peer", &self.peer)
            .field("alive", &self.alive.load(Ordering::Relaxed))
            .finish()
    }
}

impl TcpClient {
    /// Подключиться к серверу.
    pub fn connect(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        let peer = stream.peer_addr()?;
        let reader_stream = stream.try_clone()?;

        let (writer_tx, writer_rx) = mpsc::channel::<Vec<u8>>();
        let (incoming_tx, incoming_rx) = mpsc::channel::<ServerMsg>();
        let alive = Arc::new(AtomicBool::new(true));

        let mut writer = stream;
        thread::Builder::new()
            .name("vtt-net-writer".to_string())
            .spawn(move || {
                for payload in writer_rx {
                    if write_raw(&mut writer, &payload).is_err() {
                        break;
                    }
                }
                let _ = writer.shutdown(Shutdown::Both);
            })?;

        let mut reader = reader_stream;
        let reader_alive = Arc::clone(&alive);
        thread::Builder::new()
            .name("vtt-net-reader".to_string())
            .spawn(move || {
                while let Ok(Some(payload)) = read_frame(&mut reader) {
                    if let Ok(msg) = decode::<ServerMsg>(&payload) {
                        if incoming_tx.send(msg).is_err() {
                            break;
                        }
                    }
                }
                reader_alive.store(false, Ordering::Relaxed);
            })?;

        Ok(Self {
            peer,
            writer_tx,
            incoming_rx,
            alive,
        })
    }

    /// Отправить сообщение серверу.
    pub fn send(&self, msg: &ClientMsg) -> io::Result<()> {
        let payload = encode(msg)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        self.writer_tx
            .send(payload)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "соединение закрыто"))
    }

    /// Забрать все накопившиеся сообщения (не блокируется).
    #[must_use]
    pub fn poll(&self) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        while let Ok(msg) = self.incoming_rx.try_recv() {
            out.push(msg);
        }
        out
    }

    /// Адрес сервера.
    #[must_use]
    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    /// Живо ли соединение с точки зрения читающего потока.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

// ---------------------------------------------------------------------------
// Внутренние функции
// ---------------------------------------------------------------------------

fn accept_loop(
    listener: TcpListener,
    incoming_tx: mpsc::Sender<(ClientId, ClientMsg)>,
    writers: Arc<Mutex<HashMap<ClientId, mpsc::Sender<Vec<u8>>>>>,
    next_client: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
) {
    for stream in listener.incoming() {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
        let Ok(stream) = stream else {
            continue;
        };
        let client_id = next_client.fetch_add(1, Ordering::Relaxed);
        let (out_tx, out_rx) = mpsc::channel::<Vec<u8>>();
        if let Ok(mut map) = writers.lock() {
            map.insert(client_id, out_tx);
        }

        let conn_incoming = incoming_tx.clone();
        let conn_writers = Arc::clone(&writers);
        let _ = thread::Builder::new()
            .name(format!("vtt-conn-{client_id}"))
            .spawn(move || {
                connection_loop(stream, client_id, out_rx, conn_incoming, conn_writers);
            });
    }
}

fn connection_loop(
    stream: TcpStream,
    client_id: ClientId,
    out_rx: mpsc::Receiver<Vec<u8>>,
    incoming_tx: mpsc::Sender<(ClientId, ClientMsg)>,
    writers: Arc<Mutex<HashMap<ClientId, mpsc::Sender<Vec<u8>>>>>,
) {
    let Ok(reader_stream) = stream.try_clone() else {
        if let Ok(mut map) = writers.lock() {
            map.remove(&client_id);
        }
        return;
    };

    let mut writer = stream;
    let _writer_thread = thread::Builder::new()
        .name(format!("vtt-w-{client_id}"))
        .spawn(move || {
            for payload in out_rx {
                if write_raw(&mut writer, &payload).is_err() {
                    break;
                }
            }
        });

    let mut reader = reader_stream;
    loop {
        match read_frame(&mut reader) {
            Ok(Some(payload)) => match decode::<ClientMsg>(&payload) {
                Ok(msg) => {
                    if incoming_tx.send((client_id, msg)).is_err() {
                        break;
                    }
                }
                // Мусор в одном сообщении не должен рвать соединение.
                Err(_) => continue,
            },
            Ok(None) => break,
            Err(_) => break,
        }
    }

    if let Ok(mut map) = writers.lock() {
        map.remove(&client_id);
    }
}

fn write_raw(stream: &mut TcpStream, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "сообщение слишком большое"))?;
    stream.write_all(&len.to_le_bytes())?;
    stream.write_all(payload)?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    use vtt_proto::{PlayerId, Role, Welcome, PROTOCOL_VERSION};

    fn wait_for<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(value) = f() {
                return value;
            }
            if Instant::now() > deadline {
                panic!("не дождались: {what}");
            }
            thread::sleep(Duration::from_millis(5));
        }
    }

    fn hello(name: &str) -> ClientMsg {
        ClientMsg::Hello {
            name: name.to_string(),
            code: "TEST42".to_string(),
            protocol: PROTOCOL_VERSION,
            resume: None,
        }
    }

    #[test]
    fn hub_reports_address_and_zero_clients() {
        let hub = TcpHub::bind("127.0.0.1:0").unwrap();
        assert_eq!(hub.client_count(), 0);
        assert!(hub.local_addr().port() > 0);
    }

    #[test]
    fn client_connects_and_server_sees_hello() {
        let hub = TcpHub::bind("127.0.0.1:0").unwrap();
        let client = TcpClient::connect(hub.local_addr()).unwrap();
        client.send(&hello("Алиса")).unwrap();

        let (id, msg) = wait_for("Hello на сервере", || {
            hub.poll().into_iter().next()
        });
        assert!(id > 0);
        assert_eq!(msg, hello("Алиса"));
        assert_eq!(hub.client_count(), 1);
    }

    #[test]
    fn server_can_answer_one_client_and_broadcast() {
        let hub = TcpHub::bind("127.0.0.1:0").unwrap();
        let a = TcpClient::connect(hub.local_addr()).unwrap();
        let b = TcpClient::connect(hub.local_addr()).unwrap();
        a.send(&hello("A")).unwrap();
        b.send(&hello("B")).unwrap();

        let mut ids = Vec::new();
        wait_for("два подключения", || {
            for (id, _) in hub.poll() {
                ids.push(id);
            }
            if ids.len() == 2 {
                Some(())
            } else {
                None
            }
        });

        let welcome = ServerMsg::Welcome(Welcome {
            client: ids[0],
            player: PlayerId(1),
            role: Role::Player,
            campaign: "Тест".into(),
            protocol: PROTOCOL_VERSION,
        });
        hub.send_to(ids[0], &welcome).unwrap();
        let got = wait_for("Welcome у A", || a.poll().into_iter().next());
        assert_eq!(got, welcome);

        hub.broadcast(&ServerMsg::Notice {
            text: "всем привет".into(),
        })
        .unwrap();
        let got_a = wait_for("broadcast у A", || a.poll().into_iter().next());
        let got_b = wait_for("broadcast у B", || b.poll().into_iter().next());
        assert!(matches!(got_a, ServerMsg::Notice { .. }));
        assert!(matches!(got_b, ServerMsg::Notice { .. }));
    }

    #[test]
    fn disconnected_client_is_removed() {
        let hub = TcpHub::bind("127.0.0.1:0").unwrap();
        {
            let client = TcpClient::connect(hub.local_addr()).unwrap();
            client.send(&hello("Уходит")).unwrap();
            wait_for("подключение", || {
                hub.poll().into_iter().next().map(|_| ())
            });
            assert_eq!(hub.client_count(), 1);
        } // клиент закрыт здесь
        wait_for("отключение", || {
            if hub.client_count() == 0 {
                Some(())
            } else {
                None
            }
        });
    }

    #[test]
    fn sending_to_unknown_client_errors() {
        let hub = TcpHub::bind("127.0.0.1:0").unwrap();
        let err = hub
            .send_to(999, &ServerMsg::Notice { text: "x".into() })
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn shutdown_works_and_is_idempotent() {
        let hub = TcpHub::bind("127.0.0.1:0").unwrap();
        hub.shutdown();
        hub.shutdown();
    }
}
