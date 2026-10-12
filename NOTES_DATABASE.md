# My Tools — SQLite local note storage

From **v0.7.0**, My Tools stores saved Quick Notes and saved Prompt Notes in
`%LOCALAPPDATA%\MyTools\my-tools.db`. It embeds **SQLite and FTS5** via
`rusqlite` with the bundled SQLite library. No database service, online
account, or extra app is required. The downloadable Windows app remains one
`my-tools.exe`.

## Safe first-run migration

On the first database opening, My Tools imports existing
`%LOCALAPPDATA%\MyTools\quick-notes\note-*.txt` files into the `notes`
table. The import runs inside one SQLite transaction; the completion marker is
committed only after all readable notes are imported. If importing a file
fails, the transaction rolls back, an error is reported, and the app can retry
on the next launch. Imported file paths are unique in the table to prevent
duplicate rows.

**Old text files are intentionally not deleted, moved, or rewritten.** They
remain available as a migration backup. For a second independent backup, copy
the whole `%LOCALAPPDATA%\MyTools` folder before upgrading. Unsaved Quick
Note and Prompt Notes scratchpad drafts retain their original filenames.

## What is stored

- Note ID, kind (`quick` or `prompt`), title, full body, and tags
- Pinned status and creation/last-edit timestamps
- The path of an imported legacy file, where applicable
- FTS5 index over title, body, and tags, maintained by SQLite triggers

The **Saved Notes** page supports keyword search using FTS5, editing the
selected note's title/body/tags, pinning notes, and refreshing the list.
Changing a note updates its searchable index. Keyword search is **not yet
semantic search**; installing an embedding model is a separate, optional
future feature.

The **Data folder** button opens the directory containing the SQLite
database. SQLite may also create `my-tools.db-wal` and `my-tools.db-shm`
while the app is running. For a consistent backup, close My Tools using
**Exit My Tools** in the system tray and then copy the directory.

## Privacy and limitations

SQLite notes are stored locally and **not encrypted**; Windows account
permissions protect access to the directory, but they are not equivalent to
database encryption. Avoid saving passwords or credentials as notes.
FTS5 provides full-text keyword search, not embeddings or multimodal indexing.
The one-time importer recognizes the legacy `note-*.txt` naming convention
and does not delete or synchronize those original files afterward.
