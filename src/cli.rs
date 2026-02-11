mod config;
mod cli_utils;
mod service;
mod template_processor;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde_json;
use std::collections::HashMap;

use config::{load_config, save_config, Config};
use cli_utils::resolve_content;
use service::{
    ObsidianService,
    AppendToSectionRequest,
    CreateNoteFromTemplateRequest,
    CreateOrUpdateNoteRequest,
    DeleteNotesItemRequest,
    FindRelatedNotesRequest,
    GetDailyNoteRequest,
    GetNotePropertiesRequest,
    ListNotesDirectoryRequest,
    ReadNotesFileRequest,
    ReplaceTextInNoteRequest,
    SearchVaultRequest,
    UpdateNotePropertiesRequest,
};

#[derive(Parser)]
#[command(name = "obsidian-cli")]
#[command(about = "CLI for Obsidian vault operations", long_about = None)]
#[command(arg_required_else_help = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set and save vault path to config
    SetVaultPath {
        path: String,
    },
    /// List files and directories in a vault directory
    ListNotesDirectory {
        #[arg(long, default_value = ".")]
        path: Option<String>,
        #[arg(long, default_value = "50")]
        limit: Option<u32>,
        #[arg(long, default_value = "0")]
        offset: Option<u32>,
        #[arg(long)]
        recursive: bool,
    },
    /// Read a markdown note file
    ReadNotesFile {
        path: String,
    },
    /// Delete a file or directory
    DeleteNotesItem {
        path: String,
    },
    /// Create or update a note
    CreateOrUpdateNote {
        path: String,
        /// Content (or @file, - for stdin). Use \\n for newlines in literal.
        #[arg(long, conflicts_with = "content_file", conflicts_with = "content_stdin")]
        content: Option<String>,
        /// Read content from file
        #[arg(long)]
        content_file: Option<String>,
        /// Read content from stdin
        #[arg(long)]
        content_stdin: bool,
        /// Frontmatter as JSON, e.g. '{"tags":["a","b"]}'
        #[arg(long)]
        frontmatter: Option<String>,
        /// overwrite | append | prepend
        #[arg(long, default_value = "overwrite")]
        mode: String,
    },
    /// Get daily note for a date
    GetDailyNote {
        #[arg(long, default_value = "today")]
        date: String,
    },
    /// Search vault for text
    SearchVault {
        query: String,
        #[arg(long)]
        scope: Option<Vec<String>>,
        #[arg(long)]
        path_filter: Option<String>,
    },
    /// Find notes related to a source note
    FindRelatedNotes {
        path: String,
        #[arg(long)]
        on: Option<Vec<String>>,
    },
    /// Replace text in a note
    ReplaceTextInNote {
        path: String,
        #[arg(long)]
        find: String,
        /// Replacement (or @file, - for stdin)
        #[arg(long, required = true)]
        replace: String,
        #[arg(long, default_value = "true")]
        replace_all: bool,
    },
    /// Append text to a section
    AppendToSection {
        path: String,
        #[arg(long)]
        section_header: String,
        /// Text to append (or @file, - for stdin)
        #[arg(long, required = true)]
        text: String,
    },
    /// Update frontmatter properties
    UpdateNoteProperties {
        path: String,
        /// Key=value (repeatable). Value can be JSON.
        #[arg(long)]
        set: Option<Vec<String>>,
        #[arg(long)]
        remove: Option<Vec<String>>,
    },
    /// Get frontmatter properties
    GetNoteProperties {
        path: String,
    },
    /// Create note from template
    CreateNoteFromTemplate {
        path: String,
        template_path: String,
        /// key=value for template variables (repeatable)
        #[arg(long)]
        var: Option<Vec<String>>,
    },
    /// List template files
    ListNotesTemplates,
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn parse_key_value_pairs(pairs: Vec<String>) -> Result<HashMap<String, serde_json::Value>> {
    let mut map = HashMap::new();
    for s in pairs {
        if let Some((k, v)) = s.split_once('=') {
            let v = v.trim();

            // 1. Try valid JSON first (handles ["a","b"], true, 42, "quoted string", etc.)
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(v) {
                map.insert(k.to_string(), val);
                continue;
            }

            // 2. Bracket-wrapped list without quotes: [daily, note] → JSON array
            if v.starts_with('[') && v.ends_with(']') {
                let inner = &v[1..v.len() - 1];
                let items: Vec<serde_json::Value> = inner
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| serde_json::Value::String(s.to_string()))
                    .collect();
                map.insert(k.to_string(), serde_json::Value::Array(items));
                continue;
            }

            // 3. Comma-separated without brackets: daily, note → JSON array
            if v.contains(',') {
                let items: Vec<serde_json::Value> = v
                    .split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| serde_json::Value::String(s.to_string()))
                    .collect();
                map.insert(k.to_string(), serde_json::Value::Array(items));
                continue;
            }

            // 4. Plain string
            map.insert(k.to_string(), serde_json::Value::String(v.to_string()));
        }
    }
    Ok(map)
}

