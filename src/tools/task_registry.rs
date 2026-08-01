//! SQLite-backed task orchestration store.
//!
//! Mirrors the CLI Task system (`TaskCreate`/`TaskUpdate`/`TaskGet`/`TaskList`):
//! tasks are lightweight units of work with a subject/description, a status
//! (`pending` -> `in_progress` -> `completed`), an optional owner (assigned
//! sub-agent), and `blocked_by`/`blocks` dependency edges. Completing a task
//! automatically removes it from the `blocked_by` list of any tasks it blocks.
//!
//! Unlike the CLI's session-scoped, in-memory tasks, tasks here persist in a
//! per-workspace SQLite database (`<workspace_dir>/tasks/tasks.db`) so they
//! survive process restarts, following the same `with_connection`-per-call
//! pattern as [`crate::cron::store`].

use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// Status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(TaskStatus::Pending),
            "in_progress" => Some(TaskStatus::InProgress),
            "completed" => Some(TaskStatus::Completed),
            _ => None,
        }
    }
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Serializable view of a task returned by `task_get` / `task_list`.
#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub id: String,
    pub subject: String,
    pub description: String,
    pub active_form: Option<String>,
    pub status: String,
    pub owner: Option<String>,
    pub blocked_by: Vec<String>,
    pub blocks: Vec<String>,
    pub metadata: Map<String, Value>,
    pub blocked: bool,
    pub available: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Fields that may be changed by a `task_update` call.
#[derive(Debug, Default, Clone)]
pub struct TaskUpdateRequest {
    pub status: Option<TaskStatus>,
    pub owner: Option<String>,
    pub add_blocked_by: Vec<String>,
    pub add_blocks: Vec<String>,
}

/// Persistent, SQLite-backed store of tasks for orchestration.
#[derive(Debug, Clone)]
pub struct TaskRegistry {
    db_path: PathBuf,
}

impl TaskRegistry {
    /// Create a registry backed by `<workspace_dir>/tasks/tasks.db`.
    pub fn new(workspace_dir: &Path) -> Self {
        Self {
            db_path: workspace_dir.join("tasks").join("tasks.db"),
        }
    }

    /// Create a new task and return its view.
    pub fn create(
        &self,
        subject: String,
        description: String,
        active_form: Option<String>,
        metadata: Map<String, Value>,
    ) -> Result<TaskView> {
        let metadata_json = serde_json::to_string(&metadata)?;
        let now = Utc::now().to_rfc3339();

        let id = with_connection(&self.db_path, |conn| {
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "INSERT INTO tasks
                    (id, subject, description, active_form, status, owner, blocked_by, blocks, metadata, created_at, updated_at)
                 VALUES (NULL, ?1, ?2, ?3, 'pending', NULL, '[]', '[]', ?4, ?5, ?5)",
                params![subject, description, active_form, metadata_json, now],
            )
            .context("Failed to insert task")?;
            let seq = tx.last_insert_rowid();
            let id = format!("task-{seq}");
            tx.execute("UPDATE tasks SET id = ?1 WHERE seq = ?2", params![id, seq])
                .context("Failed to assign task id")?;
            tx.commit().context("Failed to commit task creation")?;
            Ok(id)
        })?;

