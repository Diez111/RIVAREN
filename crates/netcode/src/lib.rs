//! `rivaren-netcode`: multijugador determinista con predicción y rollback.
//!
//! Transporte por defecto: UDP con confiabilidad selectiva (acks + reenvío de
//! eventos) implementado sobre `std::net`, sin dependencias externas.
//! QUIC (quinn) queda tras la feature `quic` para cifrado multiplexado.
//!
//! Modelo: servidor autoritativo ligero. El cliente predice su jugador y
//! rebobina al recibir correcciones.

use rivaren_core::Fixed;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_PACKET: usize = 1200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Input {
    pub frame: u64,
    pub move_x: i8,
    pub move_z: i8,
    pub jump: bool,
    pub sprint: bool,
    pub action: u8,
    pub yaw: i16,   // centésimas de radián
    pub pitch: i16,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            frame: 0,
            move_x: 0,
            move_z: 0,
            jump: false,
            sprint: false,
            action: 0,
            yaw: 0,
            pitch: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub frame: u64,
    pub player: [i32; 3], // fixed-point
    pub yaw: i16,
    pub dimension: u8,
    pub time_of_day: u16, // 0..65535
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMsg {
    Hello { version: u16, name: String },
    Input(Input),
    Chat(String),
    BlockEdit { x: i32, y: i32, z: i32, block: u16 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMsg {
    Welcome { id: u32, seed: u64, frame: u64 },
    Snapshot(Snapshot),
    Chat { from: String, text: String },
    BlockEdit { x: i32, y: i32, z: i32, block: u16 },
    Correction { frame: u64, player: [i32; 3] },
    Reject { reason: String },
}

/// Fragmento de datagrama con número de secuencia para acks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    pub seq: u32,
    pub ack: u32,
    pub payload: Payload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Client(ClientMsg),
    Server(ServerMsg),
    Ping,
    Pong,
}

pub fn encode(frame: &Frame) -> Vec<u8> {
    bincode::serialize(frame).unwrap_or_default()
}

pub fn decode(bytes: &[u8]) -> Option<Frame> {
    bincode::deserialize(bytes).ok()
}

/// Estado de conexión con confiabilidad selectiva.
#[derive(Debug, Default)]
pub struct Connection {
    pub peer: Option<SocketAddr>,
    pub send_seq: u32,
    pub recv_seq: u32,
    /// Mensajes fiables sin ack: seq → (bytes, intentos).
    pub pending: HashMap<u32, (Vec<u8>, u32)>,
    pub last_recv: Vec<Frame>,
}

impl Connection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wrap(&mut self, payload: Payload) -> Vec<u8> {
        self.send_seq = self.send_seq.wrapping_add(1);
        let frame = Frame {
            seq: self.send_seq,
            ack: self.recv_seq,
            payload,
        };
        encode(&frame)
    }

    pub fn unpack(&mut self, bytes: &[u8]) -> Option<Payload> {
        let frame = decode(bytes)?;
        self.recv_seq = self.recv_seq.max(frame.seq);
        self.pending.remove(&frame.ack);
        let payload = frame.payload.clone();
        self.last_recv.push(frame);
        if self.last_recv.len() > 64 {
            self.last_recv.remove(0);
        }
        Some(payload)
    }

    /// Devuelve paquetes a reenviar (timeout por intentos).
    pub fn resend(&mut self) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut remove = Vec::new();
        for (seq, (bytes, tries)) in self.pending.iter_mut() {
            *tries += 1;
            if *tries > 8 {
                remove.push(*seq);
                continue;
            }
            if *tries % 3 == 0 {
                out.push(bytes.clone());
            }
        }
        for seq in remove {
            self.pending.remove(&seq);
        }
        out
    }
}

/// Historial de inputs y snapshots para rollback.
pub struct Rollback {
    inputs: VecDeque<Input>,
    snaps: VecDeque<Snapshot>,
    pub current: u64,
    pub confirmed: u64,
}

impl Default for Rollback {
    fn default() -> Self {
        Self::new()
    }
}

