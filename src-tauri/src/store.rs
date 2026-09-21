use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_NAME_CHARS: usize = 60;
const MAX_TODO_CHARS: usize = 200;

#[derive(Debug)]
pub enum StoreError {
    Invalid(String),
    NotFound(String),
    Conflict(String),
    Io(io::Error),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Invalid(message)
            | StoreError::NotFound(message)
            | StoreError::Conflict(message) => write!(f, "{message}"),
            StoreError::Io(error) => write!(f, "读写失败：{error}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(value: io::Error) -> Self {
        StoreError::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    pub text: String,
    pub done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummary {
    pub category: String,
    pub title: String,
    pub file_name: String,
    pub extension: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteDoc {
    pub category: String,
    pub title: String,
    pub file_name: String,
    pub extension: String,
    pub kind: String,
    pub body: String,
    pub size_label: String,
}

pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let store = Self { root: root.into() };
        store.ensure_layout()?;
        Ok(store)
    }

    #[cfg(test)]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn display_path(&self) -> String {
        let path = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        plain_path(&path)
    }

    pub fn ensure_layout(&self) -> Result<(), StoreError> {
        fs::create_dir_all(self.root.join("todos"))?;
        fs::create_dir_all(self.root.join(".meta"))?;
        fs::create_dir_all(self.root.join("notes"))?;
        self.write_index()?;
        Ok(())
    }

    pub fn load_day(&self, date: &str) -> Result<Vec<TodoItem>, StoreError> {
        let date = validate_date(date)?;
        parse_todos(&read_to_string(&self.root.join("todos").join(format!("{date}.md")))?)
    }

    pub fn save_day(&self, date: &str, items: &[TodoItem]) -> Result<(), StoreError> {
        let date = validate_date(date)?;
        let items = sink_done(clean_todos(items)?);
        let path = self.root.join("todos").join(format!("{date}.md"));
        write_todo_file(&path, &date, &items)?;
        self.write_index()?;
        Ok(())
    }

