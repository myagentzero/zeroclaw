//! Task listing tool.
//!
//! Implements the `task_list` tool for enumerating tasks in the persistent,
//! SQLite-backed [`TaskRegistry`](super::task_registry::TaskRegistry),
//! optionally filtered by status and/or owner. Useful for finding "available"
//! work — tasks that are pending, unowned, and have no outstanding
//! `blockedBy` dependencies.

use super::task_registry::{TaskRegistry, TaskStatus};
use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Tool that lists tasks with their current status.
pub struct TaskListTool {
    registry: Arc<TaskRegistry>,
}

impl TaskListTool {
    pub fn new(registry: Arc<TaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "task_list"
    }

    fn description(&self) -> &str {
        "List all tasks with their current status, owner, and dependency \
         relationships. Optionally filter by status or owner. A task is \
         'available' when it is pending, unowned, and unblocked."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed", "all"],
                    "description": "Filter by status. Defaults to 'all'."
                },
                "owner": {
                    "type": "string",
                    "description": "Filter by assigned owner"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let status = match args.get("status").and_then(|v| v.as_str()).map(str::trim) {
            None | Some("") | Some("all") => None,
            Some(raw) => match TaskStatus::parse(raw) {
                Some(status) => Some(status),
                None => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "Invalid status '{raw}'. Must be one of: pending, in_progress, completed, all"
                        )),
                    });
                }
            },
        };

        let owner = args
            .get("owner")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        match self.registry.list(status, owner) {
            Ok(tasks) => Ok(ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&tasks)?,
                error: None,
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::task_registry::TaskUpdateRequest;
    use serde_json::Map;
    use tempfile::TempDir;

    fn make_registry() -> (TempDir, Arc<TaskRegistry>) {
        let tmp = TempDir::new().unwrap();
        let registry = Arc::new(TaskRegistry::new(tmp.path()));
        (tmp, registry)
    }

    #[test]
    fn name_and_schema() {
        let (_tmp, registry) = make_registry();
        let tool = TaskListTool::new(registry);
        assert_eq!(tool.name(), "task_list");
        let schema = tool.parameters_schema();
        assert!(schema["properties"]["status"].is_object());
        assert!(schema["properties"]["owner"].is_object());
    }

    #[tokio::test]
    async fn list_empty_registry() {
        let (_tmp, registry) = make_registry();
        let tool = TaskListTool::new(registry);
        let result = tool.execute(json!({})).await.unwrap();
        assert!(result.success);
        assert_eq!(result.output.trim(), "[]");
    }

    #[tokio::test]
    async fn lists_all_tasks_by_default() {
        let (_tmp, registry) = make_registry();
        registry
            .create("A".into(), "a".into(), None, Map::new())
            .unwrap();
        registry
            .create("B".into(), "b".into(), None, Map::new())
            .unwrap();

        let tool = TaskListTool::new(registry);
        let result = tool.execute(json!({})).await.unwrap();
        assert!(result.success);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result.output).unwrap();
        assert_eq!(parsed.len(), 2);
    }

    #[tokio::test]
    async fn filters_by_status() {
        let (_tmp, registry) = make_registry();
        let a = registry
            .create("A".into(), "a".into(), None, Map::new())
            .unwrap();
        registry
            .create("B".into(), "b".into(), None, Map::new())
            .unwrap();
        registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    status: Some(TaskStatus::Completed),
                    ..Default::default()
                },
            )
            .unwrap();

        let tool = TaskListTool::new(registry);
        let result = tool.execute(json!({"status": "completed"})).await.unwrap();
        assert!(result.success);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result.output).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["id"], a.id);
    }

    #[tokio::test]
    async fn filters_by_owner() {
        let (_tmp, registry) = make_registry();
        let a = registry
            .create("A".into(), "a".into(), None, Map::new())
            .unwrap();
        registry
            .create("B".into(), "b".into(), None, Map::new())
            .unwrap();
        registry
            .update(
                &a.id,
                TaskUpdateRequest {
                    owner: Some("agent-1".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        let tool = TaskListTool::new(registry);
        let result = tool.execute(json!({"owner": "agent-1"})).await.unwrap();
        assert!(result.success);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result.output).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["id"], a.id);
    }

    #[tokio::test]
    async fn invalid_status_filter_returns_failed_result() {
        let (_tmp, registry) = make_registry();
        let tool = TaskListTool::new(registry);
        let result = tool.execute(json!({"status": "bogus"})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Invalid status"));
    }

    #[tokio::test]
    async fn explicit_all_status_returns_everything() {
        let (_tmp, registry) = make_registry();
        registry
            .create("A".into(), "a".into(), None, Map::new())
            .unwrap();
        let tool = TaskListTool::new(registry);
        let result = tool.execute(json!({"status": "all"})).await.unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&result.output).unwrap();
        assert_eq!(parsed.len(), 1);
    }
}