        self.get(&id)?
            .context("Task disappeared immediately after creation")
    }

    /// Retrieve a single task's view.
    pub fn get(&self, id: &str) -> Result<Option<TaskView>> {
        with_connection(&self.db_path, |conn| {
            conn.query_row(
                "SELECT id, subject, description, active_form, status, owner, blocked_by, blocks, metadata, created_at, updated_at
                 FROM tasks WHERE id = ?1",
                params![id],
                map_task_row,
            )
            .optional()
            .context("Failed to query task")
        })
    }

    /// List all tasks, optionally filtered by status and/or owner.
    pub fn list(&self, status: Option<TaskStatus>, owner: Option<&str>) -> Result<Vec<TaskView>> {
        let tasks = with_connection(&self.db_path, |conn| {
            let mut stmt = conn.prepare(
                "SELECT id, subject, description, active_form, status, owner, blocked_by, blocks, metadata, created_at, updated_at
                 FROM tasks ORDER BY seq ASC",
            )?;
            let rows = stmt.query_map([], map_task_row)?;
            let mut tasks = Vec::new();
            for row in rows {
                tasks.push(row?);
            }
            Ok(tasks)
        })?;

        Ok(tasks
            .into_iter()
            .filter(|t| status.is_none_or(|s| t.status == s.as_str()))
            .filter(|t| owner.is_none_or(|o| t.owner.as_deref() == Some(o)))
            .collect())
    }

    /// Apply an update to a task, returning the updated view.
    ///
    /// Dependency edges are kept symmetric: adding `A.blocked_by = [B]` also
    /// adds `A` to `B.blocks`, and vice versa. Marking a task `completed`
    /// automatically removes it from the `blocked_by` list of every task it
    /// blocks.
    pub fn update(&self, id: &str, request: TaskUpdateRequest) -> Result<TaskView> {
        with_connection(&self.db_path, |conn| {
            let tx = conn.unchecked_transaction()?;

            if !row_exists(&tx, id)? {
                anyhow::bail!("Task '{id}' not found");
            }

            for dep_id in request
                .add_blocked_by
                .iter()
                .chain(request.add_blocks.iter())
            {
                if dep_id == id {
                    anyhow::bail!("Task '{id}' cannot depend on itself");
                }
                if !row_exists(&tx, dep_id)? {
                    anyhow::bail!("Referenced task '{dep_id}' not found");
                }
            }

            for blocker_id in &request.add_blocked_by {
                add_to_json_array(&tx, id, "blocked_by", blocker_id)?;
                add_to_json_array(&tx, blocker_id, "blocks", id)?;
            }

            for blocked_id in &request.add_blocks {
                add_to_json_array(&tx, id, "blocks", blocked_id)?;
                add_to_json_array(&tx, blocked_id, "blocked_by", id)?;
            }

            if let Some(owner) = &request.owner {
                tx.execute(
                    "UPDATE tasks SET owner = ?1 WHERE id = ?2",
                    params![owner, id],
                )
                .context("Failed to update task owner")?;
            }

            if let Some(status) = request.status {
                tx.execute(
                    "UPDATE tasks SET status = ?1 WHERE id = ?2",
                    params![status.as_str(), id],
                )
                .context("Failed to update task status")?;
            }

            touch_updated_at(&tx, id)?;

            // Auto-unblock: if this task just became completed, remove it
            // from the blocked_by list of every task it blocks.
            if request.status == Some(TaskStatus::Completed) {
                let blocks = read_json_array(&tx, id, "blocks")?;
                for blocked_id in blocks {
                    remove_from_json_array(&tx, &blocked_id, "blocked_by", id)?;
                    touch_updated_at(&tx, &blocked_id)?;
                }
            }

            tx.commit().context("Failed to commit task update")?;
            Ok(())
        })?;

        self.get(id)?
            .context("Task disappeared immediately after update")
    }

    /// Delete a task, scrubbing it from the `blocked_by`/`blocks` lists of
    /// any other tasks that referenced it so no dangling dependency remains.
    pub fn delete(&self, id: &str) -> Result<()> {
        with_connection(&self.db_path, |conn| {
            let tx = conn.unchecked_transaction()?;

            let deleted = tx
                .execute("DELETE FROM tasks WHERE id = ?1", params![id])
                .context("Failed to delete task")?;
            if deleted == 0 {
                anyhow::bail!("Task '{id}' not found");
            }

            let mut stmt = tx.prepare("SELECT id, blocked_by, blocks FROM tasks")?;
            let others: Vec<(String, String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
                .collect::<rusqlite::Result<_>>()
                .context("Failed to scan tasks for dangling references")?;
            drop(stmt);

            for (other_id, blocked_by_raw, blocks_raw) in others {
                let mut blocked_by: Vec<String> =
                    serde_json::from_str(&blocked_by_raw).unwrap_or_default();
                let mut blocks: Vec<String> = serde_json::from_str(&blocks_raw).unwrap_or_default();
                let before = (blocked_by.len(), blocks.len());
                blocked_by.retain(|v| v != id);
                blocks.retain(|v| v != id);
                if (blocked_by.len(), blocks.len()) != before {
                    tx.execute(
                        "UPDATE tasks SET blocked_by = ?1, blocks = ?2, updated_at = ?3 WHERE id = ?4",
                        params![
                            serde_json::to_string(&blocked_by)?,
                            serde_json::to_string(&blocks)?,
                            Utc::now().to_rfc3339(),
                            other_id,
                        ],
                    )
                    .with_context(|| format!("Failed to scrub reference to '{id}' from '{other_id}'"))?;
                }
            }

            tx.commit().context("Failed to commit task deletion")?;
            Ok(())
        })
    }
}