    pub fn carry_unfinished(&self, today: &str) -> Result<usize, StoreError> {
        let today = validate_date(today)?;
        let todos = self.root.join("todos");
        let mut dates = Vec::new();
        if todos.is_dir() {
            for entry in fs::read_dir(&todos)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                let Some(date) = name.strip_suffix(".md") else {
                    continue;
                };
                if validate_date(date).is_ok() && date < today.as_str() {
                    dates.push(date.to_string());
                }
            }
        }
        dates.sort();
        let mut moved = Vec::new();
        for date in dates {
            let items = self.load_day(&date)?;
            let mut done = Vec::new();
            let mut open = Vec::new();
            for item in items {
                if item.done {
                    done.push(item);
                } else {
                    open.push(item);
                }
            }
            if open.is_empty() {
                continue;
            }
            moved.extend(open);
            self.save_day(&date, &done)?;
        }
        let count = moved.len();
        if count > 0 {
            let mut today_items = self.load_day(&today)?;
            moved.append(&mut today_items);
            self.save_day(&today, &moved)?;
        }
        Ok(count)
    }

    pub fn list_categories(&self) -> Result<Vec<String>, StoreError> {
        let found = self.raw_categories()?;
        let order = self.read_order()?;
        Ok(arrange(&order.categories, found))
    }

    fn raw_categories(&self) -> Result<Vec<String>, StoreError> {
        let mut found = Vec::new();
        for entry in fs::read_dir(self.root.join("notes"))? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            found.push(name);
        }
        Ok(found)
    }

    pub fn list_notes(
        &self,
        category: Option<&str>,
        query: Option<&str>,
    ) -> Result<Vec<NoteSummary>, StoreError> {
        let query = query.unwrap_or("").trim().to_lowercase();
        let categories = match category.map(str::trim).filter(|value| !value.is_empty()) {
            Some(name) => {
                let name = validate_name(name)?;
                let dir = self.root.join("notes").join(&name);
                if !dir.is_dir() {
                    return Err(StoreError::NotFound("分类不存在".into()));
                }
                vec![name]
            }
            None => self.list_categories()?,
        };

        let order = self.read_order()?;
        let mut notes = Vec::new();
        for category_name in categories {
            let saved = order
                .files
                .get(&category_name)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for summary in self.notes_in_category(&category_name, saved)? {
                if !query.is_empty()
                    && !summary.title.to_lowercase().contains(&query)
                    && !summary.file_name.to_lowercase().contains(&query)
                {
                    continue;
                }
                notes.push(summary);
            }
        }
        Ok(notes)
    }

    fn notes_in_category(
        &self,
        category_name: &str,
        saved: &[String],
    ) -> Result<Vec<NoteSummary>, StoreError> {
        let mut by_name = HashMap::new();
        for entry in fs::read_dir(self.root.join("notes").join(category_name))? {
            let path = entry?.path();
            let Some(summary) = summarize_entry(category_name, &path)? else {
                continue;
            };
            by_name.insert(summary.file_name.clone(), summary);
        }
        let present: Vec<_> = by_name.keys().cloned().collect();
        let mut notes = Vec::new();
        for name in arrange(saved, present) {
            if let Some(summary) = by_name.remove(&name) {
                notes.push(summary);
            }
        }
        Ok(notes)
    }

    pub fn read_note(&self, category: &str, file_name: &str) -> Result<NoteDoc, StoreError> {
        let category = validate_name(category)?;
        let file_name = validate_file_name(file_name)?;
        let path = self.entry_path(&category, &file_name);
        let Some(summary) = summarize_entry(&category, &path)? else {
            return Err(StoreError::NotFound("资料不存在".into()));
        };
        if !path.is_file() {
            return Err(StoreError::NotFound("资料不存在".into()));
        }
        let body = if summary.kind == "text" {
            let raw = read_to_string(&path)?;
            if summary.extension == "md" {
                decode_note(&summary.title, &raw)
            } else {
                raw.trim_end_matches('\n').to_string()
            }
        } else {
            String::new()
        };
        Ok(NoteDoc {
            category: summary.category,
            title: summary.title,
            file_name: summary.file_name,
            extension: summary.extension,
            kind: summary.kind,
            body,
            size_label: format_size(fs::metadata(&path)?.len()),
        })
    }

    pub fn save_note(
        &self,
        category: &str,
        title: &str,
        body: &str,
        previous_file_name: Option<&str>,
    ) -> Result<NoteDoc, StoreError> {
        let category = validate_name(category)?;
        let title = validate_name(title)?;
        let dir = self.root.join("notes").join(&category);
        if !dir.is_dir() {
            return Err(StoreError::NotFound("分类不存在".into()));
        }

        let mut extension = "md".to_string();
        let mut renamed_from = None;
        if let Some(previous) = previous_file_name.map(str::trim).filter(|value| !value.is_empty()) {
            let previous = validate_file_name(previous)?;
            renamed_from = Some(previous.clone());
            let source = self.entry_path(&category, &previous);
            let Some(summary) = summarize_entry(&category, &source)? else {
                return Err(StoreError::NotFound("资料不存在".into()));
            };
            if summary.kind != "text" {
                return Err(StoreError::Invalid("非文本文件请用系统程序打开编辑".into()));
            }
            extension = summary.extension;
            let destination_name = format!("{title}.{extension}");
            let destination = self.entry_path(&category, &destination_name);
            if previous != destination_name {
                if destination.exists() {
                    return Err(StoreError::Conflict("已有同名资料".into()));
                }
                if source.is_file() {
                    fs::rename(&source, &destination)?;
                }
            }
        } else {
            let destination = self.entry_path(&category, &format!("{title}.md"));
            if destination.exists() {
                return Err(StoreError::Conflict("已有同名资料".into()));
            }
        }

        let file_name = format!("{title}.{extension}");
        let destination = self.entry_path(&category, &file_name);
        let encoded = if extension == "md" {
            encode_note(&title, body)
        } else {
            let body = normalize_newlines(body);
            if body.ends_with('\n') {
                body
            } else if body.is_empty() {
                String::new()
            } else {
                format!("{body}\n")
            }
        };
        let body = if extension == "md" {
            decode_note(&title, &encoded)
        } else {
            encoded.trim_end_matches('\n').to_string()
        };
        write_atomic(&destination, &encoded)?;
        if let Some(previous) = renamed_from {
            if previous != file_name {
                self.retitle_file_order(&category, &previous, &file_name)?;
            }
        }
        self.sync_file_order(&category)?;
        self.write_index()?;
        Ok(NoteDoc {
            category,
            title,
            file_name: file_name.clone(),
            extension,
            kind: "text".into(),
            body,
            size_label: format_size(fs::metadata(&destination)?.len()),
        })
    }

    pub fn delete_note(&self, category: &str, file_name: &str) -> Result<(), StoreError> {
        let category = validate_name(category)?;
        let file_name = validate_file_name(file_name)?;
        let path = self.entry_path(&category, &file_name);
        if path.is_file() {
            fs::remove_file(path)?;
        }
        self.sync_file_order(&category)?;
        self.write_index()?;
        Ok(())
    }

    pub fn import_file(&self, category: &str, source: &Path) -> Result<NoteSummary, StoreError> {
        let category = validate_name(category)?;
        let dir = self.root.join("notes").join(&category);
        if !dir.is_dir() {
            return Err(StoreError::NotFound("分类不存在".into()));
        }
        if !source.is_file() {
            return Err(StoreError::NotFound("找不到要导入的文件".into()));
        }
        let source_name = source
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !is_listed_file_name(source_name) {
            return Err(StoreError::Invalid("这类文件不会在资料库中展示".into()));
        }
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();
        let stem = source
            .file_stem()
            .and_then(|value| value.to_str())
            .or_else(|| source.file_name().and_then(|value| value.to_str()))
            .unwrap_or("未命名");
        let stem = validate_name(stem)?;
        let mut file_name = if extension.is_empty() {
            stem.clone()
        } else {
            format!("{stem}.{extension}")
        };
        let mut destination = self.entry_path(&category, &file_name);
        if destination.exists() {
            let mut index = 2;
            loop {
                file_name = if extension.is_empty() {
                    format!("{stem} {index}")
                } else {
                    format!("{stem} {index}.{extension}")
                };
                destination = self.entry_path(&category, &file_name);
                if !destination.exists() {
                    break;
                }
                index += 1;
                if index > 200 {
                    return Err(StoreError::Conflict("同名文件太多了".into()));
                }
            }
        }
        fs::copy(source, &destination)?;
        self.sync_file_order(&category)?;
        self.write_index()?;
        summarize_entry(&category, &destination)?
            .ok_or_else(|| StoreError::Invalid("导入后无法识别文件".into()))
    }

    pub fn absolute_path(&self, category: &str, file_name: &str) -> Result<String, StoreError> {
        let category = validate_name(category)?;
        let file_name = validate_file_name(file_name)?;
        let path = self.entry_path(&category, &file_name);
        if !path.is_file() {
            return Err(StoreError::NotFound("资料不存在".into()));
        }
        Ok(plain_path(&path.canonicalize().unwrap_or(path)))
    }

    pub fn open_file(&self, category: &str, file_name: &str) -> Result<(), StoreError> {
        let path = self.absolute_path(category, file_name)?;
        open::that(&path)?;
        Ok(())
    }

    pub fn reveal_file(&self, category: &str, file_name: &str) -> Result<(), StoreError> {
        let path = self.absolute_path(category, file_name)?;
        reveal_path(Path::new(&path))
    }

    pub fn open_root(&self) -> Result<(), StoreError> {
        open::that(self.display_path())?;
        Ok(())
    }

    pub fn open_category_directory(&self, category: &str) -> Result<(), StoreError> {
        let category = validate_name(category)?;
        let path = self.root.join("notes").join(&category);
        if !path.is_dir() {
            return Err(StoreError::NotFound("分类不存在".into()));
        }
        open::that(plain_path(&path.canonicalize().unwrap_or(path)))?;
        Ok(())
    }

    pub fn write_export(&self, destination: &Path, content: &str) -> Result<(), StoreError> {
        if content.len() > 2_000_000 {
            return Err(StoreError::Invalid("导出内容过长".into()));
        }
        let name = destination
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if name == ".md" || !name.to_ascii_lowercase().ends_with(".md") {
            return Err(StoreError::Invalid("请保存为 .md 文件".into()));
        }
        let root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        if let Some(parent) = destination.parent() {
            if let Ok(parent) = parent.canonicalize() {
                if parent.starts_with(&root) {
                    return Err(StoreError::Invalid("请保存到数据目录以外".into()));
                }
            }
        }
        write_atomic(destination, content)
    }

    pub fn create_category(&self, name: &str) -> Result<(), StoreError> {
        let name = validate_name(name)?;
        let path = self.root.join("notes").join(&name);
        if path.exists() {
            return Err(StoreError::Conflict("已有同名分类".into()));
        }
        fs::create_dir(path)?;
        self.sync_category_order()?;
        self.write_index()?;
        Ok(())
    }

    pub fn delete_category(&self, name: &str) -> Result<(), StoreError> {
        let name = validate_name(name)?;
        let path = self.root.join("notes").join(&name);
        if !path.is_dir() {
            return Err(StoreError::NotFound("分类不存在".into()));
        }
        fs::remove_dir_all(path)?;
        self.sync_category_order()?;
        self.write_index()?;
        Ok(())
    }

    pub fn rename_category(&self, from: &str, to: &str) -> Result<(), StoreError> {
        let from = validate_name(from)?;
        let to = validate_name(to)?;
        if from == to {
            return Ok(());
        }
        let source = self.root.join("notes").join(&from);
        if !source.is_dir() {
            return Err(StoreError::NotFound("分类不存在".into()));
        }
        let destination = self.root.join("notes").join(&to);
        if destination.exists() {
            return Err(StoreError::Conflict("已有同名分类".into()));
        }
        fs::rename(source, destination)?;
        self.retitle_category_order(&from, &to)?;
        self.sync_category_order()?;
        self.write_index()?;
        Ok(())
    }

    fn entry_path(&self, category: &str, file_name: &str) -> PathBuf {
        self.root.join("notes").join(category).join(file_name)
    }

    pub fn reorder_categories(&self, names: &[String]) -> Result<(), StoreError> {
        let present = self.raw_categories()?;
        let mut order = self.read_order()?;
        order.categories = arrange(names, present);
        self.write_order(&order)?;
        self.write_index()?;
        Ok(())
    }

    pub fn reorder_files(&self, category: &str, names: &[String]) -> Result<(), StoreError> {
        let category = validate_name(category)?;
        if !self.root.join("notes").join(&category).is_dir() {
            return Err(StoreError::NotFound("分类不存在".into()));
        }
        let present = self.raw_file_names(&category)?;
        let mut order = self.read_order()?;
        order.files.insert(category, arrange(names, present));
        self.write_order(&order)?;
        self.write_index()?;
        Ok(())
    }

    fn raw_file_names(&self, category: &str) -> Result<Vec<String>, StoreError> {
        let mut names = Vec::new();
        for entry in fs::read_dir(self.root.join("notes").join(category))? {
            let path = entry?.path();
            let Some(summary) = summarize_entry(category, &path)? else {
                continue;
            };
            names.push(summary.file_name);
        }
        Ok(names)
    }

    fn sync_category_order(&self) -> Result<(), StoreError> {
        let mut order = self.read_order()?;
        order.categories = arrange(&order.categories, self.raw_categories()?);
        let present: HashSet<_> = order.categories.iter().cloned().collect();
        order.files.retain(|name, _| present.contains(name));
        self.write_order(&order)
    }

    fn sync_file_order(&self, category: &str) -> Result<(), StoreError> {
        let mut order = self.read_order()?;
        let saved = order.files.get(category).cloned().unwrap_or_default();
        order
            .files
            .insert(category.to_string(), arrange(&saved, self.raw_file_names(category)?));
        self.write_order(&order)
    }

    fn retitle_category_order(&self, from: &str, to: &str) -> Result<(), StoreError> {
        let mut order = self.read_order()?;
        for name in &mut order.categories {
            if name == from {
                *name = to.to_string();
            }
        }
        if let Some(files) = order.files.remove(from) {
            order.files.insert(to.to_string(), files);
        }
        self.write_order(&order)
    }

    fn retitle_file_order(&self, category: &str, from: &str, to: &str) -> Result<(), StoreError> {
        let mut order = self.read_order()?;
        if let Some(files) = order.files.get_mut(category) {
            for name in files {
                if name == from {
                    *name = to.to_string();
                }
            }
        }
        self.write_order(&order)
    }

    fn read_order(&self) -> Result<LibraryOrder, StoreError> {
        Ok(parse_order(&read_to_string(&self.order_path())?))
    }

    fn write_order(&self, order: &LibraryOrder) -> Result<(), StoreError> {
        write_atomic(&self.order_path(), &render_order(order))
    }

    fn order_path(&self) -> PathBuf {
        self.root.join(".meta").join("order.md")
    }

    fn write_index(&self) -> Result<(), StoreError> {
        let mut lines = vec![
            "# 资料目录".to_string(),
            String::new(),
            "本文件由摸鱼大王自动更新。".to_string(),
            String::new(),
            "## 待办".to_string(),
            String::new(),
        ];

        let todos = self.root.join("todos");
        if todos.is_dir() {
            let mut files: Vec<_> = fs::read_dir(&todos)?.collect::<Result<Vec<_>, _>>()?;
            files.sort_by_key(|entry| entry.file_name());
            let mut any = false;
            for entry in files {
                let name = entry.file_name().to_string_lossy().to_string();
                if !name.ends_with(".md") || name == "later.md" {
                    continue;
                }
                any = true;
                let label = name.trim_end_matches(".md").to_string();
                lines.push(format!("- [{label}](<../todos/{name}>)"));
            }
            if !any {
                lines.push("- （还没有待办文件）".to_string());
            }
        }

        lines.push(String::new());
        lines.push("## 资料".to_string());
        for category in self.list_categories()? {
            lines.push(String::new());
            lines.push(format!("### {category}"));
            lines.push(String::new());
            let notes = self.list_notes(Some(&category), None)?;
            if notes.is_empty() {
                lines.push("- （空）".to_string());
                continue;
            }
            for note in notes {
                lines.push(format!(
                    "- [{title}](<../notes/{category}/{file}>)",
                    title = note.title,
                    category = note.category,
                    file = note.file_name
                ));
            }
        }
        lines.push(String::new());
        write_atomic(&self.root.join(".meta").join("index.md"), &lines.join("\n"))?;
        Ok(())
    }
}