fn parse_template_vars(pairs: Option<Vec<String>>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for s in pairs.unwrap_or_default() {
        if let Some((k, v)) = s.split_once('=') {
            map.insert(k.to_string(), v.to_string());
        }
    }
    map
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::SetVaultPath { path } => {
            let config = Config {
                vault_path: path.clone(),
                daily_notes_path: None,
                weekly_notes_path: None,
                monthly_notes_path: None,
                templates_path: None,
            };
            save_config(&config)?;
            println!("Vault path set to: {}", path);
            return Ok(());
        }
        _ => {}
    }

    let config = load_config()?;
    let service = ObsidianService::new(
        &config.vault_path,
        config.daily_notes_path.as_deref(),
        config.weekly_notes_path.as_deref(),
        config.monthly_notes_path.as_deref(),
        config.templates_path.as_deref(),
    )?;

    match &cli.command {
        Commands::SetVaultPath { .. } => unreachable!(),

        Commands::ListNotesDirectory { path, limit, offset, recursive } => {
            let req = ListNotesDirectoryRequest {
                path: path.clone(),
                limit: *limit,
                offset: *offset,
                recursive: Some(*recursive),
            };
            let result = service.list_notes_directory_impl(req);
            print_json(&result)?;
        }

        Commands::ReadNotesFile { path } => {
            let req = ReadNotesFileRequest { path: path.clone() };
            let result = service.read_notes_file_impl(req);
            print_json(&result)?;
        }

        Commands::DeleteNotesItem { path } => {
            let req = DeleteNotesItemRequest { path: path.clone() };
            let result = service.delete_notes_item_impl(req);
            print_json(&result)?;
        }

        Commands::CreateOrUpdateNote { path, content, content_file, content_stdin, frontmatter, mode } => {
            let content = if *content_stdin {
                resolve_content("-")?
            } else if let Some(ref f) = content_file {
                resolve_content(&format!("@{}", f))?
            } else if let Some(ref c) = content {
                resolve_content(c)?
            } else {
                anyhow::bail!("One of --content, --content-file, or --content-stdin is required");
            };
            let frontmatter = frontmatter
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok());
            let req = CreateOrUpdateNoteRequest {
                path: path.clone(),
                content,
                frontmatter,
                mode: Some(mode.clone()),
            };
            let result = service.create_or_update_note_impl(req);
            print_json(&result)?;
        }

        Commands::GetDailyNote { date } => {
            let req = GetDailyNoteRequest { date: Some(date.clone()) };
            let result = service.get_daily_note_impl(req);
            print_json(&result)?;
        }

        Commands::SearchVault { query, scope, path_filter } => {
            let req = SearchVaultRequest {
                query: query.clone(),
                scope: scope.clone(),
                path_filter: path_filter.clone(),
            };
            let result = service.search_vault_impl(req);
            print_json(&result)?;
        }

        Commands::FindRelatedNotes { path, on } => {
            let req = FindRelatedNotesRequest {
                path: path.clone(),
                on: on.clone(),
            };
            let result = service.find_related_notes_impl(req);
            print_json(&result)?;
        }

        Commands::ReplaceTextInNote { path, find, replace, replace_all } => {
            let replace = resolve_content(replace)?;
            let req = ReplaceTextInNoteRequest {
                path: path.clone(),
                find: find.clone(),
                replace,
                replace_all: Some(*replace_all),
            };
            let result = service.replace_text_in_note_impl(req);
            print_json(&result)?;
        }

        Commands::AppendToSection { path, section_header, text } => {
            let text_to_append = resolve_content(&text)?;
            let req = AppendToSectionRequest {
                path: path.clone(),
                section_header: section_header.clone(),
                text_to_append,
            };
            let result = service.append_to_section_impl(req);
            print_json(&result)?;
        }

        Commands::UpdateNoteProperties { path, set, remove } => {
            let properties = set.as_ref().map(|s| parse_key_value_pairs(s.clone())).transpose()?;
            let req = UpdateNotePropertiesRequest {
                path: path.clone(),
                properties,
                remove: remove.clone(),
            };
            let result = service.update_note_properties_impl(req);
            print_json(&result)?;
        }

        Commands::GetNoteProperties { path } => {
            let req = GetNotePropertiesRequest { path: path.clone() };
            let result = service.get_note_properties_impl(req);
            print_json(&result)?;
        }

        Commands::CreateNoteFromTemplate { path, template_path, var } => {
            let variables = parse_template_vars(var.clone());
            let req = CreateNoteFromTemplateRequest {
                path: path.clone(),
                template_path: template_path.clone(),
                variables: if variables.is_empty() { None } else { Some(variables) },
            };
            let result = service.create_note_from_template_impl(req);
            print_json(&result)?;
        }

        Commands::ListNotesTemplates => {
            let result = service.list_notes_templates_impl();
            print_json(&result)?;
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
}
