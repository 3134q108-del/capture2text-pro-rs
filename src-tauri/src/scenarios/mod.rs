use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScenarioStore {
    active_id: String,
    scenarios: Vec<Scenario>,
}

#[derive(Debug)]
struct ScenarioRuntime {
    path: PathBuf,
    active_id: String,
    scenarios: Vec<Scenario>,
}

static SCENARIO_RUNTIME: OnceLock<Mutex<ScenarioRuntime>> = OnceLock::new();

pub fn builtin_default() -> Vec<Scenario> {
    vec![
        Scenario {
            id: "default".to_string(),
            name: "通用".to_string(),
            prompt: "你是精準的翻譯助理。處理來自 OCR 的文字片段，可能不完整。忠實翻譯，不加解釋。"
                .to_string(),
            builtin: true,
        },
        Scenario {
            id: "maritime".to_string(),
            name: "航運/輪機".to_string(),
            prompt: "使用者是輪機員，工作場景為商船和貨櫃船。請用台灣航運業慣用術語翻譯，英文專業縮寫（M/E、TEU、B/L、reefer、bunker）請保留原文並加中文註解。OCR 片段不完整時盡力推斷。"
                .to_string(),
            builtin: true,
        },
        Scenario {
            id: "game".to_string(),
            name: "遊戲".to_string(),
            prompt: "這是遊戲介面或對白文字。使用遊戲社群慣用譯法，保留專有名詞原文（角色名、裝備、技能）。"
                .to_string(),
            builtin: true,
        },
        Scenario {
            id: "code".to_string(),
            name: "程式碼/技術".to_string(),
            prompt: "這是程式碼或技術文件。程式關鍵字、API 名稱、變數名保持英文原文，只翻註解和一般敘述。"
                .to_string(),
            builtin: true,
        },
        Scenario {
            id: "medical".to_string(),
            name: "醫療".to_string(),
            prompt: "這是醫療文件。請用台灣醫學會慣用術語，不確定的專有名詞保留英文並加括號中文試譯。"
                .to_string(),
            builtin: true,
        },
    ]
}

pub fn storage_path() -> io::Result<PathBuf> {
    let local = dirs::data_local_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "local appdata not found"))?;
    Ok(local.join("Capture2TextPro").join("scenarios.json"))
}

pub fn load_scenarios() -> io::Result<Vec<Scenario>> {
    let path = storage_path()?;
    if !path.exists() {
        return Ok(builtin_default());
    }

    let raw = fs::read_to_string(&path)?;
    let store: ScenarioStore = serde_json::from_str(&raw)
        .map_err(|err| io::Error::other(format!("parse scenarios.json failed: {err}")))?;
    Ok(merge_builtin(store.scenarios))
}

pub fn save_scenarios(scenarios: &[Scenario]) -> io::Result<()> {
    let path = storage_path()?;
    write_store(
        &path,
        &ScenarioStore {
            active_id: "default".to_string(),
            scenarios: merge_builtin(scenarios.to_vec()),
        },
    )
}

pub fn init_runtime() -> io::Result<()> {
    if SCENARIO_RUNTIME.get().is_some() {
        return Ok(());
    }

    let path = storage_path()?;
    let store = read_or_default_store(&path)?;
    let merged = merge_builtin(store.scenarios);
    let active = sanitize_active_id(&merged, &store.active_id);

    write_store(
        &path,
        &ScenarioStore {
            active_id: active.clone(),
            scenarios: merged.clone(),
        },
    )?;

    let _ = SCENARIO_RUNTIME.set(Mutex::new(ScenarioRuntime {
        path,
        active_id: active,
        scenarios: merged,
    }));
    Ok(())
}

pub fn list_runtime() -> io::Result<Vec<Scenario>> {
    let runtime = runtime_guard()?;
    Ok(runtime.scenarios.clone())
}

