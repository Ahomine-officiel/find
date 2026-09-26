// LAN co-op over UDP - "FIND IT WITH FRIENDS" (the original game's headline feature).
// Design: the world is deterministic from (seed, straw_count), so we only sync:
//   - straw removal indices (idempotent u32 list)
//   - valuable pickups (index)
//   - player transforms (stateless 10 Hz snapshots)
//   - win event
// No reliability layer needed: digs are idempotent, positions are stateless.
use std::collections::HashMap;
use std::net::UdpSocket;

pub const PORT: u16 = 7777;
pub const MAX_PLAYERS: usize = 8;

pub const MSG_HELLO: u8 = 1;
pub const MSG_WELCOME: u8 = 2;
pub const MSG_POS: u8 = 3;
pub const MSG_DIGS: u8 = 4;
pub const MSG_VALUABLE: u8 = 5;
pub const MSG_FOUND: u8 = 6;
pub const MSG_BYE: u8 = 7;

#[derive(Clone, Copy, PartialEq)]
pub enum Role {
    Offline,
    Host,
    Client,
}

#[derive(Clone, Copy)]
pub struct RemotePlayer {
    pub active: bool,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub name: [u8; 12],
    pub last_seen: f32,
}

pub struct Remote {
    pub id: u8,
    pub players: [RemotePlayer; MAX_PLAYERS],
    pub pending_digs: Vec<u32>,
    pub pending_valuables: Vec<u32>,
    pub found_by: Option<u8>,
    pub my_name: [u8; 12],
}

impl Remote {
    fn new() -> Self {
        Remote {
            id: 0,
            players: core::array::from_fn(|_| RemotePlayer {
                active: false,
                pos: [0.0; 3],
                yaw: 0.0,
                pitch: 0.0,
                name: [0; 12],
                last_seen: 0.0,
            }),
            pending_digs: Vec::new(),
            pending_valuables: Vec::new(),
            found_by: None,
            my_name: *b"PLAYER      ",
        }
    }
}

pub struct Net {
    pub role: Role,
    pub remote: Remote,
    socket: Option<UdpSocket>,
    host_addr: Option<std::net::SocketAddr>,
    clients: HashMap<u8, (std::net::SocketAddr, f32)>, // host only: id -> (addr, last_seen)
    next_id: u8,
    pos_timer: f32,
    pub welcome_seed: Option<u64>,
    pub welcome_count: Option<u32>,
}

const DIGS_CHUNK: usize = 240; // stays under MTU: 240*4 + header

impl Net {
    pub fn offline() -> Self {
        Net {
            role: Role::Offline,
            remote: Remote::new(),
            socket: None,
            host_addr: None,
            clients: HashMap::new(),
            next_id: 1,
            pos_timer: 0.0,
            welcome_seed: None,
            welcome_count: None,
        }
    }

    pub fn host(my_name: [u8; 12]) -> Self {
        let socket = UdpSocket::bind(("0.0.0.0", PORT)).ok();
        let mut n = Net::offline();
        n.role = Role::Host;
        n.my_id(0);
        if let Some(s) = &socket {
            let _ = s.set_nonblocking(true);
        } else {
            n.role = Role::Offline;
        }
        n.socket = socket;
        n.remote.my_name = my_name;
        n
    }

    pub fn join(host_ip: &str, my_name: [u8; 12]) -> Self {
        let socket = UdpSocket::bind(("0.0.0.0", 0)).ok();
        let mut n = Net::offline();
        n.my_id(255);
        n.remote.my_name = my_name;
        if let Some(s) = socket {
            let _ = s.set_nonblocking(true);
            if let Ok(addr) = format!("{host_ip}:{PORT}").parse() {
                n.host_addr = Some(addr);
                n.socket = Some(s);
                n.role = Role::Client;
                // send hello until welcome arrives (main loop retries)
            } else {
                n.role = Role::Offline;
            }
        }
        n
    }

    fn my_id(&mut self, id: u8) {
        self.remote.id = id;
    }

    pub fn connected(&self) -> bool {
        match self.role {
            Role::Offline => false,
            Role::Host => true,
            Role::Client => self.remote.id != 255,
        }
    }