fn plain_path(path: &Path) -> String {
    path.display()
        .to_string()
        .trim_start_matches(r"\\?\")
        .to_string()
}

#[derive(Default)]
struct LibraryOrder {
    categories: Vec<String>,
    files: HashMap<String, Vec<String>>,
}

fn arrange(saved: &[String], present: Vec<String>) -> Vec<String> {
    let present_set: HashSet<&str> = present.iter().map(String::as_str).collect();
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    for name in saved {
        if present_set.contains(name.as_str()) && seen.insert(name.clone()) {
            ordered.push(name.clone());
        }
    }
    let mut rest = present;
    rest.sort();
    for name in rest {
        if seen.insert(name.clone()) {
            ordered.push(name);
        }
    }
    ordered
}

fn parse_order(raw: &str) -> LibraryOrder {
    let mut order = LibraryOrder::default();
    let mut section = "";
    let mut current = String::new();
    for line in normalize_newlines(raw).lines() {
        let line = line.trim_end();
        if line == "## 分类" {
            section = "categories";
            current.clear();
            continue;
        }
        if line == "## 文件" {
            section = "files";
            current.clear();
            continue;
        }
        if let Some(name) = line.strip_prefix("### ") {
            if section == "files" {
                current = name.trim().to_string();
                if !current.is_empty() {
                    order.files.entry(current.clone()).or_default();
                }
            }
            continue;
        }
        let Some(item) = line.strip_prefix("- ") else {
            continue;
        };
        if item.is_empty() {
            continue;
        }
        match section {
            "categories" => order.categories.push(item.to_string()),
            "files" if !current.is_empty() => {
                order
                    .files
                    .entry(current.clone())
                    .or_default()
                    .push(item.to_string());
            }
            _ => {}
        }
    }
    order
}

fn render_order(order: &LibraryOrder) -> String {
    let mut lines = vec![
        "# 排序".to_string(),
        String::new(),
        "本文件由摸鱼大王自动更新。".to_string(),
        String::new(),
        "## 分类".to_string(),
        String::new(),
    ];
    for name in &order.categories {
        lines.push(format!("- {name}"));
    }
    lines.push(String::new());
    lines.push("## 文件".to_string());
    let mut seen = HashSet::new();
    for category in &order.categories {
        if let Some(files) = order.files.get(category) {
            push_file_section(&mut lines, category, files);
            seen.insert(category.clone());
        }
    }
    let mut rest: Vec<_> = order
        .files
        .keys()
        .filter(|name| !seen.contains(*name))
        .cloned()
        .collect();
    rest.sort();
    for category in rest {
        if let Some(files) = order.files.get(&category) {
            push_file_section(&mut lines, &category, files);
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn push_file_section(lines: &mut Vec<String>, category: &str, files: &[String]) {
    lines.push(String::new());
    lines.push(format!("### {category}"));
    lines.push(String::new());
    for file in files {
        lines.push(format!("- {file}"));
    }
}

fn summarize_entry(category: &str, path: &Path) -> Result<Option<NoteSummary>, StoreError> {
    if !path.is_file() {
        return Ok(None);
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    if !is_listed_file_name(&file_name) {
        return Ok(None);
    }
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    let title = path
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(file_name.as_str())
        .to_string();
    if title.is_empty() {
        return Ok(None);
    }
    Ok(Some(NoteSummary {
        category: category.to_string(),
        title,
        file_name,
        extension: extension.clone(),
        kind: if is_text_extension(&extension) {
            "text".into()
        } else {
            "file".into()
        },
    }))
}

fn is_listed_file_name(file_name: &str) -> bool {
    if file_name.is_empty() || file_name.starts_with('.') || file_name.starts_with("~$") {
        return false;
    }
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".tmp")
        || lower.ends_with(".temp")
        || lower.ends_with(".swp")
        || lower.ends_with('~')
    {
        return false;
    }
    true
}

fn is_text_extension(ext: &str) -> bool {
    matches!(ext, "md" | "txt")
}

fn sink_done(items: Vec<TodoItem>) -> Vec<TodoItem> {
    let mut open = Vec::new();
    let mut done = Vec::new();
    for item in items {
        if item.done {
            done.push(item);
        } else {
            open.push(item);
        }
    }
    open.extend(done);
    open
}

fn format_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if (bytes as f64) < MB {
        format!("{:.1} KB", bytes as f64 / KB)
    } else {
        format!("{:.1} MB", bytes as f64 / MB)
    }
}

fn validate_file_name(value: &str) -> Result<String, StoreError> {
    let name = value.trim();
    if name.is_empty() || name == "." || name == ".." {
        return Err(StoreError::Invalid("文件名不能为空".into()));
    }
    if name.chars().any(|ch| {
        ch.is_control()
            || matches!(
                ch,
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
            )
    }) {
        return Err(StoreError::Invalid("文件名不合法".into()));
    }
    let path = Path::new(name);
    if path.components().count() != 1 {
        return Err(StoreError::Invalid("文件名不合法".into()));
    }
    if !is_listed_file_name(name) {
        return Err(StoreError::Invalid("文件名不合法".into()));
    }
    Ok(name.to_string())
}

fn write_todo_file(path: &Path, heading: &str, items: &[TodoItem]) -> Result<(), StoreError> {
    if items.is_empty() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }
    let mut out = format!("# {heading}\n\n");
    for item in items {
        let mark = if item.done { "x" } else { " " };
        out.push_str(&format!("- [{mark}] {}\n", item.text));
    }
    write_atomic(path, &out)
}

fn parse_todos(raw: &str) -> Result<Vec<TodoItem>, StoreError> {
    let mut items = Vec::new();
    for line in normalize_newlines(raw).lines() {
        let line = line.trim();
        let (done, rest) = if let Some(rest) = line.strip_prefix("- [x]") {
            (true, rest)
        } else if let Some(rest) = line.strip_prefix("- [X]") {
            (true, rest)
        } else if let Some(rest) = line.strip_prefix("- [ ]") {
            (false, rest)
        } else {
            continue;
        };
        let text = rest.trim();
        if text.is_empty() {
            continue;
        }
        items.push(TodoItem {
            text: text.to_string(),
            done,
        });
    }
    Ok(items)
}

fn clean_todos(items: &[TodoItem]) -> Result<Vec<TodoItem>, StoreError> {
    let mut cleaned = Vec::new();
    for item in items {
        let text = item.text.replace(['\r', '\n'], " ");
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        if text.chars().count() > MAX_TODO_CHARS {
            return Err(StoreError::Invalid("待办太长了".into()));
        }
        cleaned.push(TodoItem {
            text: text.to_string(),
            done: item.done,
        });
    }
    Ok(cleaned)
}

fn encode_note(title: &str, body: &str) -> String {
    let body = normalize_newlines(body);
    let body = body.trim_end_matches('\n');
    if body.is_empty() {
        format!("# {title}\n")
    } else {
        format!("# {title}\n\n{body}\n")
    }
}

fn decode_note(title: &str, raw: &str) -> String {
    let text = normalize_newlines(raw);
    let with_blank = format!("# {title}\n\n");
    let body = if let Some(rest) = text.strip_prefix(&with_blank) {
        rest
    } else if let Some(rest) = text.strip_prefix(&format!("# {title}\n")) {
        rest.trim_start_matches('\n')
    } else {
        text.as_str()
    };
    body.trim_end_matches('\n').to_string()
}

fn normalize_newlines(value: &str) -> String {
    value
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

fn validate_date(value: &str) -> Result<String, StoreError> {
    let invalid = || StoreError::Invalid("日期不正确".into());
    if value.len() != 10 {
        return Err(invalid());
    }
    let bytes = value.as_bytes();
    if bytes[4] != b'-' || bytes[7] != b'-' || !bytes.iter().enumerate().all(|(index, byte)| {
        index == 4 || index == 7 || byte.is_ascii_digit()
    }) {
        return Err(invalid());
    }
    let year: i32 = value[0..4].parse().map_err(|_| invalid())?;
    let month: u32 = value[5..7].parse().map_err(|_| invalid())?;
    let day: u32 = value[8..10].parse().map_err(|_| invalid())?;
    if !(1..=12).contains(&month) {
        return Err(invalid());
    }
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => return Err(invalid()),
    };
    if !(1..=max_day).contains(&day) {
        return Err(invalid());
    }
    Ok(value.to_string())
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn validate_name(value: &str) -> Result<String, StoreError> {
    let name = value.trim();
    if name.is_empty() || name == "." || name == ".." {
        return Err(StoreError::Invalid("名称不能为空".into()));
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(StoreError::Invalid("名称过长".into()));
    }
    if name.chars().any(|ch| {
        ch.is_control()
            || matches!(
                ch,
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '[' | ']'
            )
    }) {
        return Err(StoreError::Invalid(
            "名称不能包含 \\ / : * ? \" < > | [ ]".into(),
        ));
    }
    if name.ends_with('.') || name.ends_with(' ') {
        return Err(StoreError::Invalid("名称不能以空格或点结尾".into()));
    }
    let upper = name.to_ascii_uppercase();
    let stem = upper.split('.').next().unwrap_or(&upper);
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem) {
        return Err(StoreError::Invalid("这个名称不能使用".into()));
    }
    Ok(name.to_string())
}

fn reveal_path(path: &Path) -> Result<(), StoreError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let arg = format!("/select,\"{}\"", path.display());
        std::process::Command::new("explorer").raw_arg(arg).spawn()?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let parent = path.parent().unwrap_or(path);
        open::that(parent)?;
        Ok(())
    }
}

