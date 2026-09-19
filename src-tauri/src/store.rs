use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const MAX_NAME_CHARS: usize = 60;
const MAX_TODO_CHARS: usize = 200;
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "md", "txt", "doc", "docx", "xls", "xlsx", "csv", "ppt", "pptx", "pdf", "rtf",
];

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
        let items = clean_todos(items)?;
        let path = self.root.join("todos").join(format!("{date}.md"));
        write_todo_file(&path, &date, &items)?;
        self.write_index()?;
        Ok(())
    }

    pub fn load_later(&self) -> Result<Vec<TodoItem>, StoreError> {
        parse_todos(&read_to_string(&self.root.join("todos").join("later.md"))?)
    }

    pub fn save_later(&self, items: &[TodoItem]) -> Result<(), StoreError> {
        let items = clean_todos(items)?;
        write_todo_file(&self.root.join("todos").join("later.md"), "稍后", &items)?;
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
        found.sort();
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

        let mut notes = Vec::new();
        for category_name in categories {
            let dir = self.root.join("notes").join(&category_name);
            let mut files: Vec<_> = fs::read_dir(&dir)?.collect::<Result<Vec<_>, _>>()?;
            files.sort_by_key(|entry| entry.file_name());
            for entry in files {
                let path = entry.path();
                let Some(summary) = summarize_entry(&category_name, &path)? else {
                    continue;
                };
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
        if let Some(previous) = previous_file_name.map(str::trim).filter(|value| !value.is_empty()) {
            let previous = validate_file_name(previous)?;
            let source = self.entry_path(&category, &previous);
            let Some(summary) = summarize_entry(&category, &source)? else {
                return Err(StoreError::NotFound("资料不存在".into()));
            };
            if summary.kind != "text" {
                return Err(StoreError::Invalid("Office 文件请用系统程序编辑".into()));
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
        let extension = source
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .unwrap_or_default();
        if !is_supported_extension(&extension) {
            return Err(StoreError::Invalid(
                "暂不支持这种文件，可用 Word / Excel / PPT / PDF / 文本".into(),
            ));
        }
        let stem = source
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("未命名");
        let stem = validate_name(stem)?;
        let mut file_name = format!("{stem}.{extension}");
        let mut destination = self.entry_path(&category, &file_name);
        if destination.exists() {
            let mut index = 2;
            loop {
                file_name = format!("{stem} {index}.{extension}");
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
        self.write_index()?;
        Ok(())
    }

    fn entry_path(&self, category: &str, file_name: &str) -> PathBuf {
        self.root.join("notes").join(category).join(file_name)
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
                if !name.ends_with(".md") {
                    continue;
                }
                any = true;
                let label = if name == "later.md" {
                    "稍后".to_string()
                } else {
                    name.trim_end_matches(".md").to_string()
                };
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

fn summarize_entry(category: &str, path: &Path) -> Result<Option<NoteSummary>, StoreError> {
    if !path.is_file() {
        return Ok(None);
    }
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    if !is_supported_extension(&extension) {
        return Ok(None);
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    if file_name.is_empty() || file_name.starts_with('.') {
        return Ok(None);
    }
    let title = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
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

fn is_supported_extension(ext: &str) -> bool {
    SUPPORTED_EXTENSIONS.iter().any(|item| *item == ext)
}

fn is_text_extension(ext: &str) -> bool {
    matches!(ext, "md" | "txt")
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
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default();
    if !is_supported_extension(&extension) {
        return Err(StoreError::Invalid("不支持的文件类型".into()));
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    validate_name(stem)?;
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
    fn later_list_roundtrip() {
        let _scratch = Scratch::new();
        let store = _scratch.store();
        store
            .save_later(&[TodoItem {
                text: "整理常用话术".into(),
                done: false,
            }])
            .unwrap();
        assert_eq!(store.load_later().unwrap()[0].text, "整理常用话术");
        let raw = fs::read_to_string(store.root().join("todos").join("later.md")).unwrap();
        assert!(raw.starts_with("# 稍后"));
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
        let later = store.load_later().unwrap();
        assert!(!later.is_empty());
        let index = fs::read_to_string(store.root().join(".meta").join("index.md")).unwrap();
        assert!(index.contains("测试服务器"));
        assert!(!index.contains("AGENTS"));
        let shown = store.display_path();
        assert!(!shown.starts_with(r"\\?\"));
        assert!(shown.contains("data") || Path::new(&shown).is_dir());
    }

    #[test]
    fn import_office_like_files_and_reject_unknown() {
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

        let bad = _scratch.0.join("x.exe");
        fs::write(&bad, b"MZ").unwrap();
        assert!(store.import_file("部署", &bad).is_err());

        let path = store.absolute_path("部署", "手册.docx").unwrap();
        assert!(path.contains("手册.docx"));
        assert!(!path.starts_with(r"\\?\"));
        assert!(!store.display_path().starts_with(r"\\?\"));
        store.delete_note("部署", "手册.docx").unwrap();
        assert!(!store.root().join("notes").join("部署").join("手册.docx").exists());
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
