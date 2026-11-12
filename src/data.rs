use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub folders: Vec<Folder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub routes: Vec<Route>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    pub id: String,
    pub name: String,
    pub kind: RouteKind,
    pub method: String,
    pub url: String,
    pub params: String,
    #[serde(default)]
    pub param_rows: Option<Vec<ParamRow>>,
    pub headers: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamRow {
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "text_type")]
    pub kind: String,
    #[serde(default)]
    pub description: String,
}

impl Default for ParamRow {
    fn default() -> Self {
        Self {
            enabled: true,
            name: String::new(),
            value: String::new(),
            kind: text_type(),
            description: String::new(),
        }
    }
}

fn enabled() -> bool {
    true
}

fn text_type() -> String {
    "Text".into()
}

pub fn legacy_params(input: &str) -> Vec<ParamRow> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (name, value) = line.split_once('=').unwrap_or((line, ""));
            let name = name.trim();
            ParamRow {
                name: name.to_owned(),
                value: value.trim().to_owned(),
                ..ParamRow::default()
            }
        })
        .filter(|row| !row.name.is_empty())
        .collect()
}

pub fn legacy_params_text(rows: &[ParamRow]) -> String {
    rows.iter()
        .filter(|row| row.enabled && !row.name.trim().is_empty())
        .map(|row| format!("{}={}", row.name.trim(), row.value))
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteKind {
    Rest,
    WebSocket,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            folders: vec![Folder {
                id: new_id(),
                name: "My workspace".into(),
                routes: Vec::new(),
            }],
        }
    }
}

pub fn new_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{nanos}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
}

fn config_file() -> Result<PathBuf, String> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| "No encuentro tu directorio de configuración.".to_owned())?;
    Ok(base.join("apui").join("workspace.json"))
}

pub fn load() -> Result<Workspace, String> {
    let path = config_file()?;
    match fs::read(&path) {
        Ok(bytes) => {
            let workspace: Workspace = serde_json::from_slice(&bytes)
                .map_err(|error| format!("No pude leer {}: {error}", path.display()))?;
            if workspace.folders.is_empty() {
                return Err(format!("{} no contiene carpetas válidas.", path.display()));
            }
            let mut ids = HashSet::new();
            let mut names = HashSet::new();
            for folder in &workspace.folders {
                if folder.id.is_empty()
                    || folder.name.trim().is_empty()
                    || !ids.insert(folder.id.as_str())
                    || !names.insert(folder.name.to_lowercase())
                {
                    return Err(format!(
                        "{} contiene carpetas duplicadas o inválidas.",
                        path.display()
                    ));
                }
                for route in &folder.routes {
                    if route.id.is_empty()
                        || route.name.trim().is_empty()
                        || !ids.insert(route.id.as_str())
                    {
                        return Err(format!(
                            "{} contiene rutas duplicadas o inválidas.",
                            path.display()
                        ));
                    }
                }
            }
            Ok(workspace)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Workspace::default()),
        Err(error) => Err(format!("No pude leer {}: {error}", path.display())),
    }
}

pub fn save(workspace: &Workspace) -> Result<(), String> {
    let path = config_file()?;
    let parent = path.parent().expect("config path always has a parent");
    fs::create_dir_all(parent)
        .map_err(|error| format!("No pude crear {}: {error}", parent.display()))?;
    let bytes = serde_json::to_vec_pretty(workspace)
        .map_err(|error| format!("No pude preparar el espacio de trabajo: {error}"))?;
    atomic_write(&path, &bytes)
        .map_err(|error| format!("No pude guardar {}: {error}", path.display()))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_extension(format!("{}-{nonce}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&temporary, path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}
