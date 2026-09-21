mod store;

use std::path::PathBuf;
use std::sync::Mutex;

use store::{NoteDoc, NoteSummary, Store, TodoItem};
use tauri::{Manager, State};

struct AppState {
    store: Mutex<Store>,
}

fn with_store<T>(
    state: &AppState,
    action: impl FnOnce(&Store) -> Result<T, store::StoreError>,
) -> Result<T, String> {
    let store = state
        .store
        .lock()
        .map_err(|_| "应用内部锁定失败".to_string())?;
    action(&store).map_err(|error| error.to_string())
}

fn data_root() -> PathBuf {
    if let Ok(path) = std::env::var("MOYU_DATA") {
        return PathBuf::from(path);
    }
    if cfg!(debug_assertions) {
        return PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data");
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("data")))
        .unwrap_or_else(|| PathBuf::from("data"))
}

#[tauri::command]
fn load_day(state: State<AppState>, date: String) -> Result<Vec<TodoItem>, String> {
    with_store(&state, |store| store.load_day(&date))
}

#[tauri::command]
fn save_day(state: State<AppState>, date: String, items: Vec<TodoItem>) -> Result<(), String> {
    with_store(&state, |store| store.save_day(&date, &items))
}

#[tauri::command]
fn carry_unfinished(state: State<AppState>, today: String) -> Result<usize, String> {
    with_store(&state, |store| store.carry_unfinished(&today))
}

#[tauri::command]
fn list_categories(state: State<AppState>) -> Result<Vec<String>, String> {
    with_store(&state, |store| store.list_categories())
}

#[tauri::command]
fn list_notes(
    state: State<AppState>,
    category: Option<String>,
    query: Option<String>,
) -> Result<Vec<NoteSummary>, String> {
    with_store(&state, |store| {
        store.list_notes(category.as_deref(), query.as_deref())
    })
}

#[tauri::command]
fn read_note(
    state: State<AppState>,
    category: String,
    file_name: String,
) -> Result<NoteDoc, String> {
    with_store(&state, |store| store.read_note(&category, &file_name))
}

#[tauri::command]
fn save_note(
    state: State<AppState>,
    category: String,
    title: String,
    body: String,
    previous_file_name: Option<String>,
) -> Result<NoteDoc, String> {
    with_store(&state, |store| {
        store.save_note(&category, &title, &body, previous_file_name.as_deref())
    })
}

#[tauri::command]
fn delete_note(
    state: State<AppState>,
    category: String,
    file_name: String,
) -> Result<(), String> {
    with_store(&state, |store| store.delete_note(&category, &file_name))
}

#[tauri::command]
fn create_category(state: State<AppState>, name: String) -> Result<(), String> {
    with_store(&state, |store| store.create_category(&name))
}

#[tauri::command]
fn delete_category(state: State<AppState>, name: String) -> Result<(), String> {
    with_store(&state, |store| store.delete_category(&name))
}

#[tauri::command]
fn rename_category(state: State<AppState>, from: String, to: String) -> Result<(), String> {
    with_store(&state, |store| store.rename_category(&from, &to))
}

#[tauri::command]
fn reorder_categories(state: State<AppState>, names: Vec<String>) -> Result<(), String> {
    with_store(&state, |store| store.reorder_categories(&names))
}

#[tauri::command]
fn reorder_files(
    state: State<AppState>,
    category: String,
    names: Vec<String>,
) -> Result<(), String> {
    with_store(&state, |store| store.reorder_files(&category, &names))
}

#[tauri::command]
fn data_directory(state: State<AppState>) -> Result<String, String> {
    with_store(&state, |store| Ok(store.display_path()))
}

#[tauri::command]
fn import_file(
    state: State<AppState>,
    category: String,
    source_path: String,
) -> Result<NoteSummary, String> {
    with_store(&state, |store| {
        store.import_file(&category, PathBuf::from(source_path).as_path())
    })
}

#[tauri::command]
fn absolute_path(
    state: State<AppState>,
    category: String,
    file_name: String,
) -> Result<String, String> {
    with_store(&state, |store| store.absolute_path(&category, &file_name))
}

#[tauri::command]
fn open_stored_file(
    state: State<AppState>,
    category: String,
    file_name: String,
) -> Result<(), String> {
    with_store(&state, |store| store.open_file(&category, &file_name))
}

#[tauri::command]
fn reveal_stored_file(
    state: State<AppState>,
    category: String,
    file_name: String,
) -> Result<(), String> {
    with_store(&state, |store| store.reveal_file(&category, &file_name))
}

#[tauri::command]
fn open_data_directory(state: State<AppState>) -> Result<(), String> {
    with_store(&state, |store| store.open_root())
}

#[tauri::command]
fn open_category_directory(state: State<AppState>, category: String) -> Result<(), String> {
    with_store(&state, |store| store.open_category_directory(&category))
}

#[tauri::command]
fn write_export(
    state: State<AppState>,
    destination: String,
    content: String,
) -> Result<(), String> {
    with_store(&state, |store| {
        store.write_export(PathBuf::from(destination).as_path(), &content)
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let store = Store::open(data_root())?;
            app.manage(AppState {
                store: Mutex::new(store),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_day,
            save_day,
            carry_unfinished,
            list_categories,
            list_notes,
            read_note,
            save_note,
            delete_note,
            create_category,
            delete_category,
            rename_category,
            reorder_categories,
            reorder_files,
            data_directory,
            import_file,
            absolute_path,
            open_stored_file,
            reveal_stored_file,
            open_data_directory,
            open_category_directory,
            write_export
        ])
        .run(tauri::generate_context!())
        .expect("摸鱼大王启动失败");
}
