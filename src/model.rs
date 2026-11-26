//! Pure application state: port rows, filtering, sorting, favorites validation, presets.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    pub fn as_str(self) -> &'static str {
        match self {
            Proto::Tcp => "TCP",
            Proto::Udp => "UDP",
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct PortEntry {
    pub port: u16,
    pub proto: Proto,
    pub address: String,
    pub pid: u32,
    pub name: String,
    pub memory_mb: Option<u64>,
    pub path: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProtoFilter {
    All,
    Tcp,
    Udp,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKey {
    Port,
    App,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sort {
    pub key: SortKey,
    pub desc: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Sort {
            key: SortKey::Port,
            desc: false,
        }
    }
}

impl Sort {
    /// Header click: same column flips the order, another column starts ascending.
    pub fn click(&mut self, key: SortKey) {
        if self.key == key {
            self.desc = !self.desc;
        } else {
            *self = Sort { key, desc: false };
        }
    }
}

/// Rows shown for a search text (port or application name, case-insensitive), protocol filter and sort.
pub fn visible(
    ports: &[PortEntry],
    search: &str,
    filter: ProtoFilter,
    sort: Sort,
) -> Vec<PortEntry> {
    let term = search.trim().to_lowercase();
    let mut rows: Vec<PortEntry> = ports
        .iter()
        .filter(|p| match filter {
            ProtoFilter::All => true,
            ProtoFilter::Tcp => p.proto == Proto::Tcp,
            ProtoFilter::Udp => p.proto == Proto::Udp,
        })
        .filter(|p| {
            term.is_empty()
                || p.port.to_string().contains(&term)
                || p.name.to_lowercase().contains(&term)
        })
        .cloned()
        .collect();
    rows.sort_by(|a, b| {
        let ord = match sort.key {
            SortKey::Port => a.port.cmp(&b.port).then(a.proto.cmp(&b.proto)),
            SortKey::App => a
                .name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.port.cmp(&b.port)),
        };
        if sort.desc { ord.reverse() } else { ord }
    });
    rows
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n > 1 { "s" } else { "" })
}

/// "3 ports (filtrés sur 15)" or "15 ports".
pub fn count_label(shown: usize, total: usize) -> String {
    if shown == total {
        plural(shown, "port")
    } else {
        format!("{} (filtrés sur {total})", plural(shown, "port"))
    }
}

pub fn favorites_label(n: usize) -> String {
    plural(n, "favori")
}

/// Distinct PIDs of the given rows, in first-seen order, with their name and ports.
pub fn distinct_pids(rows: &[&PortEntry]) -> Vec<(u32, String, Vec<u16>)> {
    let mut out: Vec<(u32, String, Vec<u16>)> = Vec::new();
    for r in rows {
        match out.iter_mut().find(|(pid, _, _)| *pid == r.pid) {
            Some((_, _, ports)) => {
                if !ports.contains(&r.port) {
                    ports.push(r.port);
                }
            }
            None => out.push((r.pid, r.name.clone(), vec![r.port])),
        }
    }
    out
}

/// PID 0 (idle), 4 (System) and OpenKiller itself are never killed.
pub fn is_protected(pid: u32, self_pid: u32) -> bool {
    pid == 0 || pid == 4 || pid == self_pid
}

/// Bytes to "9,8 Go" (French decimal comma).
pub fn fmt_gb(bytes: u64) -> String {
    format!("{:.1} Go", bytes as f64 / 1_073_741_824.0).replace('.', ",")
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Favorite {
    pub port: u16,
    pub name: String,
    pub description: String,
}

pub const NAME_MAX: usize = 100;
pub const DESCRIPTION_MAX: usize = 500;

pub const PRESETS: [(u16, &str); 12] = [
    (3000, "Nuxt / Node dev"),
    (5173, "Vite dev"),
    (4200, "Angular dev"),
    (8000, "Django dev"),
    (5000, "Flask dev"),
    (80, "Caddy / HTTP"),
    (443, "Caddy / HTTPS"),
    (8080, "HTTP alternatif"),
    (5432, "PostgreSQL"),
    (3306, "MySQL"),
    (6379, "Redis"),
    (27017, "MongoDB"),
];

/// Validates the favorite form (same messages as v1). `editing` = port of the favorite being edited.
pub fn validate_favorite(
    favorites: &[Favorite],
    editing: Option<u16>,
    port: &str,
    name: &str,
    description: &str,
) -> Result<Favorite, &'static str> {
    let port = match port.trim().parse::<u32>() {
        Ok(p @ 1..=65535) => p as u16,
        _ => return Err("Port invalide. Doit être entre 1 et 65535"),
    };
    let name = name.trim();
    if name.is_empty() {
        return Err("Le nom est requis");
    }
    if editing != Some(port) && favorites.iter().any(|f| f.port == port) {
        return Err("Ce port est déjà dans vos favoris");
    }
    Ok(Favorite {
        port,
        name: name.chars().take(NAME_MAX).collect(),
        description: description.trim().chars().take(DESCRIPTION_MAX).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(port: u16, proto: Proto, pid: u32, name: &str) -> PortEntry {
        PortEntry {
            port,
            proto,
            address: format!("0.0.0.0:{port}"),
            pid,
            name: name.into(),
            memory_mb: None,
            path: None,
        }
    }

    fn sample() -> Vec<PortEntry> {
        vec![
            e(5173, Proto::Tcp, 10, "node"),
            e(3000, Proto::Tcp, 11, "Node"),
            e(445, Proto::Tcp, 4, "System"),
            e(5353, Proto::Udp, 12, "chrome"),
            e(51730, Proto::Udp, 13, "python"),
        ]
    }

    #[test]
    fn search_by_name_case_insensitive() {
        let r = visible(&sample(), "NODE", ProtoFilter::All, Sort::default());
        assert_eq!(
            r.iter().map(|p| p.port).collect::<Vec<_>>(),
            vec![3000, 5173]
        );
    }

    #[test]
    fn search_by_port() {
        let r = visible(&sample(), "5173", ProtoFilter::All, Sort::default());
        assert_eq!(
            r.iter().map(|p| p.port).collect::<Vec<_>>(),
            vec![5173, 51730]
        );
    }

    #[test]
    fn protocol_filter() {
        let r = visible(&sample(), "", ProtoFilter::Udp, Sort::default());
        assert!(r.iter().all(|p| p.proto == Proto::Udp));
        assert_eq!(r.len(), 2);
        assert_eq!(
            visible(&sample(), "", ProtoFilter::Tcp, Sort::default()).len(),
            3
        );
    }

    #[test]
    fn sorting() {
        let mut s = Sort::default();
        let r = visible(&sample(), "", ProtoFilter::All, s);
        assert_eq!(r[0].port, 445);
        s.click(SortKey::Port);
        assert!(s.desc);
        assert_eq!(visible(&sample(), "", ProtoFilter::All, s)[0].port, 51730);
        s.click(SortKey::App);
        assert_eq!(
            s,
            Sort {
                key: SortKey::App,
                desc: false
            }
        );
        let names: Vec<_> = visible(&sample(), "", ProtoFilter::All, s)
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["chrome", "Node", "node", "python", "System"]);
    }

    #[test]
    fn labels() {
        assert_eq!(count_label(1, 1), "1 port");
        assert_eq!(count_label(15, 15), "15 ports");
        assert_eq!(count_label(3, 15), "3 ports (filtrés sur 15)");
        assert_eq!(favorites_label(0), "0 favori");
        assert_eq!(fmt_gb(10_522_669_875), "9,8 Go");
    }

    #[test]
    fn pids_are_distinct() {
        let rows = [
            e(1, Proto::Tcp, 7, "a"),
            e(2, Proto::Tcp, 7, "a"),
            e(3, Proto::Udp, 9, "b"),
        ];
        let refs: Vec<&PortEntry> = rows.iter().collect();
        let d = distinct_pids(&refs);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0], (7, "a".to_string(), vec![1, 2]));
    }

    #[test]
    fn protected_pids() {
        assert!(is_protected(0, 99));
        assert!(is_protected(4, 99));
        assert!(is_protected(99, 99));
        assert!(!is_protected(1234, 99));
    }

    #[test]
    fn favorite_validation() {
        let favs = vec![Favorite {
            port: 5173,
            name: "Vite".into(),
            description: String::new(),
        }];
        assert_eq!(
            validate_favorite(&favs, None, "0", "x", ""),
            Err("Port invalide. Doit être entre 1 et 65535")
        );
        assert_eq!(
            validate_favorite(&favs, None, "70000", "x", ""),
            Err("Port invalide. Doit être entre 1 et 65535")
        );
        assert_eq!(
            validate_favorite(&favs, None, "abc", "x", ""),
            Err("Port invalide. Doit être entre 1 et 65535")
        );
        assert_eq!(
            validate_favorite(&favs, None, "3000", "  ", ""),
            Err("Le nom est requis")
        );
        assert_eq!(
            validate_favorite(&favs, None, "5173", "Vite 2", ""),
            Err("Ce port est déjà dans vos favoris")
        );
        // Editing the same favorite keeps its port.
        assert!(validate_favorite(&favs, Some(5173), "5173", "Vite", "dev").is_ok());
        let ok = validate_favorite(&favs, None, " 3000 ", " Nuxt ", &"d".repeat(600)).unwrap();
        assert_eq!(
            (ok.port, ok.name.as_str(), ok.description.len()),
            (3000, "Nuxt", DESCRIPTION_MAX)
        );
    }
}
