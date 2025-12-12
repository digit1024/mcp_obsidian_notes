use anyhow::{Context, Result};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::{Json, Parameters}},
    model::{ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;
use serde_json::Map as JsonMap;
use chrono::{Local, NaiveDate};
use regex::Regex;
use walkdir::WalkDir;
use std::fs;

pub struct ObsidianService {
    vault_root: PathBuf,
    daily_notes_path: Option<PathBuf>,
    weekly_notes_path: Option<PathBuf>,
    monthly_notes_path: Option<PathBuf>,
    templates_path: Option<PathBuf>,
    tool_router: ToolRouter<Self>,
}

// Request/Response types
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ListNotesDirectoryRequest {
    #[schemars(description = "Directory path relative to vault root (default: '.' for root)")]
    pub path: Option<String>,
    #[schemars(description = "Maximum items to return (default: 50)")]
    pub limit: Option<u32>,
    #[schemars(description = "Pagination offset (default: 0)")]
    pub offset: Option<u32>,
    #[schemars(description = "If true, recursively search subdirectories and return only .md files. If false (default), returns immediate contents (files and directories)")]
    pub recursive: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DirectoryItem {
    pub path: String,
    pub name: String,
    pub is_file: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReadNotesFileRequest {
    #[schemars(description = "Path to the note file relative to vault root. Can include or omit .md extension (auto-added if missing)")]
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FileContent {
    pub content: String,
    #[schemars(description = "YAML frontmatter metadata")]
    pub frontmatter: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DeleteNotesItemRequest {
    #[schemars(description = "Path to file or directory relative to vault root. For files, can include or omit .md extension. For directories, must not include .md extension. Deletes directories recursively.")]
    pub path: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateOrUpdateNoteRequest {
    #[schemars(description = "Path for the note relative to vault root. Should NOT include .md extension (auto-added)")]
    pub path: String,
    #[schemars(description = "Note content body (without frontmatter)")]
    pub content: String,
    #[schemars(description = "YAML frontmatter metadata. If note exists, frontmatter is merged (new keys added, existing keys updated)")]
    pub frontmatter: Option<JsonMap<String, serde_json::Value>>,
    #[schemars(description = "Update mode: 'overwrite' (default) - replaces entire file, 'append' - adds content after existing body, 'prepend' - adds content before existing body")]
    pub mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetDailyNoteRequest {
    #[schemars(description = "Date: 'today' (default), 'yesterday', 'tomorrow', or 'YYYY-MM-DD' format")]
    pub date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchVaultRequest {
    #[schemars(description = "Search query - literal text (case-sensitive substring match)")]
    pub query: String,
    #[schemars(description = "Search scope: array of 'content' (note body), 'filename' (file paths), 'tags' (frontmatter tags). Can specify multiple. Default: ['content', 'filename']")]
    pub scope: Option<Vec<String>>,
    #[schemars(description = "Limit search to specific subdirectory relative to vault root")]
    pub path_filter: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SearchResult {
    pub path: String,
    pub match_preview: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FindRelatedNotesRequest {
    #[schemars(description = "Path to the source note relative to vault root. Can include or omit .md extension (auto-added)")]
    pub path: String,
    #[schemars(description = "Relationship criteria: array of 'tags' and/or 'links'. Extracts tags from frontmatter and wikilinks [[...]] from content, then finds notes with matching tags or filenames. Default: ['tags', 'links']")]
    pub on: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ReplaceTextInNoteRequest {
    #[schemars(description = "Path to the note file relative to vault root. Can include or omit .md extension (auto-added)")]
    pub path: String,
    #[schemars(description = "Literal text to find within the note")]
    pub find: String,
    #[schemars(description = "Replacement text. \\n in string is converted to newline")]
    pub replace: String,
    #[schemars(description = "If true (default), replaces all occurrences. If false, replaces only the first occurrence")]
    pub replace_all: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AppendToSectionRequest {
    #[schemars(description = "Path to the note file relative to vault root. Can include or omit .md extension (auto-added)")]
    pub path: String,
    #[schemars(description = "Section header with # markers (e.g., '## End day'). Must include # to specify level. Whitespace is normalized. Must match exactly (level and text)")]
    pub section_header: String,
    #[schemars(description = "Text to append to the section. \\n in string is converted to newline. Newline is automatically added before this text")]
    pub text_to_append: String,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct UpdateNotePropertiesRequest {
    #[schemars(description = "Path to the note file relative to vault root. Can include or omit .md extension (auto-added)")]
    pub path: String,
    #[schemars(description = "Properties to update or add. Existing properties with same key are overwritten. New properties are added.")]
    pub properties: Option<HashMap<String, serde_json::Value>>,
    #[schemars(description = "Property keys to remove from frontmatter")]
    pub remove: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CreateNoteFromTemplateRequest {
    #[schemars(description = "Destination path for the new note relative to vault root. SHOULD include .md extension")]
    pub path: String,
    #[schemars(description = "Template path: if starts with '/' or contains ':', treated as absolute path relative to vault root; otherwise relative to templates directory (paths from list_notes_templates can be used directly)")]
    pub template_path: String,
    #[schemars(description = "Key-value pairs for template substitution. Replaces {{variable}} placeholders in template")]
    pub variables: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OperationResult {
    pub success: bool,
    #[schemars(description = "Path of the affected file")]
    pub path: Option<String>,
    #[schemars(description = "Error message if operation failed")]
    pub error: Option<String>,
    #[schemars(description = "Path of deleted item (for delete operations)")]
    #[serde(rename = "deleted")]
    pub deleted_path: Option<String>,
}

#[tool_router]
impl ObsidianService {
    pub fn new(
        vault_root: &str,
        daily_notes_path: Option<&str>,
        weekly_notes_path: Option<&str>,
        monthly_notes_path: Option<&str>,
        templates_path: Option<&str>,
    ) -> Result<Self> {
        let vault_path = PathBuf::from(vault_root);
        if !vault_path.exists() {
            anyhow::bail!("Vault location does not exist: {}", vault_root);
        }

        let daily = daily_notes_path.map(PathBuf::from);
        let weekly = weekly_notes_path.map(PathBuf::from);
        let monthly = monthly_notes_path.map(PathBuf::from);
        let templates = templates_path.map(PathBuf::from);

        Ok(Self {
            vault_root: vault_path.canonicalize()?,
            daily_notes_path: daily,
            weekly_notes_path: weekly,
            monthly_notes_path: monthly,
            templates_path: templates,
            tool_router: Self::tool_router(),
        })
    }

    // Helper: Validate path is within vault
    fn validate_path(&self, path: &str) -> Result<PathBuf> {
        let full_path = self.vault_root.join(path);
        let canonical = full_path.canonicalize()
            .with_context(|| format!("Path does not exist or cannot be accessed: {}", path))?;
        
        if !canonical.starts_with(&self.vault_root) {
            anyhow::bail!("Path is outside vault root: {}", path);
        }
        Ok(canonical)
    }


    // Helper: Ensure path has .md extension
    fn ensure_md_extension(&self, path: &str) -> String {
        if path.ends_with(".md") {
            path.to_string()
        } else {
            format!("{}.md", path)
        }
    }

    // Helper: Normalize newlines - convert \n to actual newlines
    fn normalize_newlines(text: &str) -> String {
        text.replace("\\n", "\n")
    }

    // Helper: Parse header line - returns (level, text) if it's a header
    fn parse_header_line(line: &str) -> Option<(u32, String)> {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            return None;
        }
        
        let level = trimmed.chars().take_while(|c| *c == '#').count() as u32;
        if level == 0 || level > 6 {
            return None;
        }
        
        let text = trimmed[level as usize..].trim().to_string();
        if text.is_empty() {
            return None;
        }
        
        Some((level, text))
    }

    // Helper: Parse section header from user input
    fn parse_section_header(header: &str) -> Result<(u32, String), String> {
        let trimmed = header.trim();
        
        if !trimmed.starts_with('#') {
            return Err("Header level must be specified. Provide header with # markers (e.g., '## End day')".to_string());
        }
        
        let level = trimmed.chars().take_while(|c| *c == '#').count() as u32;
        if level == 0 || level > 6 {
            return Err("Invalid header level. Must be 1-6 (# to ######)".to_string());
        }
        
        let text = trimmed[level as usize..].trim().to_string();
        if text.is_empty() {
            return Err("Header text cannot be empty".to_string());
        }
        
        Ok((level, text))
    }

    // Helper: Find section matches in content - returns (line_number, level, text)
    fn find_sections(content: &str, target_level: u32, target_text: &str) -> Vec<(usize, u32, String)> {
        let lines: Vec<&str> = content.lines().collect();
        let mut matches = Vec::new();
        
        for (line_num, line) in lines.iter().enumerate() {
            if let Some((level, text)) = Self::parse_header_line(line) {
                if level == target_level && text.trim() == target_text.trim() {
                    matches.push((line_num, level, text));
                }
            }
        }
        
        matches
    }

    // Helper: Parse frontmatter from content
    fn parse_frontmatter(content: &str) -> (Option<JsonMap<String, serde_json::Value>>, String) {
        if !content.starts_with("---\n") {
            return (None, content.to_string());
        }

        if let Some(end_pos) = content[4..].find("\n---\n") {
            let yaml_str = &content[4..end_pos + 4];
            let body = &content[end_pos + 9..];
            
            match serde_yaml::from_str::<JsonMap<String, serde_json::Value>>(yaml_str) {
                Ok(fm) => (Some(fm), body.to_string()),
                Err(_) => (None, content.to_string()),
            }
        } else {
            (None, content.to_string())
        }
    }

    // Helper: Format content with frontmatter
    fn format_with_frontmatter(content: &str, frontmatter: Option<&JsonMap<String, serde_json::Value>>) -> String {
        if let Some(fm) = frontmatter {
            let yaml_str = serde_yaml::to_string(fm).unwrap_or_else(|_| "".to_string());
            format!("---\n{}\n---\n\n{}", yaml_str.trim(), content)
        } else {
            content.to_string()
        }
    }

    // Helper: Get date for daily note
    fn parse_date(date_str: Option<&String>) -> Result<NaiveDate> {
        let date_str = date_str.map(|s| s.as_str()).unwrap_or("today");
        let today = Local::now().date_naive();
        
        match date_str {
            "today" => Ok(today),
            "yesterday" => Ok(today - chrono::Duration::days(1)),
            "tomorrow" => Ok(today + chrono::Duration::days(1)),
            _ => {
                NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                    .context("Date must be 'today', 'yesterday', 'tomorrow', or YYYY-MM-DD")
            }
        }
    }

    // Helper: Find daily note file
    fn find_daily_note(&self, date: NaiveDate) -> Result<PathBuf> {
        let date_str = date.format("%Y-%m-%d").to_string();
        let mut candidates = vec![
            format!("{}.md", date_str),
            format!("daily/{}.md", date_str),
            format!("Daily Notes/{}.md", date_str),
            format!("daily/{}.md", date_str),
        ];

        if let Some(daily_path) = &self.daily_notes_path {
            candidates.insert(0, daily_path.join(format!("{}.md", date_str)).to_string_lossy().to_string());
        }

        for candidate in candidates {
            let full_path = self.vault_root.join(&candidate);
            if full_path.exists() {
                return Ok(full_path);
            }
        }

        anyhow::bail!("Daily note not found for date: {}", date_str);
    }

    #[tool(description = "List files and directories in a vault directory. Returns both files and directories by default. When recursive=true, only returns .md files from subdirectories. Path is relative to vault root (use '.' for root). Returns empty array if path doesn't exist.")]
    pub fn list_notes_directory(
        &self,
        Parameters(ListNotesDirectoryRequest { path, limit, offset, recursive }): Parameters<ListNotesDirectoryRequest>,
    ) -> Json<Vec<DirectoryItem>> {
        let path = path.unwrap_or_else(|| ".".to_string());
        let limit = limit.unwrap_or(50) as usize;
        let offset = offset.unwrap_or(0) as usize;
        let recursive = recursive.unwrap_or(false);

        let dir_path = match self.validate_path(&path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Invalid path: {}", e);
                return Json(Vec::new());
            }
        };

        let mut items = Vec::new();

        if recursive {
            if dir_path.is_dir() {
                for entry in WalkDir::new(&dir_path)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .skip(offset)
                    .take(limit)
                {
                    let entry_path = entry.path();
                    if entry_path.is_file() && entry_path.extension().and_then(|s| s.to_str()) == Some("md") {
                        if let Ok(rel_path) = entry_path.strip_prefix(&self.vault_root) {
                            let metadata = entry.metadata().ok();
                            items.push(DirectoryItem {
                                path: rel_path.to_string_lossy().to_string(),
                                name: entry_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                                is_file: true,
                                size: metadata.and_then(|m| Some(m.len())),
                            });
                        }
                    }
                }
            }
        } else {
            if dir_path.is_dir() {
                let entries: Vec<_> = fs::read_dir(&dir_path)
                    .ok()
                    .map(|rd| rd.filter_map(|e| e.ok()).collect())
                    .unwrap_or_default();
                
                for entry in entries.into_iter().skip(offset).take(limit) {
                    let entry_path = entry.path();
                    if let Ok(rel_path) = entry_path.strip_prefix(&self.vault_root) {
                        let metadata = entry.metadata().ok();
                        items.push(DirectoryItem {
                            path: rel_path.to_string_lossy().to_string(),
                            name: entry_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                            is_file: entry_path.is_file(),
                            size: metadata.filter(|_| entry_path.is_file()).and_then(|m| Some(m.len())),
                        });
                    }
                }
            }
        }

        Json(items)
    }

    #[tool(description = "Read a markdown note file from the vault. Path can include or omit .md extension (auto-added if missing). Path is relative to vault root. Returns content body (without frontmatter) and frontmatter separately as YAML metadata.")]
    pub fn read_notes_file(
        &self,
        Parameters(ReadNotesFileRequest { path }): Parameters<ReadNotesFileRequest>,
    ) -> Json<FileContent> {
        let path_with_ext = self.ensure_md_extension(&path);
        match self.validate_path(&path_with_ext) {
            Ok(full_path) => {
                match fs::read_to_string(&full_path) {
                    Ok(content) => {
                        let (frontmatter, body) = Self::parse_frontmatter(&content);
                        Json(FileContent {
                            content: body,
                            frontmatter,
                        })
                    }
                    Err(e) => {
                        eprintln!("Failed to read file {}: {}", path_with_ext, e);
                        Json(FileContent {
                            content: format!("Error reading file: {}", e),
                            frontmatter: None,
                        })
                    }
                }
            }
            Err(e) => {
                eprintln!("Invalid path {}: {}", path_with_ext, e);
                Json(FileContent {
                    content: format!("Error: {}", e),
                    frontmatter: None,
                })
            }
        }
    }

    #[tool(description = "Delete a file or directory from the vault. For files, path can include or omit .md extension. For directories, path must not include .md extension. Deletes directories recursively. Path is relative to vault root. Returns error if path doesn't exist.")]
    pub fn delete_notes_item(
        &self,
        Parameters(DeleteNotesItemRequest { path }): Parameters<DeleteNotesItemRequest>,
    ) -> Json<OperationResult> {
        // Try with .md extension first (for files), then without (for directories)
        let path_with_ext = self.ensure_md_extension(&path);
        let result = match self.validate_path(&path_with_ext) {
            Ok(full_path) => {
                if full_path.is_dir() {
                    fs::remove_dir_all(&full_path)
                } else {
                    fs::remove_file(&full_path)
                }
            }
            Err(_) => {
                // If path with .md doesn't exist, try without extension (might be directory)
                match self.validate_path(&path) {
                    Ok(full_path) => {
                        if full_path.is_dir() {
                            fs::remove_dir_all(&full_path)
                        } else {
                            fs::remove_file(&full_path)
                        }
                    }
                    Err(e) => {
                        eprintln!("Invalid path {}: {}", path, e);
                        return Json(OperationResult {
                            success: false,
                            path: None,
                            error: Some(format!("{}", e)),
                            deleted_path: None,
                        });
                    }
                }
            }
        };

        match result {
                
            Ok(_) => Json(OperationResult {
                success: true,
                path: None,
                error: None,
                deleted_path: Some(path),
            }),
            Err(e) => {
                eprintln!("Failed to delete {}: {}", path, e);
                Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                })
            }
        }
    }

    #[tool(description = "Create a new note or update existing one. Path should NOT include .md extension (auto-added). Mode options: 'overwrite' (default) - replaces entire file, 'append' - adds content after existing body, 'prepend' - adds content before existing body. Frontmatter is merged (new keys added, existing keys updated). Creates parent directories if needed. Path is relative to vault root.")]
    pub fn create_or_update_note(
        &self,
        Parameters(CreateOrUpdateNoteRequest { path, content, frontmatter, mode }): Parameters<CreateOrUpdateNoteRequest>,
    ) -> Json<OperationResult> {
        let md_path = self.ensure_md_extension(&path);
        let full_path = self.vault_root.join(&md_path);

        // Create parent directory if needed
        if let Some(parent) = full_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Failed to create directory: {}", e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        }

        let mode = mode.as_deref().unwrap_or("overwrite");
        let final_content = if full_path.exists() && mode != "overwrite" {
            match fs::read_to_string(&full_path) {
                Ok(existing) => {
                    let (existing_fm, existing_body) = Self::parse_frontmatter(&existing);
                    
                    let merged_fm = match (existing_fm, &frontmatter) {
                        (Some(mut fm), Some(new_fm)) => {
                            fm.extend(new_fm.clone());
                            Some(fm)
                        }
                        (Some(fm), None) => Some(fm),
                        (None, Some(new_fm)) => Some(new_fm.clone()),
                        (None, None) => None,
                    };

                    let body = match mode {
                        "append" => format!("{}\n{}", existing_body, content),
                        "prepend" => format!("{}\n{}", content, existing_body),
                        _ => existing_body,
                    };

                    Self::format_with_frontmatter(&body, merged_fm.as_ref())
                }
                Err(e) => {
                    eprintln!("Failed to read existing file: {}", e);
                    return Json(OperationResult {
                        success: false,
                        path: None,
                        error: Some(format!("{}", e)),
                        deleted_path: None,
                    });
                }
            }
        } else {
            Self::format_with_frontmatter(&content, frontmatter.as_ref())
        };

        match fs::write(&full_path, final_content) {
            Ok(_) => Json(OperationResult {
                success: true,
                path: Some(md_path),
                error: None,
                deleted_path: None,
            }),
            Err(e) => {
                eprintln!("Failed to write file: {}", e);
                Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                })
            }
        }
    }

    #[tool(description = "Get daily note for a date. Date can be 'today' (default), 'yesterday', 'tomorrow', or 'YYYY-MM-DD' format. Searches common locations: configured daily_notes_path, root, 'daily/', 'Daily Notes/'. Returns error message in content field if note not found.")]
    pub fn get_daily_note(
        &self,
        Parameters(GetDailyNoteRequest { date }): Parameters<GetDailyNoteRequest>,
    ) -> Json<FileContent> {
        match Self::parse_date(date.as_ref()) {
            Ok(target_date) => {
                match self.find_daily_note(target_date) {
                    Ok(note_path) => {
                        match fs::read_to_string(&note_path) {
                            Ok(content) => {
                                let (frontmatter, body) = Self::parse_frontmatter(&content);
                                Json(FileContent {
                                    content: body,
                                    frontmatter,
                                })
                            }
                            Err(e) => {
                                eprintln!("Failed to read daily note: {}", e);
                                Json(FileContent {
                                    content: format!("Error reading file: {}", e),
                                    frontmatter: None,
                                })
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Daily note not found: {}", e);
                        Json(FileContent {
                            content: format!("Error: {}", e),
                            frontmatter: None,
                        })
                    }
                }
            }
            Err(e) => {
                eprintln!("Invalid date: {}", e);
                Json(FileContent {
                    content: format!("Error: {}", e),
                    frontmatter: None,
                })
            }
        }
    }

    #[tool(description = "Search for text in vault notes. Query is literal text (case-sensitive substring match). Scope options: 'content' (note body), 'filename' (file paths), 'tags' (frontmatter tags). Can specify multiple scopes. path_filter limits search to specific subdirectory (relative to vault root). Returns file paths and match previews.")]
    pub fn search_vault(
        &self,
        Parameters(SearchVaultRequest { query, scope, path_filter }): Parameters<SearchVaultRequest>,
    ) -> Json<Vec<SearchResult>> {
        let scope = scope.unwrap_or_else(|| vec!["content".to_string(), "filename".to_string()]);
        let query_regex = Regex::new(&regex::escape(&query)).ok();
        let mut results = Vec::new();

        let search_root = if let Some(filter) = path_filter {
            match self.validate_path(&filter) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Invalid path filter: {}", e);
                    return Json(Vec::new());
                }
            }
        } else {
            self.vault_root.clone()
        };

        for entry in WalkDir::new(&search_root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let entry_path = entry.path();
            if !entry_path.is_file() || entry_path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }

            if let Ok(rel_path) = entry_path.strip_prefix(&self.vault_root) {
                let rel_path_str = rel_path.to_string_lossy().to_string();

                // Search filename
                if scope.contains(&"filename".to_string()) {
                    if let Some(re) = &query_regex {
                        if re.is_match(&rel_path_str) {
                            results.push(SearchResult {
                                path: rel_path_str.clone(),
                                match_preview: Some(format!("Filename match: {}", rel_path_str)),
                            });
                            continue;
                        }
                    }
                }

                // Search content and tags
                if scope.contains(&"content".to_string()) || scope.contains(&"tags".to_string()) {
                    if let Ok(content) = fs::read_to_string(entry_path) {
                        // Search tags in frontmatter
                        if scope.contains(&"tags".to_string()) {
                            let (fm, _) = Self::parse_frontmatter(&content);
                            if let Some(frontmatter) = fm {
                                if let Some(tags) = frontmatter.get("tags") {
                                    let tags_str = serde_json::to_string(tags).unwrap_or_default();
                                    if let Some(re) = &query_regex {
                                        if re.is_match(&tags_str) {
                                            results.push(SearchResult {
                                                path: rel_path_str.clone(),
                                                match_preview: Some(format!("Tag match: {}", tags_str)),
                                            });
                                            continue;
                                        }
                                    }
                                }
                            }
                        }

                        // Search content
                        if scope.contains(&"content".to_string()) {
                            if let Some(re) = &query_regex {
                                if let Some(mat) = re.find(&content) {
                                    let start = mat.start().saturating_sub(50);
                                    let end = (mat.end() + 50).min(content.len());
                                    let preview = content[start..end].to_string();
                                    results.push(SearchResult {
                                        path: rel_path_str.clone(),
                                        match_preview: Some(preview),
                                    });
                                }
                            } else if content.contains(&query) {
                                let idx = content.find(&query).unwrap_or(0);
                                let start = idx.saturating_sub(50);
                                let end = (idx + query.len() + 50).min(content.len());
                                let preview = content[start..end].to_string();
                                results.push(SearchResult {
                                    path: rel_path_str.clone(),
                                    match_preview: Some(preview),
                                });
                            }
                        }
                    }
                }
            }
        }

        Json(results)
    }

    #[tool(description = "Find notes related to a source note. Extracts tags from source note's frontmatter and wikilinks [[...]] from content. Finds other notes that: (1) have matching tags in frontmatter, or (2) have filenames matching extracted link names. 'on' parameter controls which relationships to use: 'tags' and/or 'links'. Path is relative to vault root. Returns empty array if source note not found.")]
    pub fn find_related_notes(
        &self,
        Parameters(FindRelatedNotesRequest { path, on }): Parameters<FindRelatedNotesRequest>,
    ) -> Json<Vec<SearchResult>> {
        let on = on.unwrap_or_else(|| vec!["tags".to_string(), "links".to_string()]);
        
        let path_with_ext = self.ensure_md_extension(&path);
        let (frontmatter, body, full_path) = match self.validate_path(&path_with_ext) {
            Ok(full_path) => {
                match fs::read_to_string(&full_path) {
                    Ok(content) => {
                        let (fm, body) = Self::parse_frontmatter(&content);
                        (fm, body, full_path)
                    }
                    Err(e) => {
                        eprintln!("Failed to read file {}: {}", path_with_ext, e);
                        return Json(Vec::new());
                    }
                }
            }
            Err(e) => {
                eprintln!("Invalid path {}: {}", path_with_ext, e);
                return Json(Vec::new());
            }
        };

        let mut related = Vec::new();
        let mut search_terms = Vec::new();

        // Extract tags
        if on.contains(&"tags".to_string()) {
            if let Some(fm) = &frontmatter {
                if let Some(tags) = fm.get("tags") {
                    if let Ok(tags_vec) = serde_json::from_value::<Vec<String>>(tags.clone()) {
                        search_terms.extend(tags_vec);
                    }
                }
            }
        }

        // Extract links
        if on.contains(&"links".to_string()) {
            let link_regex = Regex::new(r"\[\[([^\]]+)\]\]").unwrap();
            for cap in link_regex.captures_iter(&body) {
                if let Some(link) = cap.get(1) {
                    search_terms.push(link.as_str().to_string());
                }
            }
        }

        // Find notes with matching tags or names
        for term in search_terms {
            for entry in WalkDir::new(&self.vault_root)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if !entry_path.is_file() || entry_path == full_path {
                    continue;
                }

                if let Ok(rel_path) = entry_path.strip_prefix(&self.vault_root) {
                    let rel_path_str = rel_path.to_string_lossy().to_string();
                    
                    // Check if filename matches
                    if rel_path_str.contains(&term) || rel_path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.contains(&term))
                        .unwrap_or(false) {
                        related.push(SearchResult {
                            path: rel_path_str.clone(),
                            match_preview: Some(format!("Related via: {}", term)),
                        });
                    } else if let Ok(file_content) = fs::read_to_string(entry_path) {
                        // Check tags in other files
                        let (other_fm, _) = Self::parse_frontmatter(&file_content);
                        if let Some(other_fm) = other_fm {
                            if let Some(other_tags) = other_fm.get("tags") {
                                if let Ok(tags_vec) = serde_json::from_value::<Vec<String>>(other_tags.clone()) {
                                    if tags_vec.contains(&term) {
                                        related.push(SearchResult {
                                            path: rel_path_str,
                                            match_preview: Some(format!("Shared tag: {}", term)),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Json(related)
    }

    #[tool(description = "Replace text in a note. Finds target text and replaces it with new content. replace_all (default: true) controls whether to replace all occurrences or just the first. Path is relative to vault root, .md extension auto-added. Returns error if target text not found.")]
    pub fn replace_text_in_note(
        &self,
        Parameters(ReplaceTextInNoteRequest { path, find, replace, replace_all }): Parameters<ReplaceTextInNoteRequest>,
    ) -> Json<OperationResult> {
        let path_with_ext = self.ensure_md_extension(&path);
        let full_path = match self.validate_path(&path_with_ext) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Invalid path {}: {}", path, e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        };
        
        let file_content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read file {}: {}", path, e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        };

        let replace_all = replace_all.unwrap_or(true);
        let normalized_replace = Self::normalize_newlines(&replace);
        let find_regex = match Regex::new(&regex::escape(&find)) {
            Ok(re) => re,
            Err(e) => {
                eprintln!("Invalid regex pattern: {}", e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("Invalid pattern: {}", e)),
                    deleted_path: None,
                });
            }
        };

        if !find_regex.is_match(&file_content) {
            eprintln!("Target text not found in file");
            return Json(OperationResult {
                success: false,
                path: None,
                error: Some("Target text not found in file".to_string()),
                deleted_path: None,
            });
        }

        let new_content = if replace_all {
            find_regex.replace_all(&file_content, &normalized_replace).to_string()
        } else {
            find_regex.replace(&file_content, &normalized_replace).to_string()
        };

        match fs::write(&full_path, new_content) {
            Ok(_) => Json(OperationResult {
                success: true,
                path: Some(path_with_ext),
                error: None,
                deleted_path: None,
            }),
            Err(e) => {
                eprintln!("Failed to write file: {}", e);
                Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                })
            }
        }
    }

    #[tool(description = "Append text to a specific markdown section. section_header must include # markers (e.g., '## End day') and must match exactly (level and text). Appends content before the next header of the same or higher level (or at end of file). Returns error if: header level not specified, section not found, level mismatch, or multiple sections match. Path is relative to vault root, .md extension auto-added.")]
    pub fn append_to_section(
        &self,
        Parameters(AppendToSectionRequest { path, section_header, text_to_append }): Parameters<AppendToSectionRequest>,
    ) -> Json<OperationResult> {
        let path_with_ext = self.ensure_md_extension(&path);
        let full_path = match self.validate_path(&path_with_ext) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Invalid path {}: {}", path, e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        };

        let file_content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read file {}: {}", path, e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        };

        // Parse section header
        let (target_level, target_text) = match Self::parse_section_header(&section_header) {
            Ok((level, text)) => (level, text),
            Err(e) => {
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(e),
                    deleted_path: None,
                });
            }
        };

        // Find matching sections
        let matches = Self::find_sections(&file_content, target_level, &target_text);
        
        match matches.len() {
            0 => {
                // Check if there's a level mismatch
                let all_headers: Vec<_> = file_content.lines()
                    .enumerate()
                    .filter_map(|(i, line)| {
                        Self::parse_header_line(line).map(|(level, text)| (i, level, text))
                    })
                    .filter(|(_, _, text)| text.trim() == target_text.trim())
                    .collect();
                
                if !all_headers.is_empty() {
                    let header_info: Vec<String> = all_headers.iter()
                        .map(|(line, level, _)| format!("'{}' at line {}", "#".repeat(*level as usize), line + 1))
                        .collect();
                    return Json(OperationResult {
                        success: false,
                        path: None,
                        error: Some(format!("Section not found. Header level mismatch. Looking for '{} {}' but found {}",
                            "#".repeat(target_level as usize), target_text, header_info.join(", "))),
                        deleted_path: None,
                    });
                }
                
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("Section not found. No header matching '{} {}' found in file",
                        "#".repeat(target_level as usize), target_text)),
                    deleted_path: None,
                });
            }
            1 => {
                // Found exactly one match - proceed
            }
            n => {
                let line_numbers: Vec<String> = matches.iter()
                    .map(|(line, _, _)| (line + 1).to_string())
                    .collect();
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("Multiple sections found. Found {} headers matching '{} {}' at lines {}. Use replace_text_in_note for precise targeting.",
                        n, "#".repeat(target_level as usize), target_text, line_numbers.join(", "))),
                    deleted_path: None,
                });
            }
        }

        let (target_line, _, _) = matches[0];
        let lines: Vec<&str> = file_content.lines().collect();
        
        // Find insertion point: after target header, before next header of same or higher level
        let mut insert_line = target_line + 1;
        
        // Find next header of same or higher level
        for i in (target_line + 1)..lines.len() {
            if let Some((level, _)) = Self::parse_header_line(lines[i]) {
                if level <= target_level {
                    // Found next header of same or higher level - insert before it
                    insert_line = i;
                    break;
                }
            }
        }
        // If no next header found, insert_line will be at end of file
        
        // Normalize newlines in text_to_append
        let normalized_text = Self::normalize_newlines(&text_to_append);
        
        // Build new content
        let mut new_lines: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        
        // Determine if we need to add a newline before the text
        // Always add a newline before the appended text for consistency
        let text_to_insert = format!("\n{}", normalized_text);
        
        if insert_line >= new_lines.len() {
            // Append to end of file
            // Check if last line is empty - if so, don't add extra newline
            if new_lines.is_empty() || new_lines[new_lines.len() - 1].trim().is_empty() {
                new_lines.push(normalized_text);
            } else {
                new_lines.push(text_to_insert);
            }
        } else {
            // Insert before the line
            new_lines.insert(insert_line, text_to_insert);
        }
        
        let new_content = new_lines.join("\n");
        
        match fs::write(&full_path, new_content) {
            Ok(_) => Json(OperationResult {
                success: true,
                path: Some(path_with_ext),
                error: None,
                deleted_path: None,
            }),
            Err(e) => {
                eprintln!("Failed to write file: {}", e);
                Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                })
            }
        }
    }

    #[tool(description = "Update frontmatter properties (Obsidian properties) in a note. Updates/adds properties from 'properties' map and removes properties listed in 'remove'. Does not modify note content body. Creates frontmatter if it doesn't exist. Path is relative to vault root, .md extension auto-added.")]
    pub fn update_note_properties(
        &self,
        Parameters(UpdateNotePropertiesRequest { path, properties, remove }): Parameters<UpdateNotePropertiesRequest>,
    ) -> Json<OperationResult> {
        let path_with_ext = self.ensure_md_extension(&path);
        let full_path = match self.validate_path(&path_with_ext) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Invalid path {}: {}", path, e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        };

        let file_content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read file {}: {}", path, e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        };

        // Parse frontmatter and body
        let (existing_fm, body) = Self::parse_frontmatter(&file_content);
        
        // Start with existing frontmatter or create new
        let mut fm = existing_fm.unwrap_or_else(|| JsonMap::new());

        // Update/add properties
        if let Some(props) = properties {
            for (key, value) in props {
                fm.insert(key, value);
            }
        }

        // Remove properties
        if let Some(keys_to_remove) = remove {
            for key in keys_to_remove {
                fm.remove(&key);
            }
        }

        // Format with updated frontmatter
        let new_content = Self::format_with_frontmatter(&body, Some(&fm));

        match fs::write(&full_path, new_content) {
            Ok(_) => Json(OperationResult {
                success: true,
                path: Some(path_with_ext),
                error: None,
                deleted_path: None,
            }),
            Err(e) => {
                eprintln!("Failed to write file: {}", e);
                Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                })
            }
        }
    }

    #[tool(description = "Create note from template with variable substitution. Template path: if starts with '/' or contains ':', treated as absolute path relative to vault root; otherwise relative to templates directory (paths from list_notes_templates can be used directly). Destination path SHOULD include .md extension. Replaces {{variable}} placeholders in template with values from variables map. Creates parent directories if needed.")]
    pub fn create_note_from_template(
        &self,
        Parameters(CreateNoteFromTemplateRequest { path, template_path, variables }): Parameters<CreateNoteFromTemplateRequest>,
    ) -> Json<OperationResult> {
        let template_full = if template_path.starts_with('/') || template_path.contains(':') {
            // Absolute path
            match self.validate_path(&template_path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Invalid template path {}: {}", template_path, e);
                    return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
                }
            }
        } else {
            // Relative to templates directory
            let templates_dir = self.templates_path.as_ref()
                .map(|p| self.vault_root.join(p))
                .unwrap_or_else(|| self.vault_root.join("templates"));
            templates_dir.join(&template_path)
        };

        if !template_full.exists() {
            eprintln!("Template file not found: {}", template_path);
            return Json(OperationResult {
                success: false,
                path: None,
                error: Some(format!("Template file not found: {}", template_path)),
                deleted_path: None,
            });
        }

        let template_content = match fs::read_to_string(&template_full) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read template: {}", e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        };
        
        let variables = variables.unwrap_or_default();

        // Replace {{variable}} placeholders
        let mut final_content = template_content;
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            final_content = final_content.replace(&placeholder, &value);
        }

        // Write to destination (ensure .md extension)
        let final_path = self.ensure_md_extension(&path);
        let dest_path = self.vault_root.join(&final_path);
        if let Some(parent) = dest_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("Failed to create directory: {}", e);
                return Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                });
            }
        }

        match fs::write(&dest_path, final_content) {
            Ok(_) => Json(OperationResult {
                success: true,
                path: Some(final_path),
                error: None,
                deleted_path: None,
            }),
            Err(e) => {
                eprintln!("Failed to write file: {}", e);
                Json(OperationResult {
                    success: false,
                    path: None,
                    error: Some(format!("{}", e)),
                    deleted_path: None,
                })
            }
        }
    }

    #[tool(description = "List all .md template files in templates directory. Returns paths relative to templates directory (can be used directly with create_note_from_template). Templates directory is configured templates_path or 'templates/' in vault root. Returns template file paths, names, and sizes. Returns empty array if templates directory doesn't exist.")]
    pub fn list_notes_templates(&self) -> Json<Vec<DirectoryItem>> {
        let templates_dir = self.templates_path.as_ref()
            .map(|p| self.vault_root.join(p))
            .unwrap_or_else(|| self.vault_root.join("templates"));

        let mut items = Vec::new();

        if templates_dir.exists() && templates_dir.is_dir() {
            for entry in fs::read_dir(&templates_dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
            {
                let entry_path = entry.path();
                if entry_path.is_file() && entry_path.extension().and_then(|s| s.to_str()) == Some("md") {
                    // Return path relative to templates directory (not vault root)
                    // This allows direct use with create_note_from_template
                    if let Ok(rel_path) = entry_path.strip_prefix(&templates_dir) {
                        let metadata = entry.metadata().ok();
                        items.push(DirectoryItem {
                            path: rel_path.to_string_lossy().to_string(),
                            name: entry_path.file_name().unwrap_or_default().to_string_lossy().to_string(),
                            is_file: true,
                            size: metadata.and_then(|m| Some(m.len())),
                        });
                    }
                }
            }
        }

        Json(items)
    }
}

#[tool_handler]
impl ServerHandler for ObsidianService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("MCP server for interacting with Obsidian notes without requiring Obsidian to be running.".to_string()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