fn row_exists(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn
        .query_row("SELECT 1 FROM tasks WHERE id = ?1", params![id], |_| Ok(()))
        .optional()
        .context("Failed to check task existence")?
        .is_some())
}

fn read_json_array(conn: &Connection, id: &str, column: &str) -> Result<Vec<String>> {
    let raw: String = conn
        .query_row(
            &format!("SELECT {column} FROM tasks WHERE id = ?1"),
            params![id],
            |row| row.get(0),
        )
        .with_context(|| format!("Failed to read tasks.{column} for '{id}'"))?;
    serde_json::from_str(&raw).with_context(|| format!("Corrupt tasks.{column} JSON for '{id}'"))
}

fn write_json_array(conn: &Connection, id: &str, column: &str, values: &[String]) -> Result<()> {
    let json = serde_json::to_string(values)?;
    conn.execute(
        &format!("UPDATE tasks SET {column} = ?1 WHERE id = ?2"),
        params![json, id],
    )
    .with_context(|| format!("Failed to write tasks.{column} for '{id}'"))?;
    Ok(())
}

fn add_to_json_array(conn: &Connection, id: &str, column: &str, value: &str) -> Result<()> {
    let mut values = read_json_array(conn, id, column)?;
    if !values.iter().any(|v| v == value) {
        values.push(value.to_string());
        write_json_array(conn, id, column, &values)?;
    }
    Ok(())
}

fn remove_from_json_array(conn: &Connection, id: &str, column: &str, value: &str) -> Result<()> {
    let mut values = read_json_array(conn, id, column)?;
    let before = values.len();
    values.retain(|v| v != value);
    if values.len() != before {
        write_json_array(conn, id, column, &values)?;
    }
    Ok(())
}

fn touch_updated_at(conn: &Connection, id: &str) -> Result<()> {
    conn.execute(
        "UPDATE tasks SET updated_at = ?1 WHERE id = ?2",
        params![Utc::now().to_rfc3339(), id],
    )
    .context("Failed to update tasks.updated_at")?;
    Ok(())
}

