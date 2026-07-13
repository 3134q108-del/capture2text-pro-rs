use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InventoryItem {
    pub id: String,
    pub path: String,
    pub size: u64,
    pub category: String,
    pub removable: bool,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Inventory {
    pub version: String,
    pub items: Vec<InventoryItem>,
}

#[derive(Clone, Copy)]
struct KnownItem {
    id: &'static str,
    path: &'static str,
    category: &'static str,
    removable: bool,
    label: &'static str,
}

const KNOWN_ITEMS: [KnownItem; 14] = [
    KnownItem {
        id: "model-qwen3vl-2b-gguf",
        path: "models/qwen3-vl-2b-instruct.Q4_K_M.gguf",
        category: "ai-model",
        removable: true,
        label: "Qwen3-VL-2B-Instruct 模型",
    },
    KnownItem {
        id: "model-qwen3vl-2b-mmproj",
        path: "models/qwen3-vl-2b-instruct.mmproj.gguf",
        category: "ai-model",
        removable: true,
        label: "Qwen3-VL-2B mmproj",
    },
    KnownItem {
        id: "model-qwen3vl-4b-gguf",
        path: "models/qwen3-vl-4b-instruct.Q4_K_M.gguf",
        category: "ai-model",
        removable: true,
        label: "Qwen3-VL-4B-Instruct 模型",
    },
    KnownItem {
        id: "model-qwen3vl-4b-mmproj",
        path: "models/qwen3-vl-4b-instruct.mmproj.gguf",
        category: "ai-model",
        removable: true,
        label: "Qwen3-VL-4B mmproj",
    },
    KnownItem {
        id: "model-qwen3vl-8b-gguf",
        path: "models/qwen3-vl-8b-instruct.Q4_K_M.gguf",
        category: "ai-model",
        removable: true,
        label: "Qwen3-VL-8B-Instruct 模型",
    },
    KnownItem {
        id: "model-qwen3vl-8b-mmproj",
        path: "models/qwen3-vl-8b-instruct.mmproj.gguf",
        category: "ai-model",
        removable: true,
        label: "Qwen3-VL-8B mmproj",
    },
    KnownItem {
        id: "captures",
        path: "captures/",
        category: "user-data",
        removable: true,
        label: "OCR 擷取圖像與記錄",
    },
    KnownItem {
        id: "scenarios",
        path: "scenarios.json",
        category: "settings",
        removable: true,
        label: "情境設定",
    },
    KnownItem {
        id: "window-state",
        path: "window_state.json",
        category: "settings",
        removable: true,
        label: "視窗位置與狀態",
    },
    KnownItem {
        id: "output-lang",
        path: "output_lang.txt",
        category: "settings",
        removable: true,
        label: "輸出語言偏好",
    },
    KnownItem {
        id: "tts-config",
        path: "tts_config.json",
        category: "settings",
        removable: true,
        label: "Azure TTS 設定",
    },
    KnownItem {
        id: "tts-preview-cache",
        path: "tts_preview_cache/",
        category: "cache",
        removable: true,
        label: "TTS 預覽快取",
    },
    KnownItem {
        id: "tts-speak-cache",
        path: "tts_speak_cache/",
        category: "cache",
        removable: true,
        label: "TTS 朗讀快取",
    },
    KnownItem {
        id: "llama-bin",
        path: "bin/",
        category: "dependency",
        removable: false,
        label: "llama.cpp 執行檔（必要）",
    },
];

fn empty_inventory() -> Inventory {
    Inventory {
        version: "1.0".to_string(),
        items: Vec::new(),
    }
}

pub fn inventory_path() -> PathBuf {
    crate::app_paths::data_dir().join("inventory.json")
}

pub fn load() -> Inventory {
    load_from_path(&inventory_path())
}

fn load_from_path(path: &Path) -> Inventory {
    let Ok(raw) = fs::read_to_string(path) else {
        return empty_inventory();
    };
    serde_json::from_str(&raw).unwrap_or_else(|err| {
        eprintln!("[inventory] parse {} failed: {}", path.display(), err);
        empty_inventory()
    })
}

pub fn save(inv: &Inventory) {
    let final_path = inventory_path();
    save_to_path(inv, &final_path);
}

fn save_to_path(inv: &Inventory, final_path: &Path) {
    if let Some(parent) = final_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "[inventory] create dir {} failed: {}",
                parent.display(),
                err
            );
            return;
        }
    }

    let tmp_path = final_path.with_extension("json.tmp");
    let json = match serde_json::to_vec_pretty(inv) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("[inventory] serialize failed: {}", err);
            return;
        }
    };

    let mut file = match fs::File::create(&tmp_path) {
        Ok(f) => f,
        Err(err) => {
            eprintln!(
                "[inventory] create tmp {} failed: {}",
                tmp_path.display(),
                err
            );
            return;
        }
    };
    if let Err(err) = file.write_all(&json) {
        eprintln!(
            "[inventory] write tmp {} failed: {}",
            tmp_path.display(),
            err
        );
        return;
    }
    if let Err(err) = file.sync_all() {
        eprintln!(
            "[inventory] sync tmp {} failed: {}",
            tmp_path.display(),
            err
        );
        return;
    }
    drop(file);
    if let Err(err) = fs::rename(&tmp_path, final_path) {
        eprintln!(
            "[inventory] rename {} -> {} failed: {}",
            tmp_path.display(),
            final_path.display(),
            err
        );
        let _ = fs::remove_file(&tmp_path);
    }
}

