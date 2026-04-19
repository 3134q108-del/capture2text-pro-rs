use crate::scenarios::{self, Scenario};

#[tauri::command]
pub fn list_scenarios() -> Result<Vec<Scenario>, String> {
    scenarios::list_runtime().map_err(|err| err.to_string())
}

#[tauri::command]
pub fn save_scenario(scenario: Scenario) -> Result<(), String> {
    if scenario.id.trim().is_empty() {
        return Err("scenario id cannot be empty".to_string());
    }
    scenarios::upsert_runtime(scenario).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn delete_scenario(id: String) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("scenario id cannot be empty".to_string());
    }
    scenarios::delete_runtime(&id).map_err(|err| err.to_string())
}

#[tauri::command]
pub fn get_active_scenario() -> Result<String, String> {
    Ok(scenarios::get_active_scenario_id())
}

#[tauri::command]
pub fn set_active_scenario(id: String) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("scenario id cannot be empty".to_string());
    }
    scenarios::set_active_scenario_id(id).map_err(|err| err.to_string())
}