    pub fn status_line(&self) -> String {
        match self.role {
            Role::Offline => "OFFLINE".into(),
            Role::Host => format!("HOST - {} PLAYER(S) - PORT {}", self.clients.len() + 1, PORT),
            Role::Client if !self.connected() => "CONNECTING...".into(),
            Role::Client => format!("CONNECTED AS PLAYER #{}", self.remote.id),
        }
    }

    // ---------------- send ----------------

    fn send(&self, buf: &[u8], to: &std::net::SocketAddr) {
        if let Some(s) = &self.socket {
            let _ = s.send_to(buf, to);
        }
    }

    pub fn broadcast(&self, buf: &[u8], except_id: Option<u8>) {
        for (id, (addr, _)) in &self.clients {
            if Some(*id) == except_id {
                continue;
            }
            self.send(buf, addr);
        }
    }

    pub fn send_hello(&self) {
        if self.role != Role::Client {
            return;
        }
        let Some(host) = self.host_addr else { return };
        let mut buf = vec![MSG_HELLO];
        buf.extend_from_slice(&self.remote.my_name);
        self.send(&buf, &host);
    }

    pub fn send_pos(&mut self, pos: [f32; 3], yaw: f32, pitch: f32, dt: f32) {
        if self.role == Role::Offline || !self.connected() {
            return;
        }
        self.pos_timer -= dt;
        if self.pos_timer > 0.0 {
            return;
        }
        self.pos_timer = 0.1; // 10 Hz
        let mut buf = Vec::with_capacity(27);
        buf.push(MSG_POS);
        buf.push(self.remote.id);
        for v in pos {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf.extend_from_slice(&yaw.to_le_bytes());
        buf.extend_from_slice(&pitch.to_le_bytes());
        if self.role == Role::Client {
            if let Some(h) = self.host_addr {
                self.send(&buf, &h);
            }
        } else {
            self.broadcast(&buf, Some(self.remote.id));
        }
    }

    pub fn send_digs(&mut self, indices: &[u32]) {
        if self.role == Role::Offline || indices.is_empty() {
            return;
        }
        for chunk in indices.chunks(DIGS_CHUNK) {
            let mut buf = Vec::with_capacity(6 + chunk.len() * 4);
            buf.push(MSG_DIGS);
            buf.push(self.remote.id);
            buf.extend_from_slice(&(chunk.len() as u16).to_le_bytes());
            for i in chunk {
                buf.extend_from_slice(&i.to_le_bytes());
            }
            if self.role == Role::Client {
                if let Some(h) = self.host_addr {
                    self.send(&buf, &h);
                }
            } else {
                self.broadcast(&buf, Some(self.remote.id));
            }
        }
    }

    pub fn send_valuable(&mut self, idx: u32) {
        if self.role == Role::Offline {
            return;
        }
        let mut buf = Vec::with_capacity(6);
        buf.push(MSG_VALUABLE);
        buf.push(self.remote.id);
        buf.extend_from_slice(&idx.to_le_bytes());
        if self.role == Role::Client {
            if let Some(h) = self.host_addr {
                self.send(&buf, &h);
            }
        } else {
            self.broadcast(&buf, Some(self.remote.id));
        }
    }

    pub fn send_found(&mut self, time_s: f32) {
        if self.role == Role::Offline {
            return;
        }
        let mut buf = Vec::with_capacity(6);
        buf.push(MSG_FOUND);
        buf.push(self.remote.id);
        buf.extend_from_slice(&time_s.to_le_bytes());
        if self.role == Role::Client {
            if let Some(h) = self.host_addr {
                self.send(&buf, &h);
            }
        } else {
            self.broadcast(&buf, None);
        }
    }

    // ---------------- receive ----------------

    pub fn poll(&mut self, dt: f32) {
        let Some(s) = self.socket.as_ref() else { return };
        let mut msgs: Vec<(Vec<u8>, std::net::SocketAddr)> = Vec::new();
        let mut buf = [0u8; 1500];
        loop {
            match s.recv_from(&mut buf) {
                Ok((n, from)) => msgs.push((buf[..n].to_vec(), from)),
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        for (m, from) in msgs {
            self.handle(&m, from, dt);
        }
        // timeouts
        for p in self.remote.players.iter_mut() {
            if p.active {
                p.last_seen += dt;
                if p.last_seen > 12.0 {
                    p.active = false;
                }
            }
        }
        if self.role == Role::Host {
            let dead: Vec<u8> = self
                .clients
                .iter()
                .filter(|(_, (_, t))| *t > 12.0)
                .map(|(k, _)| *k)
                .collect();
            for id in dead {
                self.clients.remove(&id);
                self.remote.players[id as usize].active = false;
            }
        }
    }

    fn handle(&mut self, buf: &[u8], from: std::net::SocketAddr, dt: f32) {
        if buf.is_empty() {
            return;
        }
        let msg = buf[0];
        match msg {
            MSG_HELLO => {
                if self.role != Role::Host || buf.len() < 13 {
                    return;
                }
                if self.next_id >= MAX_PLAYERS as u8 {
                    return;
                }
                let id = self.next_id;
                self.next_id += 1;
                self.clients.insert(id, (from, 0.0));
                let pid = id as usize;
                if pid < MAX_PLAYERS {
                    let p = &mut self.remote.players[pid];
                    p.active = true;
                    p.last_seen = 0.0;
                    p.name.copy_from_slice(&buf[1..13]);
                }
                // welcome: seed+count are filled by main (see set_seed_welcome)
                let mut w = vec![MSG_WELCOME, id];
                w.extend_from_slice(&self.welcome_seed.unwrap_or(1).to_le_bytes());
                w.extend_from_slice(&self.welcome_count.unwrap_or(0).to_le_bytes());
                self.send(&w, &from);
                // tell everyone about the newcomer via pos snapshot soon
            }
            MSG_WELCOME => {
                if self.role != Role::Client || buf.len() < 13 {
                    return;
                }
                self.remote.id = buf[1];
                let seed = u64::from_le_bytes(buf[2..10].try_into().unwrap());
                let count = u32::from_le_bytes(buf[10..14].try_into().unwrap());
                self.welcome_seed = Some(seed);
                self.welcome_count = Some(count);
            }
            MSG_POS => {
                if buf.len() < 27 {
                    return;
                }
                let id = buf[1] as usize;
                let rd = |o: usize| f32::from_le_bytes(buf[o..o + 4].try_into().unwrap());
                if self.role == Role::Host && id < MAX_PLAYERS {
                    if let Some((addr, t)) = self.clients.get_mut(&(id as u8)) {
                        *addr = from;
                        *t = 0.0;
                    }
                    // relay to other clients
                    self.broadcast(&buf[..27], Some(id as u8));
                }
                if id < MAX_PLAYERS && id != self.remote.id as usize {
                    let p = &mut self.remote.players[id];
                    p.active = true;
                    p.last_seen = 0.0;
                    p.pos = [rd(2), rd(6), rd(10)];
                    p.yaw = rd(14);
                    p.pitch = rd(18);
                }
            }
            MSG_DIGS => {
                if buf.len() < 5 {
                    return;
                }
                let _id = buf[1];
                let n = u16::from_le_bytes(buf[2..4].try_into().unwrap()) as usize;
                if buf.len() < 4 + n * 4 {
                    return;
                }
                if self.role == Role::Host {
                    self.broadcast(&buf[..4 + n * 4], None);
                }
                for k in 0..n {
                    let i = u32::from_le_bytes(buf[4 + k * 4..8 + k * 4].try_into().unwrap());
                    self.remote.pending_digs.push(i);
                }
            }
            MSG_VALUABLE => {
                if buf.len() < 6 {
                    return;
                }
                if self.role == Role::Host {
                    self.broadcast(&buf[..6], None);
                }
                let idx = u32::from_le_bytes(buf[2..6].try_into().unwrap());
                self.remote.pending_valuables.push(idx);
            }
            MSG_FOUND => {
                if buf.len() < 6 {
                    return;
                }
                if self.role == Role::Host {
                    self.broadcast(&buf[..6], None);
                }
                self.remote.found_by = Some(buf[1]);
            }
            MSG_BYE => {
                if self.role == Role::Host && buf[1] < MAX_PLAYERS as u8 {
                    self.clients.remove(&buf[1]);
                    self.remote.players[buf[1] as usize].active = false;
                }
            }
            _ => {}
        }
        let _ = dt;
    }

    // fields injected by main for the welcome payload
    pub fn set_world_info(&mut self, seed: u64, count: u32) {
        self.welcome_seed = Some(seed);
        self.welcome_count = Some(count);
    }
}
