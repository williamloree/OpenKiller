//! Fake data for `--demo` and `--screenshot`.

use crate::memory::MemInfo;
use crate::model::{Favorite, PortEntry, Proto};

/// port, protocol, ip, pid, name, RAM (MB), path
type Row = (
    u16,
    Proto,
    &'static str,
    u32,
    &'static str,
    Option<u64>,
    Option<&'static str>,
);

pub fn ports() -> Vec<PortEntry> {
    let rows: [Row; 16] = [
        (
            80,
            Proto::Tcp,
            "0.0.0.0",
            6120,
            "caddy",
            Some(38),
            Some(r"C:\Tools\caddy\caddy.exe"),
        ),
        (135, Proto::Tcp, "0.0.0.0", 1184, "svchost", None, None),
        (
            443,
            Proto::Tcp,
            "0.0.0.0",
            6120,
            "caddy",
            Some(38),
            Some(r"C:\Tools\caddy\caddy.exe"),
        ),
        (445, Proto::Tcp, "0.0.0.0", 4, "System", None, None),
        (
            2375,
            Proto::Tcp,
            "127.0.0.1",
            9012,
            "com.docker.backend",
            Some(212),
            Some(r"C:\Program Files\Docker\Docker\resources\com.docker.backend.exe"),
        ),
        (
            3000,
            Proto::Tcp,
            "0.0.0.0",
            14820,
            "node",
            Some(164),
            Some(r"C:\Program Files\nodejs\node.exe"),
        ),
        (
            5173,
            Proto::Tcp,
            "[::1]",
            15376,
            "node",
            Some(97),
            Some(r"C:\Program Files\nodejs\node.exe"),
        ),
        (
            5353,
            Proto::Udp,
            "0.0.0.0",
            11204,
            "chrome",
            Some(286),
            Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        ),
        (
            5432,
            Proto::Tcp,
            "0.0.0.0",
            5068,
            "postgres",
            Some(26),
            Some(r"C:\Program Files\PostgreSQL\17\bin\postgres.exe"),
        ),
        (
            6379,
            Proto::Tcp,
            "127.0.0.1",
            7740,
            "redis-server",
            Some(9),
            Some(r"C:\Tools\redis\redis-server.exe"),
        ),
        (
            8000,
            Proto::Tcp,
            "127.0.0.1",
            16032,
            "python",
            Some(41),
            Some(r"C:\Users\dev\AppData\Local\Programs\Python\Python313\python.exe"),
        ),
        (
            9229,
            Proto::Tcp,
            "127.0.0.1",
            13388,
            "Code",
            Some(512),
            Some(r"C:\Users\dev\AppData\Local\Programs\Microsoft VS Code\Code.exe"),
        ),
        (
            27017,
            Proto::Tcp,
            "127.0.0.1",
            5544,
            "mongod",
            Some(118),
            Some(r"C:\Program Files\MongoDB\Server\8.0\bin\mongod.exe"),
        ),
        (49664, Proto::Tcp, "0.0.0.0", 1032, "lsass", None, None),
        (
            57621,
            Proto::Udp,
            "0.0.0.0",
            12960,
            "Spotify",
            Some(178),
            Some(r"C:\Users\dev\AppData\Roaming\Spotify\Spotify.exe"),
        ),
        (5050, Proto::Udp, "0.0.0.0", 1184, "svchost", None, None),
    ];
    rows.iter()
        .map(|&(port, proto, ip, pid, name, memory_mb, path)| PortEntry {
            port,
            proto,
            address: format!("{ip}:{port}"),
            pid,
            name: name.into(),
            memory_mb,
            path: path.map(str::to_string),
        })
        .collect()
}

pub fn favorites() -> Vec<Favorite> {
    [
        (3000, "Nuxt / Node dev", "Front du projet client"),
        (5173, "Vite dev", ""),
        (5432, "PostgreSQL", "Base locale"),
        (4200, "Angular dev", ""),
    ]
    .iter()
    .map(|&(port, name, description)| Favorite {
        port,
        name: name.into(),
        description: description.into(),
    })
    .collect()
}

pub fn memory() -> MemInfo {
    const GB: f64 = 1_073_741_824.0;
    MemInfo {
        total: (16.0 * GB) as u64,
        used: (9.8 * GB) as u64,
        freeable: (2.1 * GB) as u64,
    }
}