fn read_to_string(path: &Path) -> Result<String, StoreError> {
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(normalize_newlines(&fs::read_to_string(path)?))
}

fn write_atomic(path: &Path, content: &str) -> Result<(), StoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, content.as_bytes())?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            let n = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!("moyu-{}-{}-{}", std::process::id(), n, nanos));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn store(&self) -> Store {
            Store::open(&self.0).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn first_open_creates_no_categories_and_does_not_restore_deleted_ones() {
        let scratch = Scratch::new();
        let store = scratch.store();
        assert!(store.list_categories().unwrap().is_empty());
        store.create_category("指令").unwrap();
        store.delete_category("指令").unwrap();
        let reopened = Store::open(&scratch.0).unwrap();
        assert!(reopened.list_categories().unwrap().is_empty());
    }

    #[test]
    fn unfinished_items_roll_forward_and_finished_stay() {
        let scratch = Scratch::new();
        let store = scratch.store();
        store
            .save_day(
                "2026-09-17",
                &[
                    TodoItem {
                        text: "旧任务".into(),
                        done: false,
                    },
                    TodoItem {
                        text: "已做完".into(),
                        done: true,
                    },
                ],
            )
            .unwrap();
        store
            .save_day(
                "2026-09-18",
                &[TodoItem {
                    text: "昨天没做".into(),
                    done: false,
                }],
            )
            .unwrap();
        store
            .save_day(
                "2026-09-19",
                &[TodoItem {
                    text: "今天的".into(),
                    done: false,
                }],
            )
            .unwrap();
        store
            .save_day(
                "2026-09-20",
                &[TodoItem {
                    text: "明天的".into(),
                    done: false,
                }],
            )
            .unwrap();
        assert_eq!(store.carry_unfinished("2026-09-19").unwrap(), 2);
        assert_eq!(
            store.load_day("2026-09-17").unwrap(),
            vec![TodoItem {
                text: "已做完".into(),
                done: true,
            }]
        );
        assert!(store.load_day("2026-09-18").unwrap().is_empty());
        assert_eq!(
            store
                .load_day("2026-09-19")
                .unwrap()
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            vec!["旧任务", "昨天没做", "今天的"]
        );
        assert_eq!(store.load_day("2026-09-20").unwrap()[0].text, "明天的");
        assert_eq!(store.carry_unfinished("2026-09-19").unwrap(), 0);
    }

    #[test]
    fn todos_roundtrip_and_reject_bad_dates() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        assert!(store.load_day("2026-09-19").unwrap().is_empty());
        store
            .save_day(
                "2026-09-19",
                &[
                    TodoItem {
                        text: "  写周报  ".into(),
                        done: false,
                    },
                    TodoItem {
                        text: "更新部署说明".into(),
                        done: true,
                    },
                    TodoItem {
                        text: "   ".into(),
                        done: false,
                    },
                ],
            )
            .unwrap();
        assert_eq!(
            store.load_day("2026-09-19").unwrap(),
            vec![
                TodoItem {
                    text: "写周报".into(),
                    done: false,
                },
                TodoItem {
                    text: "更新部署说明".into(),
                    done: true,
                },
            ]
        );
        let raw = fs::read_to_string(store.root().join("todos").join("2026-09-19.md")).unwrap();
        assert!(raw.contains("- [ ] 写周报"));
        assert!(raw.contains("- [x] 更新部署说明"));

        store
            .save_day(
                "2026-09-19",
                &[
                    TodoItem {
                        text: "已做完".into(),
                        done: true,
                    },
                    TodoItem {
                        text: "还没做".into(),
                        done: false,
                    },
                ],
            )
            .unwrap();
        assert_eq!(
            store
                .load_day("2026-09-19")
                .unwrap()
                .iter()
                .map(|item| item.text.as_str())
                .collect::<Vec<_>>(),
            vec!["还没做", "已做完"]
        );

        assert!(store.load_day("2026-02-29").is_err());
        assert!(store.load_day("2024-02-29").unwrap().is_empty());
        assert!(store.load_day("../2026-09-19").is_err());
        assert!(store.load_day("2026-13-01").is_err());
        assert!(store.save_day("not-a-date", &[]).is_err());

        store.save_day("2026-09-19", &[]).unwrap();
        assert!(!store.root().join("todos").join("2026-09-19.md").exists());
    }

    #[test]
    fn handwritten_checkbox_file_is_readable() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        fs::write(
            store.root().join("todos").join("2026-09-18.md"),
            "# 2026-09-18\r\n\r\n- [x] 已完成\r\n说明文字会被忽略\r\n- [ ] 未完成\r\n",
        )
        .unwrap();
        assert_eq!(
            store.load_day("2026-09-18").unwrap(),
            vec![
                TodoItem {
                    text: "已完成".into(),
                    done: true,
                },
                TodoItem {
                    text: "未完成".into(),
                    done: false,
                },
            ]
        );
    }

    #[test]
    fn note_roundtrip_rename_conflict_and_search() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        store.create_category("部署").unwrap();
        store.create_category("配置").unwrap();
        let saved = store
            .save_note("部署", "测试服务器", "Host: 10.0.0.8\n仅内网", None)
            .unwrap();
        assert_eq!(saved.title, "测试服务器");
        assert_eq!(saved.file_name, "测试服务器.md");
        let loaded = store.read_note("部署", "测试服务器.md").unwrap();
        assert_eq!(loaded.body, "Host: 10.0.0.8\n仅内网");
        let raw = fs::read_to_string(store.root().join("notes").join("部署").join("测试服务器.md")).unwrap();
        assert!(raw.starts_with("# 测试服务器\n"));
        assert!(!raw.contains("tags:"));

        let renamed = store
            .save_note(
                "部署",
                "预发服务器",
                "Host: 10.0.0.9",
                Some("测试服务器.md"),
            )
            .unwrap();
        assert_eq!(renamed.title, "预发服务器");
        assert!(store.read_note("部署", "测试服务器.md").is_err());
        assert_eq!(
            store.read_note("部署", "预发服务器.md").unwrap().body,
            "Host: 10.0.0.9"
        );

        store.save_note("配置", "预发服务器", "别的内容", None).unwrap();
        let conflict = store.save_note("部署", "预发服务器", "覆盖", None);
        assert!(matches!(conflict, Err(StoreError::Conflict(_))));

        let hits = store.list_notes(None, Some("预发")).unwrap();
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().any(|item| item.category == "部署"));
        assert!(store.list_notes(None, Some("10.0.0.9")).unwrap().is_empty());
        assert!(store.list_notes(Some("配置"), Some("不存在")).unwrap().is_empty());
    }

    #[test]
    fn path_traversal_is_rejected() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        assert!(store.save_note("../todos", "x", "secret", None).is_err());
        assert!(store.save_note("部署", "..\\..\\x", "secret", None).is_err());
        assert!(store.read_note("部署", "../../Cargo.toml").is_err());
        assert!(store.create_category("配置/逃逸").is_err());
        assert!(store.create_category("CON").is_err());
        assert!(store.create_category("com1").is_err());
        assert!(!store.root().join("secret").exists());
        assert!(!store.root().join("x.md").exists());
    }

    #[test]
    fn index_tracks_notes_and_todos_without_agent_instructions() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        store
            .save_day(
                "2026-09-19",
                &[TodoItem {
                    text: "写周报".into(),
                    done: false,
                }],
            )
            .unwrap();
        store.create_category("配置").unwrap();
        store.save_note("配置", "数据库", "端口 5432", None).unwrap();
        let index = fs::read_to_string(store.root().join(".meta").join("index.md")).unwrap();
        assert!(index.contains("# 资料目录"));
        assert!(index.contains("../todos/2026-09-19.md"));
        assert!(index.contains("../notes/配置/数据库.md"));
        assert!(!index.contains("AGENTS"));
        assert!(!index.contains("frontmatter"));

        store.delete_note("配置", "数据库.md").unwrap();
        let index = fs::read_to_string(store.root().join(".meta").join("index.md")).unwrap();
        assert!(!index.contains("数据库.md"));
    }

    #[test]
    fn empty_note_body_and_custom_category_order() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        store.create_category("临时").unwrap();
        let categories = store.list_categories().unwrap();
        assert_eq!(categories, vec!["临时"]);
        store.save_note("临时", "空笔记", "", None).unwrap();
        assert_eq!(store.read_note("临时", "空笔记.md").unwrap().body, "");
        let too_long = "名".repeat(61);
        assert!(store.create_category(&too_long).is_err());
        store.rename_category("临时", "归档").unwrap();
        assert_eq!(store.read_note("归档", "空笔记.md").unwrap().body, "");
        store.rename_category("归档", "归档").unwrap();
        store.create_category("别的").unwrap();
        assert!(matches!(
            store.rename_category("归档", "别的"),
            Err(StoreError::Conflict(_))
        ));
        assert!(matches!(
            store.rename_category("不存在", "甲"),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn todo_length_limit() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        let too_long = "事".repeat(201);
        let error = store.save_day(
            "2026-09-19",
            &[TodoItem {
                text: too_long,
                done: false,
            }],
        );
        assert!(matches!(error, Err(StoreError::Invalid(_))));
    }

    #[test]
    fn encode_decode_preserves_body() {
        let encoded = encode_note("标题", "第一行\n第二行");
        assert_eq!(decode_note("标题", &encoded), "第一行\n第二行");
        assert_eq!(decode_note("标题", &encode_note("标题", "")), "");
        assert_eq!(decode_note("标题", "没有标题的外部修改"), "没有标题的外部修改");
    }

    #[test]
    fn sample_project_data_is_readable() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data");
        if !root.join("notes").is_dir() {
            return;
        }
        let store = Store::open(&root).unwrap();
        let categories = store.list_categories().unwrap();
        assert!(categories.contains(&"部署".to_string()));
        let note = store.read_note("部署", "测试服务器.md").unwrap();
        assert!(note.body.contains("10.0.0.8"));
        let day = store.load_day("2026-09-19").unwrap();
        assert!(!day.is_empty());
        let index = fs::read_to_string(store.root().join(".meta").join("index.md")).unwrap();
        assert!(index.contains("测试服务器"));
        assert!(!index.contains("AGENTS"));
        let shown = store.display_path();
        assert!(!shown.starts_with(r"\\?\"));
        assert!(shown.contains("data") || Path::new(&shown).is_dir());
    }

    #[test]
    fn lists_and_imports_any_regular_file() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        store.create_category("部署").unwrap();
        let source = _scratch.0.join("手册.docx");
        fs::write(&source, b"PK fake docx").unwrap();
        let imported = store.import_file("部署", &source).unwrap();
        assert_eq!(imported.extension, "docx");
        assert_eq!(imported.kind, "file");
        assert_eq!(imported.file_name, "手册.docx");
        assert!(store.root().join("notes").join("部署").join("手册.docx").is_file());

        let again = store.import_file("部署", &source).unwrap();
        assert_eq!(again.file_name, "手册 2.docx");

        let png = _scratch.0.join("截图.png");
        fs::write(&png, b"PNG").unwrap();
        let image = store.import_file("部署", &png).unwrap();
        assert_eq!(image.extension, "png");
        assert_eq!(image.kind, "file");

        let bare = _scratch.0.join("LICENSE");
        fs::write(&bare, b"mit").unwrap();
        let no_ext = store.import_file("部署", &bare).unwrap();
        assert_eq!(no_ext.extension, "");
        assert_eq!(no_ext.file_name, "LICENSE");

        let exe = _scratch.0.join("tool.exe");
        fs::write(&exe, b"MZ").unwrap();
        let binary = store.import_file("部署", &exe).unwrap();
        assert_eq!(binary.extension, "exe");

        fs::write(
            store.root().join("notes").join("部署").join(".hidden"),
            b"x",
        )
        .unwrap();
        fs::write(
            store.root().join("notes").join("部署").join("draft.tmp"),
            b"x",
        )
        .unwrap();
        fs::create_dir_all(store.root().join("notes").join("部署").join("子目录")).unwrap();

        let listed = store.list_notes(Some("部署"), None).unwrap();
        let names: Vec<_> = listed.iter().map(|item| item.file_name.as_str()).collect();
        assert!(names.contains(&"手册.docx"));
        assert!(names.contains(&"截图.png"));
        assert!(names.contains(&"LICENSE"));
        assert!(names.contains(&"tool.exe"));
        assert!(!names.iter().any(|name| name.starts_with('.')));
        assert!(!names.iter().any(|name| name.ends_with(".tmp")));

        let path = store.absolute_path("部署", "手册.docx").unwrap();
        assert!(path.contains("手册.docx"));
        assert!(!path.starts_with(r"\\?\"));
        store.delete_note("部署", "手册.docx").unwrap();
        assert!(!store.root().join("notes").join("部署").join("手册.docx").exists());
    }

    #[test]
    fn library_order_keeps_dragged_items_and_appends_new_ones() {
        let scratch = Scratch::new();
        let store = scratch.store();
        store.create_category("甲").unwrap();
        store.create_category("乙").unwrap();
        store.reorder_categories(&["乙".into(), "甲".into()]).unwrap();
        assert_eq!(store.list_categories().unwrap(), vec!["乙", "甲"]);
        store.create_category("丙").unwrap();
        assert_eq!(store.list_categories().unwrap(), vec!["乙", "甲", "丙"]);
        store.rename_category("甲", "甲二").unwrap();
        assert_eq!(store.list_categories().unwrap(), vec!["乙", "甲二", "丙"]);

        let dir = store.root().join("notes").join("乙");
        fs::write(dir.join("b.txt"), "b").unwrap();
        fs::write(dir.join("a.txt"), "a").unwrap();
        store.reorder_files("乙", &["b.txt".into(), "a.txt".into()]).unwrap();
        let names = |store: &Store| {
            store
                .list_notes(Some("乙"), None)
                .unwrap()
                .into_iter()
                .map(|note| note.file_name)
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&store), vec!["b.txt", "a.txt"]);
        fs::write(dir.join("c.txt"), "c").unwrap();
        assert_eq!(names(&store), vec!["b.txt", "a.txt", "c.txt"]);
        store.delete_category("甲二").unwrap();
        assert_eq!(store.list_categories().unwrap(), vec!["乙", "丙"]);
        let raw = fs::read_to_string(store.root().join(".meta").join("order.md")).unwrap();
        assert!(raw.contains("## 分类"));
        assert!(raw.contains("- 乙"));
        assert!(!raw.contains("甲二"));
    }

    #[test]
    fn export_writes_markdown_outside_data_and_rejects_inside() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let outside = std::env::temp_dir().join(format!("moyu-export-{}-{}.md", std::process::id(), 7));
        store.write_export(&outside, "# 2026-09\n\n- [ ] 写周报\n").unwrap();
        let written = fs::read_to_string(&outside).unwrap();
        assert!(written.contains("- [ ] 写周报"));
        let _ = fs::remove_file(&outside);
        assert!(store
            .write_export(&scratch.0.join("todos").join("导出.md"), "x")
            .is_err());
        assert!(store.write_export(&outside.with_extension("txt"), "x").is_err());
    }
}
