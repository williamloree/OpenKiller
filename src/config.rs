//! %APPDATA%\openkiller\config.json: favorites, window rect, auto_update. Tolerant read, atomic write.

use crate::model::{DESCRIPTION_MAX, Favorite, NAME_MAX};
use serde_json::{Value, json};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Debug)]
pub struct Config {
    pub favorites: Vec<Favorite>,
    /// x, y, w, h in physical pixels.
    pub window: Option<[i32; 4]>,
    pub auto_update: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            favorites: Vec::new(),
            window: None,
            auto_update: true,
        }
    }
}

/// Any missing or invalid field falls back to its default; bad favorites are dropped.
pub fn parse(text: &str) -> Config {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let mut cfg = Config::default();
    if let Some(b) = v.get("auto_update").and_then(Value::as_bool) {
        cfg.auto_update = b;
    }
    if let Some(w) = v.get("window") {
        let n = |k: &str| {
            w.get(k)
                .and_then(Value::as_i64)
                .and_then(|x| i32::try_from(x).ok())
        };
        if let (Some(x), Some(y), Some(w), Some(h)) = (n("x"), n("y"), n("w"), n("h"))
            && w > 0
            && h > 0
        {
            cfg.window = Some([x, y, w, h]);
        }
    }
    for f in v
        .get("favorites")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let port = f.get("port").and_then(Value::as_u64).unwrap_or(0);
        let name = f.get("name").and_then(Value::as_str).unwrap_or("").trim();
        if !(1..=65535).contains(&port)
            || name.is_empty()
            || cfg.favorites.iter().any(|x| u64::from(x.port) == port)
        {
            continue;
        }
        cfg.favorites.push(Favorite {
            port: port as u16,
            name: name.chars().take(NAME_MAX).collect(),
            description: f
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(DESCRIPTION_MAX)
                .collect(),
        });
    }
    cfg
}

pub fn to_json(cfg: &Config) -> String {
    let favorites: Vec<Value> = cfg
        .favorites
        .iter()
        .map(|f| json!({ "port": f.port, "name": f.name, "description": f.description }))
        .collect();
    let mut v = json!({ "favorites": favorites, "auto_update": cfg.auto_update });
    if let Some([x, y, w, h]) = cfg.window {
        v["window"] = json!({ "x": x, "y": y, "w": w, "h": h });
    }
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

fn path() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join(env!("CARGO_PKG_NAME"))
            .join("config.json"),
    )
}

pub fn load() -> Config {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|t| parse(&t))
        .unwrap_or_default()
}

/// Write to a temp file then rename, so a crash never leaves a half-written config.
pub fn save(cfg: &Config) -> std::io::Result<()> {
    let p = path().ok_or_else(|| std::io::Error::other("APPDATA not set"))?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, to_json(cfg))?;
    std::fs::rename(&tmp, &p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_broken_gives_defaults() {
        assert_eq!(parse(""), Config::default());
        assert_eq!(parse("{}"), Config::default());
        assert_eq!(parse("[1,2]"), Config::default());
        assert_eq!(
            parse(r#"{"auto_update":"yes","window":{"x":1}}"#),
            Config::default()
        );
    }

    #[test]
    fn favorites_are_sanitised() {
        let c = parse(&format!(
            r#"{{"favorites":[
                {{"port":5173,"name":"Vite"}},
                {{"port":0,"name":"zero"}},
                {{"port":70000,"name":"big"}},
                {{"port":"80","name":"string"}},
                {{"port":3000,"name":"  "}},
                {{"port":5173,"name":"dup"}},
                {{"port":3000,"name":"{}","description":"{}"}}
            ],"auto_update":false,"window":{{"x":-5,"y":10,"w":1000,"h":700}}}}"#,
            "n".repeat(150),
            "d".repeat(900)
        ));
        assert_eq!(c.favorites.len(), 2);
        assert_eq!(
            c.favorites[0],
            Favorite {
                port: 5173,
                name: "Vite".into(),
                description: String::new()
            }
        );
        assert_eq!(c.favorites[1].name.len(), NAME_MAX);
        assert_eq!(c.favorites[1].description.len(), DESCRIPTION_MAX);
        assert!(!c.auto_update);
        assert_eq!(c.window, Some([-5, 10, 1000, 700]));
    }

    #[test]
    fn round_trip() {
        let c = Config {
            favorites: vec![Favorite {
                port: 80,
                name: "Caddy \"HTTP\"".into(),
                description: "é".into(),
            }],
            window: Some([1, 2, 3, 4]),
            auto_update: false,
        };
        assert_eq!(parse(&to_json(&c)), c);
    }
}