fn map_task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskView> {
    let status_raw: String = row.get(4)?;
    let owner: Option<String> = row.get(5)?;
    let blocked_by_raw: String = row.get(6)?;
    let blocks_raw: String = row.get(7)?;
    let metadata_raw: String = row.get(8)?;

    let blocked_by: Vec<String> = serde_json::from_str(&blocked_by_raw).unwrap_or_default();
    let blocks: Vec<String> = serde_json::from_str(&blocks_raw).unwrap_or_default();
    let metadata: Map<String, Value> = serde_json::from_str(&metadata_raw).unwrap_or_default();

    Ok(TaskView {
        id: row.get(0)?,
        subject: row.get(1)?,
        description: row.get(2)?,
        active_form: row.get(3)?,
        available: status_raw == TaskStatus::Pending.as_str()
            && owner.is_none()
            && blocked_by.is_empty(),
        blocked: !blocked_by.is_empty(),
        status: status_raw,
        owner,
        blocked_by,
        blocks,
        metadata,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn with_connection<T>(db_path: &Path, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create tasks directory: {}", parent.display()))?;
    }

    let conn = Connection::open(db_path)
        .with_context(|| format!("Failed to open tasks DB: {}", db_path.display()))?;

    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous  = NORMAL;
         CREATE TABLE IF NOT EXISTS tasks (
            seq          INTEGER PRIMARY KEY AUTOINCREMENT,
            id           TEXT UNIQUE,
            subject      TEXT NOT NULL,
            description  TEXT NOT NULL,
            active_form  TEXT,
            status       TEXT NOT NULL DEFAULT 'pending',
            owner        TEXT,
            blocked_by   TEXT NOT NULL DEFAULT '[]',
            blocks       TEXT NOT NULL DEFAULT '[]',
            metadata     TEXT NOT NULL DEFAULT '{}',
            created_at   TEXT NOT NULL,
            updated_at   TEXT NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
         CREATE INDEX IF NOT EXISTS idx_tasks_owner ON tasks(owner);",
    )
    .context("Failed to initialize tasks schema")?;

    f(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn test_registry() -> (TempDir, TaskRegistry) {
        let tmp = TempDir::new().unwrap();
        let registry = TaskRegistry::new(tmp.path());
        (tmp, registry)
    }

    #[test]
    fn create_assigns_sequential_ids_and_pending_status() {
        let (_tmp, registry) = test_registry();
        let t1 = registry
            .create("Do A".into(), "Details A".into(), None, Map::new())
            .unwrap();
        let t2 = registry
            .create("Do B".into(), "Details B".into(), None, Map::new())
            .unwrap();

        assert_eq!(t1.id, "task-1");
        assert_eq!(t2.id, "task-2");
        assert_eq!(t1.status, "pending");
        assert!(t1.available);
        assert!(t1.blocked_by.is_empty());
    }

    #[test]
    fn create_preserves_active_form_and_metadata() {
        let (_tmp, registry) = test_registry();
        let mut metadata = Map::new();
        metadata.insert("feature".into(), json!("auth"));
        let view = registry
            .create(
                "Implement JWT".into(),
                "Add JWT validation".into(),
                Some("Implementing JWT".into()),
                metadata.clone(),
            )
            .unwrap();

        assert_eq!(view.active_form.as_deref(), Some("Implementing JWT"));
        assert_eq!(view.metadata, metadata);
    }

    #[test]
    fn get_returns_none_for_unknown_id() {
        let (_tmp, registry) = test_registry();
        assert!(registry.get("task-999").unwrap().is_none());
    }

    #[test]
    fn tasks_persist_across_registry_instances_for_same_workspace() {
        let tmp = TempDir::new().unwrap();
        let created = {
            let registry = TaskRegistry::new(tmp.path());
            registry
                .create("Persisted task".into(), "details".into(), None, Map::new())
                .unwrap()
        };

        // Simulate a process restart: a brand new TaskRegistry pointed at the
        // same workspace directory should see the previously created task.
        let reopened = TaskRegistry::new(tmp.path());
        let fetched = reopened.get(&created.id).unwrap().unwrap();
        assert_eq!(fetched.subject, "Persisted task");

        let listed = reopened.list(None, None).unwrap();
        assert_eq!(listed.len(), 1);
    }

    #[test]
    fn update_sets_status_and_owner() {
        let (_tmp, registry) = test_registry();
        let t1 = registry
            .create("Do A".into(), "Details".into(), None, Map::new())
            .unwrap();

        let updated = registry
            .update(
                &t1.id,
                TaskUpdateRequest {
                    status: Some(TaskStatus::InProgress),
                    owner: Some("researcher".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.status, "in_progress");
        assert_eq!(updated.owner.as_deref(), Some("researcher"));
        assert!(!updated.available);
    }

    #[test]
    fn update_unknown_task_errors() {
        let (_tmp, registry) = test_registry();
        let err = registry.update("task-missing", TaskUpdateRequest::default());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn add_blocked_by_creates_symmetric_blocks_edge() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();
        let b = registry
            .create("Task B".into(), "..".into(), None, Map::new())
            .unwrap();

        let updated_a = registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    add_blocked_by: vec![b.id.clone()],
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated_a.blocked_by, vec![b.id.clone()]);
        assert!(!updated_a.available);

        let view_b = registry.get(&b.id).unwrap().unwrap();
        assert_eq!(view_b.blocks, vec![a.id.clone()]);
    }

    #[test]
    fn add_blocks_creates_symmetric_blocked_by_edge() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();
        let b = registry
            .create("Task B".into(), "..".into(), None, Map::new())
            .unwrap();

        registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    add_blocks: vec![b.id.clone()],
                    ..Default::default()
                },
            )
            .unwrap();

        let view_a = registry.get(&a.id).unwrap().unwrap();
        let view_b = registry.get(&b.id).unwrap().unwrap();
        assert_eq!(view_a.blocks, vec![b.id.clone()]);
        assert_eq!(view_b.blocked_by, vec![a.id.clone()]);
        assert!(!view_b.available);
    }

    #[test]
    fn self_dependency_is_rejected() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();

        let err = registry.update(
            &a.id,
            TaskUpdateRequest {
                add_blocked_by: vec![a.id.clone()],
                ..Default::default()
            },
        );
        assert!(
            err.unwrap_err()
                .to_string()
                .contains("cannot depend on itself")
        );
    }

    #[test]
    fn unknown_dependency_is_rejected() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();

        let err = registry.update(
            &a.id,
            TaskUpdateRequest {
                add_blocked_by: vec!["task-999".into()],
                ..Default::default()
            },
        );
        assert!(err.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn completing_task_auto_unblocks_dependents() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();
        let b = registry
            .create("Task B".into(), "..".into(), None, Map::new())
            .unwrap();

        registry
            .update(
                &b.id,
                TaskUpdateRequest {
                    add_blocked_by: vec![a.id.clone()],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(!registry.get(&b.id).unwrap().unwrap().available);

        registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    status: Some(TaskStatus::Completed),
                    ..Default::default()
                },
            )
            .unwrap();

        let view_b = registry.get(&b.id).unwrap().unwrap();
        assert!(view_b.blocked_by.is_empty());
        assert!(view_b.available);
    }

    #[test]
    fn list_filters_by_status_and_owner() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();
        let b = registry
            .create("Task B".into(), "..".into(), None, Map::new())
            .unwrap();
        registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    status: Some(TaskStatus::InProgress),
                    owner: Some("agent-1".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        let in_progress = registry.list(Some(TaskStatus::InProgress), None).unwrap();
        assert_eq!(in_progress.len(), 1);
        assert_eq!(in_progress[0].id, a.id);

        let owned = registry.list(None, Some("agent-1")).unwrap();
        assert_eq!(owned.len(), 1);
        assert_eq!(owned[0].id, a.id);

        let all = registry.list(None, None).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, a.id);
        assert_eq!(all[1].id, b.id);
    }

    #[test]
    fn delete_removes_task_and_scrubs_dangling_references() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();
        let b = registry
            .create("Task B".into(), "..".into(), None, Map::new())
            .unwrap();
        registry
            .update(
                &b.id,
                TaskUpdateRequest {
                    add_blocked_by: vec![a.id.clone()],
                    ..Default::default()
                },
            )
            .unwrap();

        registry.delete(&a.id).unwrap();

        assert!(registry.get(&a.id).unwrap().is_none());
        let view_b = registry.get(&b.id).unwrap().unwrap();
        assert!(view_b.blocked_by.is_empty());
        assert!(view_b.available);

        let all = registry.list(None, None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, b.id);
    }

    #[test]
    fn delete_unknown_task_errors() {
        let (_tmp, registry) = test_registry();
        let err = registry.delete("task-999");
        assert!(err.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn status_parse_and_display() {
        assert_eq!(TaskStatus::parse("pending"), Some(TaskStatus::Pending));
        assert_eq!(
            TaskStatus::parse("in_progress"),
            Some(TaskStatus::InProgress)
        );
        assert_eq!(TaskStatus::parse("completed"), Some(TaskStatus::Completed));
        assert_eq!(TaskStatus::parse("bogus"), None);
        assert_eq!(format!("{}", TaskStatus::Pending), "pending");
    }

    #[test]
    fn registry_starts_empty() {
        let (_tmp, registry) = test_registry();
        assert!(registry.list(None, None).unwrap().is_empty());
    }

    #[test]
    fn adding_duplicate_dependency_is_idempotent() {
        let (_tmp, registry) = test_registry();
        let a = registry
            .create("Task A".into(), "..".into(), None, Map::new())
            .unwrap();
        let b = registry
            .create("Task B".into(), "..".into(), None, Map::new())
            .unwrap();

        registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    add_blocked_by: vec![b.id.clone()],
                    ..Default::default()
                },
            )
            .unwrap();
        let updated = registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    add_blocked_by: vec![b.id.clone()],
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(updated.blocked_by, vec![b.id.clone()]);
        assert_eq!(
            registry.get(&b.id).unwrap().unwrap().blocks,
            vec![a.id.clone()]
        );
    }
}
