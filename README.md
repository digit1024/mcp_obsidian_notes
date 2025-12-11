# MCP Server for Obsidian Notes

An MCP (Model Context Protocol) server that provides access to Obsidian notes vault without requiring Obsidian to be running. This server allows you to interact with your Obsidian vault directly through the filesystem, enabling read, write, search, and template operations.

## Features

- **File Operations**: List directories, read files, create/update notes, delete items
- **Daily Notes**: Get daily notes with automatic date handling (today, yesterday, tomorrow, YYYY-MM-DD)
- **Advanced Search**: Search across note content, filenames, and tags
- **Note Relationships**: Find related notes based on shared tags or wikilinks
- **Text Editing**: Insert or replace text at specific locations within notes
- **Template System**: Create notes from templates with variable substitution
- **Frontmatter Support**: Parse and manipulate YAML frontmatter in notes

## Tools

### Core File Operations

#### `list_notes_directory`
List notes directory contents with pagination to prevent context overflow. Shows immediate contents by default.

**Parameters:**
- `path` (string, optional, default: "."): Directory path to list
- `limit` (number, optional, default: 50): Maximum items to return
- `offset` (number, optional): Pagination offset
- `recursive` (boolean, optional): Include subdirectories recursively

#### `read_notes_file`
Read content of a specific file from the vault.

**Parameters:**
- `path` (string, required): Path to the file

**Returns:** File content with parsed frontmatter (if present)

#### `delete_notes_item`
Delete a file or directory from the vault.

**Parameters:**
- `path` (string, required): Path to the item to delete

#### `create_or_update_note`
Create or update a note with content and frontmatter. Performs upsert operation - creates if doesn't exist, updates if it does with different modes: overwrite (default), append, or prepend.

**Parameters:**
- `path` (string, required): Path for the note (without .md extension)
- `content` (string, required): Note content
- `frontmatter` (object, optional): Frontmatter metadata
- `mode` (string, optional): Update mode - "overwrite", "append", or "prepend" (default: "overwrite")

#### `get_daily_note`
Get daily note for a specific date. Handles common daily note naming conventions and file locations.

**Parameters:**
- `date` (string, optional, default: "today"): Date (today, yesterday, tomorrow, or YYYY-MM-DD)

### Advanced Search & Discovery

#### `search_vault`
Search notes vault content across files, filenames, and metadata with advanced filtering.

**Parameters:**
- `query` (string, required): Search query
- `scope` (array, optional): Search scope - where to look for the query (options: "content", "filename", "tags", default: ["content", "filename"])
- `path_filter` (string, optional): Limit search to specific path prefix

#### `find_related_notes`
Find notes related to a given note based on shared tags, links, or backlinks.

**Parameters:**
- `path` (string, required): Path to the source note
- `on` (array, optional): Relationship criteria to use for finding related notes (options: "tags", "links", default: ["tags", "links"])

#### `edit_note_text`
Insert or replace text at specific locations within a note. Handles the common "find and modify" pattern without requiring manual text manipulation or position calculations. Automatically manages newlines and formatting. Returns error if pattern not found.

**Parameters:**
- `path` (string, required): Path to the note file
- `operation` (string, required): Type of edit to perform - "insert_after", "insert_before", or "replace"
- `target` (string, required): Text to find within the note
- `content` (string, required): Content to add or replace with
- `in_new_line` (boolean, optional, default: true): Whether to add content on a new line (ignored for 'replace' operation)

### Template System

#### `create_note_from_template`
Create a new note by applying a template with simple variable substitution. Replaces `{{variable}}` placeholders in the template with provided values. Perfect for creating structured notes from predefined templates without manual copying and editing.

**Parameters:**
- `path` (string, required): Destination path for the new note (including .md extension)
- `template_path` (string, required): Path to the template file
- `variables` (object, optional): Key-value pairs for template substitution (e.g., `{"date": "2024-01-15", "title": "Meeting Notes"}`)

#### `list_notes_templates`
List all available Notes templates in the templates directory with their paths and basic metadata. Helps discover existing templates before creating notes from them.

**Parameters:** None

## Building

```bash
cargo build --release
```

## Running

The server communicates via stdio (standard input/output) and requires the `VAULT_LOCATION` environment variable to be set:

```bash
export VAULT_LOCATION="/path/to/your/obsidian/vault"
./target/release/mcp_obsidian_notes
```

Or using cargo:

```bash
export VAULT_LOCATION="/path/to/your/obsidian/vault"
cargo run --release
```

## Environment Variables

**Required:**
- `VAULT_LOCATION`: Root path to your Obsidian vault directory

**Optional:**
- `DAILY_NOTES_PATH`: Path to daily notes folder (relative to vault root, default: "daily" or "Daily Notes")
- `WEEKLY_NOTES_PATH`: Path to weekly notes folder (relative to vault root)
- `MONTHLY_NOTES_PATH`: Path to monthly notes folder (relative to vault root)
- `TEMPLATES_PATH`: Path to templates folder (relative to vault root, default: "templates" or "Templates")

**Example:**
```bash
export VAULT_LOCATION="/home/user/Documents/MyVault"
export DAILY_NOTES_PATH="Daily Notes"
export TEMPLATES_PATH="Templates"
./target/release/mcp_obsidian_notes
```

## MCP Client Configuration

To use this server with an MCP client, configure it to run this binary with stdio transport.

Example configuration (for Claude Desktop or similar):

```json
{
  "mcpServers": {
    "obsidian-notes": {
      "command": "/path/to/mcp_obsidian_notes/target/release/mcp_obsidian_notes",
      "args": [],
      "env": {
        "VAULT_LOCATION": "/path/to/your/obsidian/vault",
        "DAILY_NOTES_PATH": "Daily Notes",
        "TEMPLATES_PATH": "Templates"
      }
    }
  }
}
```

## Vault Structure

The server works with standard Obsidian vault structures:

```
vault/
├── Daily Notes/
│   ├── 2024-01-15.md
│   └── 2024-01-16.md
├── Templates/
│   ├── Meeting Template.md
│   └── Note Template.md
├── Project A/
│   └── notes.md
└── other-notes.md
```

## Frontmatter Support

Notes with YAML frontmatter are fully supported:

```markdown
---
title: My Note
tags: [work, important]
created: 2024-01-15
---

Note content here...
```

The server can read, parse, and update frontmatter while preserving note content.

## Security

All file operations are validated to ensure they remain within the vault root directory, preventing directory traversal attacks. Paths are canonicalized and checked before any file operations.

## License

MIT