pub fn upsert(item: InventoryItem) {
    let mut inv = load();
    upsert_in_memory(&mut inv, item);
    save(&inv);
}

#[allow(dead_code)]
pub fn remove(id: &str) {
    let mut inv = load();
    inv.items.retain(|i| i.id != id);
    save(&inv);
}

pub fn reconcile() {
    let inv = reconcile_at(&crate::app_paths::data_dir());
    save(&inv);
}

pub fn reconcile_one(id: &str) {
    let root = crate::app_paths::data_dir();
    let mut inv = load();
    inv.items.retain(|item| item.id != id);
    if let Some(known) = known_item(id) {
        let size = item_size(&root, known.path);
        if should_include(known, size) {
            inv.items.push(InventoryItem {
                id: known.id.to_string(),
                path: known.path.to_string(),
                size,
                category: known.category.to_string(),
                removable: known.removable,
                label: known.label.to_string(),
            });
        }
    }
    save(&inv);
}

fn known_item(id: &str) -> Option<KnownItem> {
    KNOWN_ITEMS.iter().copied().find(|item| item.id == id)
}

fn upsert_in_memory(inv: &mut Inventory, item: InventoryItem) {
    if let Some(existing) = inv.items.iter_mut().find(|i| i.id == item.id) {
        *existing = item;
    } else {
        inv.items.push(item);
    }
}

fn should_include(item: KnownItem, size: u64) -> bool {
    item.id == "captures" || item.id == "llama-bin" || size > 0
}

fn item_size(root: &Path, rel_path: &str) -> u64 {
    let is_dir = rel_path.ends_with('/');
    let full = root.join(rel_path.trim_end_matches('/'));
    if is_dir {
        dir_size(&full)
    } else {
        fs::metadata(&full).map(|m| m.len()).unwrap_or(0)
    }
}

fn dir_size(path: &Path) -> u64 {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return 0,
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let entry_path = entry.path();
        match entry.metadata() {
            Ok(meta) if meta.is_file() => total = total.saturating_add(meta.len()),
            Ok(meta) if meta.is_dir() => total = total.saturating_add(dir_size(&entry_path)),
            _ => {}
        }
    }
    total
}

fn reconcile_at(root: &Path) -> Inventory {
    let mut inv = empty_inventory();
    for known in KNOWN_ITEMS {
        let size = item_size(root, known.path);
        if should_include(known, size) {
            inv.items.push(InventoryItem {
                id: known.id.to_string(),
                path: known.path.to_string(),
                size,
                category: known.category.to_string(),
                removable: known.removable,
                label: known.label.to_string(),
            });
        }
    }
    inv
}