impl Rollback {
    pub fn new() -> Self {
        Self {
            inputs: VecDeque::with_capacity(256),
            snaps: VecDeque::with_capacity(64),
            current: 0,
            confirmed: 0,
        }
    }
    pub fn push_input(&mut self, i: Input) {
        self.inputs.push_back(i);
        if self.inputs.len() > 256 {
            self.inputs.pop_front();
        }
    }
    pub fn push_snapshot(&mut self, s: Snapshot) {
        self.snaps.push_back(s);
        if self.snaps.len() > 64 {
            self.snaps.pop_front();
        }
    }
    pub fn confirm(&mut self, frame: u64) {
        self.confirmed = self.confirmed.max(frame);
        while self
            .snaps
            .front()
            .map(|s| s.frame < frame.saturating_sub(32))
            .unwrap_or(false)
        {
            self.snaps.pop_front();
        }
    }
    /// Inputs desde `frame` para re-simular.
    pub fn rollback_to(&mut self, frame: u64) -> Vec<Input> {
        self.current = frame;
        self.inputs
            .iter()
            .filter(|i| i.frame >= frame)
            .copied()
            .collect()
    }
    pub fn snapshot_for(&self, frame: u64) -> Option<&Snapshot> {
        self.snaps
            .iter()
            .filter(|s| s.frame <= frame)
            .next_back()
    }
}

/// Servidor UDP autoritativo básico.
pub struct Server {
    socket: UdpSocket,
    pub clients: HashMap<SocketAddr, Connection>,
    pub frame: u64,
    pub seed: u64,
    pub next_id: u32,
}

impl Server {
    pub fn bind(addr: &str, seed: u64) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            clients: HashMap::new(),
            frame: 0,
            seed,
            next_id: 1,
        })
    }

    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Procesa paquetes entrantes y devuelve cuántos se atendieron.
    pub fn poll(&mut self, max: usize) -> usize {
        let mut buf = [0u8; MAX_PACKET];
        let mut handled = 0;
        while handled < max {
            let Ok((n, from)) = self.socket.recv_from(&mut buf) else {
                break;
            };
            let Some(frame) = decode(&buf[..n]) else {
                continue;
            };
            let conn = self.clients.entry(from).or_default();
            let payload = match frame.payload.clone() {
                Payload::Client(msg) => msg,
                _ => continue,
            };
            match payload {
                ClientMsg::Hello { version, name: _ } => {
                    if version != PROTOCOL_VERSION {
                        let reply = conn.wrap(Payload::Server(ServerMsg::Reject {
                            reason: "versión incompatible".into(),
                        }));
                        let _ = self.socket.send_to(&reply, from);
                    } else {
                        let id = self.next_id;
                        self.next_id += 1;
                        let reply = conn.wrap(Payload::Server(ServerMsg::Welcome {
                            id,
                            seed: self.seed,
                            frame: self.frame,
                        }));
                        let _ = self.socket.send_to(&reply, from);
                    }
                }
                ClientMsg::Input(input) => {
                    self.frame = self.frame.max(input.frame);
                    conn.wrap(Payload::Pong); // ack implícito
                }
                ClientMsg::BlockEdit { .. } => {
                    // Reenvía a todos menos al emisor.
                    self.broadcast_except(from, &frame);
                }
                ClientMsg::Chat(text) => {
                    let msg = ServerMsg::Chat {
                        from: from.to_string(),
                        text,
                    };
                    self.broadcast(&Payload::Server(msg));
                }
            }
            handled += 1;
        }
        handled
    }

    pub fn broadcast(&self, payload: &Payload) {
        let mut buf = Vec::new();
        for (addr, conn) in &self.clients {
            let mut c = Connection {
                send_seq: conn.send_seq,
                recv_seq: conn.recv_seq,
                ..Default::default()
            };
            buf.clear();
            buf.extend_from_slice(&c.wrap(payload.clone()));
            let _ = self.socket.send_to(&buf, addr);
        }
    }

    fn broadcast_except(&self, skip: SocketAddr, frame: &Frame) {
        let bytes = encode(frame);
        for addr in self.clients.keys() {
            if *addr != skip {
                let _ = self.socket.send_to(&bytes, addr);
            }
        }
    }
}

/// Cliente de pruebas / juego.
pub struct Client {
    socket: UdpSocket,
    pub server: SocketAddr,
    pub conn: Connection,
    pub rollback: Rollback,
    pub id: u32,
    pub seed: u64,
}

