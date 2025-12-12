# MCP Server for Obsidian Notes

An MCP (Model Context Protocol) server that provides access to Obsidian notes vault without requiring Obsidian to be running. This server allows you to interact with your Obsidian vault directly through the filesystem, enabling read, write, search, and template operations.

## Features

- **File Operations**: List directories, read files, create/update notes, delete items
- **Daily Notes**: Get daily notes with automatic date handling (today, yesterday, tomorrow, YYYY-MM-DD)
- **Search**: Search across note content, filenames, and tags with flexible filtering
- **Note Relationships**: Find related notes based on shared tags or wikilinks
- **Text Editing**: Replace text in notes or append to specific markdown sections
- **Property Management**: Update Obsidian frontmatter properties (add, update, remove)
- **Template System**: Create notes from templates with variable substitution
- **Frontmatter Support**: Parse and manipulate YAML frontmatter in notes

## Tools

### Core File Operations

#### `list_notes_directory`
List files and directories in a vault directory. Returns both files and directories by default. When recursive=true, only returns .md files from subdirectories.

**Parameters:**
- `path` (string, optional, default: "."): Directory path relative to vault root (use '.' for root)
- `limit` (number, optional, default: 50): Maximum items to return
- `offset` (number, optional, default: 0): Pagination offset
- `recursive` (boolean, optional, default: false): If true, recursively search subdirectories and return only .md files. If false, returns immediate contents (files and directories)

**Returns:** Empty array if path doesn't exist

#### `read_notes_file`
Read a markdown note file from the vault. Returns content body (without frontmatter) and frontmatter separately as YAML metadata.

**Parameters:**
- `path` (string, required): Path to the note file relative to vault root. Can include or omit .md extension (auto-added if missing)

**Returns:** File content with parsed frontmatter (separated)

#### `delete_notes_item`
Delete a file or directory from the vault. Deletes directories recursively.

**Parameters:**
- `path` (string, required): Path to file or directory relative to vault root. For files, can include or omit .md extension. For directories, must not include .md extension

**Returns:** Error if path doesn't exist

#### `create_or_update_note`
Create a new note or update existing one. Path should NOT include .md extension (auto-added). Mode options: 'overwrite' (default) - replaces entire file, 'append' - adds content after existing body, 'prepend' - adds content before existing body. Frontmatter is merged (new keys added, existing keys updated). Creates parent directories if needed.

**Parameters:**
- `path` (string, required): Path for the note relative to vault root. Should NOT include .md extension (auto-added)
- `content` (string, required): Note content body (without frontmatter)
- `frontmatter` (object, optional): YAML frontmatter metadata. If note exists, frontmatter is merged (new keys added, existing keys updated)
- `mode` (string, optional, default: "overwrite"): Update mode - "overwrite" (replaces entire file), "append" (adds content after existing body), "prepend" (adds content before existing body)

#### `get_daily_note`
Get daily note for a date. Searches common locations: configured daily_notes_path, root, 'daily/', 'Daily Notes/'.

**Parameters:**
- `date` (string, optional, default: "today"): Date format: 'today', 'yesterday', 'tomorrow', or 'YYYY-MM-DD'

**Returns:** Error message in content field if note not found

### Advanced Search & Discovery

#### `search_vault`
Search for text in vault notes. Query is literal text (case-sensitive substring match). Can specify multiple scopes. Returns file paths and match previews.

**Parameters:**
- `query` (string, required): Search query - literal text (case-sensitive substring match)
- `scope` (array, optional, default: ["content", "filename"]): Search scope: array of 'content' (note body), 'filename' (file paths), 'tags' (frontmatter tags). Can specify multiple
- `path_filter` (string, optional): Limit search to specific subdirectory relative to vault root

#### `find_related_notes`
Find notes related to a source note. Extracts tags from source note's frontmatter and wikilinks [[...]] from content. Finds other notes that: (1) have matching tags in frontmatter, or (2) have filenames matching extracted link names.

**Parameters:**
- `path` (string, required): Path to the source note relative to vault root. Can include or omit .md extension (auto-added)
- `on` (array, optional, default: ["tags", "links"]): Relationship criteria: array of 'tags' and/or 'links'. Extracts tags from frontmatter and wikilinks [[...]] from content, then finds notes with matching tags or filenames

**Returns:** Empty array if source note not found

#### `replace_text_in_note`
Replace text in a note. Finds target text and replaces it with new content. Simple find and replace operation.

**Parameters:**
- `path` (string, required): Path to the note file relative to vault root. Can include or omit .md extension (auto-added)
- `find` (string, required): Literal text to find within the note
- `replace` (string, required): Replacement text. `\n` in string is converted to newline
- `replace_all` (boolean, optional, default: true): If true, replaces all occurrences. If false, replaces only the first occurrence

#### `append_to_section`
Append text to a specific markdown section. Finds the section header and appends content before the next header of the same or higher level (or at end of file).

**Parameters:**
- `path` (string, required): Path to the note file relative to vault root. Can include or omit .md extension (auto-added)
- `section_header` (string, required): Section header with # markers (e.g., '## End day'). Must include # to specify level. Whitespace is normalized. Must match exactly (level and text)
- `text_to_append` (string, required): Text to append to the section. `\n` in string is converted to newline. Newline is automatically added before this text

**Errors:**
- Returns error if header level not specified (no # markers)
- Returns error if section not found
- Returns error if header level mismatch (e.g., looking for `# End day` but only `## End day` exists)
- Returns error if multiple sections match (suggests using `replace_text_in_note` for precise targeting)

#### `update_note_properties`
Update frontmatter properties (Obsidian properties) in a note. Updates/adds properties and removes specified properties. Does not modify note content body. Creates frontmatter if it doesn't exist.

**Parameters:**
- `path` (string, required): Path to the note file relative to vault root. Can include or omit .md extension (auto-added)
- `properties` (object, optional): Properties to update or add. Existing properties with same key are overwritten. New properties are added. Values can be strings, numbers, booleans, or arrays
- `remove` (array of strings, optional): Property keys to remove from frontmatter

**Examples:**
- Update status: `{"properties": {"status": "done"}}`
- Add multiple properties: `{"properties": {"priority": "high", "due-date": "2024-01-15", "completed": true}}`
- Remove property: `{"remove": ["old-tag", "deprecated-field"]}`
- Update and remove: `{"properties": {"status": "archived"}, "remove": ["active"]}`

### Template System

#### `create_note_from_template`
Create note from template with variable substitution. Template path: if starts with '/' or contains ':', treated as absolute path relative to vault root; otherwise relative to templates directory (paths from list_notes_templates can be used directly).

**Parameters:**
- `path` (string, required): Destination path for the new note relative to vault root. SHOULD include .md extension
- `template_path` (string, required): Path to the template file. If starts with '/' or contains ':', treated as absolute path; otherwise relative to templates directory
- `variables` (object, optional): Key-value pairs for template substitution. Replaces {{variable}} placeholders in template

#### `list_notes_templates`
List all .md template files in templates directory. Returns paths relative to templates directory (can be used directly with create_note_from_template).

**Parameters:** None

**Returns:** Template file paths, names, and sizes. Returns empty array if templates directory doesn't exist

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