pub fn model_items_for_id(
    model_id: crate::llama_runtime::manifest::ModelId,
) -> Option<(InventoryItem, InventoryItem)> {
    match model_id {
        crate::llama_runtime::manifest::ModelId::Qwen3Vl2bInstruct => Some((
            known_to_item("model-qwen3vl-2b-gguf"),
            known_to_item("model-qwen3vl-2b-mmproj"),
        )),
        crate::llama_runtime::manifest::ModelId::Qwen3Vl4bInstruct => Some((
            known_to_item("model-qwen3vl-4b-gguf"),
            known_to_item("model-qwen3vl-4b-mmproj"),
        )),
        crate::llama_runtime::manifest::ModelId::Qwen3Vl8bInstruct => Some((
            known_to_item("model-qwen3vl-8b-gguf"),
            known_to_item("model-qwen3vl-8b-mmproj"),
        )),
    }
}

fn known_to_item(id: &str) -> InventoryItem {
    let known = known_item(id).expect("known id must exist");
    InventoryItem {
        id: known.id.to_string(),
        path: known.path.to_string(),
        size: item_size(&crate::app_paths::data_dir(), known.path),
        category: known.category.to_string(),
        removable: known.removable,
        label: known.label.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_new_item() {
        let mut inv = empty_inventory();
        upsert_in_memory(
            &mut inv,
            InventoryItem {
                id: "x".to_string(),
                path: "a".to_string(),
                size: 1,
                category: "settings".to_string(),
                removable: true,
                label: "A".to_string(),
            },
        );
        assert_eq!(inv.items.len(), 1);
    }

    #[test]
    fn test_upsert_existing() {
        let mut inv = empty_inventory();
        upsert_in_memory(
            &mut inv,
            InventoryItem {
                id: "x".to_string(),
                path: "a".to_string(),
                size: 1,
                category: "settings".to_string(),
                removable: true,
                label: "A".to_string(),
            },
        );
        upsert_in_memory(
            &mut inv,
            InventoryItem {
                id: "x".to_string(),
                path: "b".to_string(),
                size: 2,
                category: "cache".to_string(),
                removable: false,
                label: "B".to_string(),
            },
        );
        assert_eq!(inv.items.len(), 1);
        assert_eq!(inv.items[0].path, "b");
        assert_eq!(inv.items[0].size, 2);
    }

    #[test]
    fn test_remove() {
        let mut inv = empty_inventory();
        upsert_in_memory(
            &mut inv,
            InventoryItem {
                id: "x".to_string(),
                path: "a".to_string(),
                size: 1,
                category: "settings".to_string(),
                removable: true,
                label: "A".to_string(),
            },
        );
        inv.items.retain(|i| i.id != "x");
        assert!(inv.items.is_empty());
    }

    #[test]
    fn test_load_missing_file() {
        let loaded = load_from_path(Path::new("Z:/this/path/should/not/exist/inventory.json"));
        assert_eq!(loaded, empty_inventory());
    }

    #[test]
    fn test_serde_roundtrip() {
        let inv = Inventory {
            version: "1.0".to_string(),
            items: vec![InventoryItem {
                id: "x".to_string(),
                path: "captures/".to_string(),
                size: 9,
                category: "user-data".to_string(),
                removable: true,
                label: "測試".to_string(),
            }],
        };
        let raw = serde_json::to_string(&inv).expect("serialize");
        let parsed: Inventory = serde_json::from_str(&raw).expect("deserialize");
        assert_eq!(parsed, inv);
    }

    #[test]
    fn reconcile_with_root_non_existent_returns_without_panic() {
        let inv = reconcile_at(Path::new("Z:/this/path/should/not/exist/inventory-root"));
        assert_eq!(inv.version, "1.0");
    }
}