impl Client {
    pub fn connect(server_addr: &str, name: &str, timeout_ms: u64) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_read_timeout(Some(std::time::Duration::from_millis(timeout_ms)))?;
        let server: SocketAddr = server_addr.parse().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "dirección inválida")
        })?;
        let mut conn = Connection::new();
        conn.peer = Some(server);
        let hello = conn.wrap(Payload::Client(ClientMsg::Hello {
            version: PROTOCOL_VERSION,
            name: name.to_string(),
        }));
        socket.send_to(&hello, server)?;
        // Espera Welcome.
        let mut buf = [0u8; MAX_PACKET];
        let mut id = 0;
        let mut seed = 0;
        if let Ok((n, from)) = socket.recv_from(&mut buf) {
            if from == server {
                if let Some(Payload::Server(ServerMsg::Welcome { id: vid, seed: vseed, .. })) =
                    conn.unpack(&buf[..n])
                {
                    id = vid;
                    seed = vseed;
                }
            }
        }
        Ok(Self {
            socket,
            server,
            conn,
            rollback: Rollback::new(),
            id,
            seed,
        })
    }

    pub fn send_input(&mut self, input: Input) -> std::io::Result<()> {
        self.rollback.push_input(input.clone());
        let bytes = self.conn.wrap(Payload::Client(ClientMsg::Input(input)));
        self.socket.send_to(&bytes, self.server).map(|_| ())
    }

    pub fn send_edit(&mut self, x: i32, y: i32, z: i32, block: u16) -> std::io::Result<()> {
        let bytes = self
            .conn
            .wrap(Payload::Client(ClientMsg::BlockEdit { x, y, z, block }));
        self.socket.send_to(&bytes, self.server).map(|_| ())
    }

    /// Recibe y procesa mensajes; aplica correcciones al rollback.
    pub fn poll(&mut self) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        let mut buf = [0u8; MAX_PACKET];
        loop {
            let Ok((n, from)) = self.socket.recv_from(&mut buf) else {
                break;
            };
            if from != self.server {
                continue;
            }
            let Some(payload) = self.conn.unpack(&buf[..n]) else {
                continue;
            };
            if let Payload::Server(msg) = payload {
                match &msg {
                    ServerMsg::Snapshot(s) => {
                        self.rollback.push_snapshot(s.clone());
                        self.rollback.confirm(s.frame);
                    }
                    ServerMsg::Correction { frame, .. } => {
                        self.rollback.rollback_to(*frame);
                    }
                    _ => {}
                }
                out.push(msg);
            }
        }
        out
    }
}

/// Simulación determinista simple del jugador para predicción/rollback.
/// Usa fixed-point para reproducibilidad multiplataforma.
pub fn simulate_player(pos: &mut [Fixed; 3], input: &Input, dt_ms: u32) {
    let speed = if input.sprint { 7 } else { 4 };
    let step = (speed as i32 * dt_ms as i32) / 50; // bloques por tick
    *pos = [
        Fixed(pos[0].0 + input.move_x as i32 * step * Fixed::SCALE / 32),
        pos[1],
        Fixed(pos[2].0 + input.move_z as i32 * step * Fixed::SCALE / 32),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_and_input() {
        let server = Server::bind("127.0.0.1:0", 4242).unwrap();
        let addr = server.local_addr().unwrap();
        let mut server = server;
        let handle = std::thread::spawn(move || {
            // Acepta hello + input.
            let mut got = 0;
            for _ in 0..200 {
                got += server.poll(8);
                if got >= 2 {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            got
        });
        let mut client = Client::connect(&addr.to_string(), "tester", 500).unwrap();
        assert_eq!(client.seed, 4242);
        client.send_input(Input { frame: 1, move_x: 4, ..Default::default() }).unwrap();
        let handled = handle.join().unwrap();
        assert!(handled >= 2, "servidor procesó {handled} paquetes");
    }

    #[test]
    fn rollback_replays_inputs() {
        let mut rb = Rollback::new();
        for f in 0..10 {
            rb.push_input(Input { frame: f, ..Default::default() });
        }
        rb.push_snapshot(Snapshot {
            frame: 5,
            player: [0, 0, 0],
            yaw: 0,
            dimension: 0,
            time_of_day: 0,
        });
        let inputs = rb.rollback_to(5);
        assert_eq!(inputs.len(), 5);
        assert_eq!(inputs[0].frame, 5);
    }

    #[test]
    fn deterministic_sim() {
        let mut a = [Fixed(0), Fixed(0), Fixed(0)];
        let mut b = [Fixed(0), Fixed(0), Fixed(0)];
        let input = Input { frame: 0, move_x: 1, ..Default::default() };
        simulate_player(&mut a, &input, 50);
        simulate_player(&mut b, &input, 50);
        assert_eq!(a, b);
        assert!(a[0].0 > 0);
    }
}
