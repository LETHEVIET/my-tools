# Quick Launcher & Quick Tasks — My Tools v0.8.0

Both utilities run inside the existing **single `my-tools.exe`**.
They store data locally, without extra services, accounts, or AI models.

## Quick Launcher

Press **Ctrl+Shift+Space** anywhere in Windows. Type a tool name, saved-note
keyword, or task title; click **Open** or use **Up/Down** and **Enter** to
select a result. **Esc** closes the palette.

Results include the Quick Note and Prompt Notes editors, Quick Tasks, Saved
Notes, iPhone pairing, and the main dashboard. Saved notes are searched with
SQLite FTS5; task names are searched locally in SQLite. Selecting a note
opens that item in Saved Notes. Selecting a task opens the matching item in
Quick Tasks.

If another app already uses Ctrl+Shift+Space, Quick Launcher remains
accessible from the tray menu or the main dashboard.

## Quick Tasks

In the sidebar, open **Quick Tasks** and type a description. Optionally enter
a due time in `YYYY-MM-DD HH:MM` in **your Windows local timezone**, or
choose **Tomorrow 9 AM**. Leave it blank for an ordinary task without a
reminder. Select **Add task**.

The task views are **Today** (due today or overdue), **Upcoming** (later due
dates or no due date), **Completed**, and **All**. Use **Done** to mark a task
complete, **Reopen** to restore it, and **Set due** to apply the due date
currently in the input field to an unfinished task.

Tasks are saved in the existing
`%LOCALAPPDATA%\MyTools\my-tools.db` database.
All reminder timestamps are stored as UTC milliseconds and converted to
Windows local time when displayed.

### Windows reminders

The already-running My Tools tray process checks for due, unfinished,
unnotified tasks when it starts and then every **30 seconds**. It asks
Windows to display a tray notification; clicking the notification opens Quick
Tasks. Completing a task prevents further reminders. At most one reminder is
displayed per interval so simultaneous reminders don't immediately replace
each other.

**Reminders work only while My Tools is running** (being hidden to the tray is
fine). Enable **Start with Windows** in the tray menu for reminders after
sign-in. If the PC was shut down or sleeping, notifications are checked
again when My Tools is running. Windows Focus Assist / Do Not Disturb may
suppress banners, so these are best-effort reminders rather than guaranteed
alarms.

All task content stays in unencrypted, local SQLite. See
[NOTES_DATABASE.md](NOTES_DATABASE.md) for backup instructions.