pub fn upsert_runtime(scenario: Scenario) -> io::Result<()> {
    let mut runtime = runtime_guard()?;
    if let Some(existing) = runtime.scenarios.iter_mut().find(|item| item.id == scenario.id) {
        existing.name = scenario.name;
        existing.prompt = scenario.prompt;
        if !existing.builtin {
            existing.builtin = scenario.builtin;
        }
    } else {
        runtime.scenarios.push(scenario);
    }
    runtime.scenarios = merge_builtin(runtime.scenarios.clone());
    persist_runtime(&runtime)
}

pub fn delete_runtime(id: &str) -> io::Result<()> {
    let mut runtime = runtime_guard()?;
    if let Some(s) = runtime.scenarios.iter().find(|item| item.id == id) {
        if s.builtin {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "builtin scenario cannot be deleted",
            ));
        }
    }

    runtime.scenarios.retain(|item| item.id != id);
    runtime.scenarios = merge_builtin(runtime.scenarios.clone());
    runtime.active_id = sanitize_active_id(&runtime.scenarios, &runtime.active_id);
    persist_runtime(&runtime)
}

pub fn get_active_scenario_id() -> String {
    match runtime_guard() {
        Ok(runtime) => runtime.active_id.clone(),
        Err(_) => "default".to_string(),
    }
}

pub fn set_active_scenario_id(id: String) -> io::Result<()> {
    let mut runtime = runtime_guard()?;
    runtime.active_id = sanitize_active_id(&runtime.scenarios, &id);
    persist_runtime(&runtime)
}

pub fn current_scenario() -> Scenario {
    match runtime_guard() {
        Ok(runtime) => runtime
            .scenarios
            .iter()
            .find(|item| item.id == runtime.active_id)
            .cloned()
            .unwrap_or_else(default_scenario),
        Err(_) => default_scenario(),
    }
}

fn runtime_guard() -> io::Result<std::sync::MutexGuard<'static, ScenarioRuntime>> {
    let runtime = SCENARIO_RUNTIME
        .get()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "scenario runtime not initialized"))?;
    runtime
        .lock()
        .map_err(|_| io::Error::other("scenario runtime lock poisoned"))
}

fn read_or_default_store(path: &PathBuf) -> io::Result<ScenarioStore> {
    if !path.exists() {
        return Ok(ScenarioStore {
            active_id: "default".to_string(),
            scenarios: builtin_default(),
        });
    }

    let raw = fs::read_to_string(path)?;
    let parsed = serde_json::from_str::<ScenarioStore>(&raw)
        .map_err(|err| io::Error::other(format!("parse scenarios.json failed: {err}")))?;
    Ok(parsed)
}

fn merge_builtin(mut scenarios: Vec<Scenario>) -> Vec<Scenario> {
    let builtins = builtin_default();
    for builtin in builtins {
        match scenarios.iter_mut().find(|item| item.id == builtin.id) {
            Some(existing) => {
                existing.builtin = true;
                if existing.name.trim().is_empty() {
                    existing.name = builtin.name;
                }
                if existing.prompt.trim().is_empty() {
                    existing.prompt = builtin.prompt;
                }
            }
            None => scenarios.push(builtin),
        }
    }
    scenarios
}

fn sanitize_active_id(scenarios: &[Scenario], current: &str) -> String {
    if scenarios.iter().any(|item| item.id == current) {
        return current.to_string();
    }
    "default".to_string()
}

fn write_store(path: &PathBuf, store: &ScenarioStore) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(store)
        .map_err(|err| io::Error::other(format!("serialize scenario store failed: {err}")))?;
    fs::write(path, content)?;
    Ok(())
}

fn persist_runtime(runtime: &ScenarioRuntime) -> io::Result<()> {
    write_store(
        &runtime.path,
        &ScenarioStore {
            active_id: runtime.active_id.clone(),
            scenarios: runtime.scenarios.clone(),
        },
    )
}

fn default_scenario() -> Scenario {
    builtin_default()
        .into_iter()
        .find(|item| item.id == "default")
        .unwrap_or(Scenario {
            id: "default".to_string(),
            name: "通用".to_string(),
            prompt: "你是精準的翻譯助理。".to_string(),
            builtin: true,
        })
}
