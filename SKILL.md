---
name: obsidian-notes-cli
description: Manage Obsidian vault notes via the `obsidian-cli` (create, read, update, delete, search, find related notes, append to sections, manage frontmatter, use templates). Works without Obsidian running. Use when a user asks to add a note, list notes, search notes, manage daily notes, or work with an Obsidian vault.
homepage: https://github.com/digit1024/mcp_obsidian_notes
metadata: {"clawdbot":{"emoji":"📓","os":["linux","darwin"],"requires":{"bins":["obsidian-cli"]},"install":[{"id":"source","kind":"source","label":"Build: cargo build --release --bin obsidian-cli (from project root)"}]}}
---

# Obsidian Notes CLI

Use `obsidian-cli` to manage Obsidian vault notes from the terminal. Create, read, update, delete, search, find related notes, append to sections, manage frontmatter, and create notes from templates. Works directly on markdown files—no Obsidian app required.



## View Notes

- List directory: `obsidian-cli list-notes-directory`
- List with path: `obsidian-cli list-notes-directory --path "daily"`
- Recursive (only .md): `obsidian-cli list-notes-directory --recursive`
- Read note: `obsidian-cli read-notes-file "path/to/note"`
- Get daily note: `obsidian-cli get-daily-note --date today`
- Daily note by date: `obsidian-cli get-daily-note --date 2025-01-31`

## Create Notes

- Create/overwrite: `obsidian-cli create-or-update-note path/to/note --content "Note body"`
- Multiline (use `\n`): `obsidian-cli create-or-update-note my-note --content "line1\nline2"`
- From file: `obsidian-cli create-or-update-note my-note --content-file draft.md`
- From stdin: `echo "content" | obsidian-cli create-or-update-note my-note --content-stdin`
- Append mode: `obsidian-cli create-or-update-note my-note --content "more" --mode append`
- With frontmatter: `obsidian-cli create-or-update-note my-note --content "body" --frontmatter '{"tags":["work"]}'`

## Search & Discover

- Search vault: `obsidian-cli search-vault "query"`
- Search in path: `obsidian-cli search-vault "query" --path-filter "Daily Notes"`
- Search tags only: `obsidian-cli search-vault "tag" --scope tags`
- Find related: `obsidian-cli find-related-notes "path/to/source-note"`
- Related by links only: `obsidian-cli find-related-notes "note" --on links`

## Edit Notes

- Replace text: `obsidian-cli replace-text-in-note "path" --find "old" --replace "new"`
- Append to section: `obsidian-cli append-to-section "path" --section-header "## End day" --text "Item added"`
- Update properties: `obsidian-cli update-note-properties "path" --set tags='["a","b"]'`
- Remove property: `obsidian-cli update-note-properties "path" --remove old-tag`

## Properties & Templates

- Get frontmatter: `obsidian-cli get-note-properties "path"`
- List templates: `obsidian-cli list-notes-templates`
- Create from template: `obsidian-cli create-note-from-template "output.md" "Meeting.md" --var title="Meeting 1"`

## Delete

- Delete file: `obsidian-cli delete-notes-item "path/to/note"`
- Delete directory: `obsidian-cli delete-notes-item "path/to/folder"` (recursive)

## Multiline & Content Sources

For `--content`, `--find`, `--replace`, `--text`:
- Literal with newlines: use `\n` in the string
- From file: prefix with `@` (e.g. `--content "@draft.md"`)
- From stdin: use `-` (e.g. `--content-stdin` or `--replace "-"` with pipe)

## Limitations

- Requires vault path to be set via `set-vault-path` before other commands.
- All paths are relative to vault root.
- Template variables use `{{name}}` syntax; templates support date expressions like `{{date:YYYY-MM-DD}}`.

## Notes

- Cross-platform (Linux, macOS).
- Output is JSON; parse with `jq` if needed.
- Paths can omit `.md` for note files (auto-added).

