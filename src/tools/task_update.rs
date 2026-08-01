//! Task update tool.
//!
//! Implements the `task_update` tool for changing a task's status/owner and
//! wiring up `blockedBy`/`blocks` dependency edges in the persistent,
//! SQLite-backed [`TaskRegistry`](super::task_registry::TaskRegistry).
//! Completing a task automatically unblocks any tasks that were waiting on it.

use super::task_registry::{TaskRegistry, TaskStatus, TaskUpdateRequest};
use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Tool that updates a task's status, owner, or dependencies.
pub struct TaskUpdateTool {
    registry: Arc<TaskRegistry>,
}

impl TaskUpdateTool {
    pub fn new(registry: Arc<TaskRegistry>) -> Self {
        Self { registry }
    }
}

fn string_array(args: &serde_json::Value, field: &str) -> Vec<String> {
    args.get(field)
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "task_update"
    }

    fn description(&self) -> &str {
        "Update a task's status, owner, or dependencies. 'add_blocked_by'/'add_blocks' \
         declare prerequisite/dependent tasks. Completing a task auto-unblocks its dependents."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["task_id"],
            "properties": {
                "task_id": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Task ID to update, e.g. 'task-1'"
                },
                "status": {
                    "type": "string",
                    "enum": ["pending", "in_progress", "completed"],
                    "description": "New status for the task"
                },
                "owner": {
                    "type": "string",
                    "description": "Agent/sub-agent name to assign this task to"
                },
                "add_blocked_by": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Task IDs that must complete before this task can start"
                },
                "add_blocks": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Task IDs that cannot start until this task completes"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let task_id = args
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .ok_or_else(|| anyhow::anyhow!("Missing 'task_id' parameter"))?;
        if task_id.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'task_id' parameter must not be empty".into()),
            });
        }

        let status = match args.get("status").and_then(|v| v.as_str()) {
            Some(raw) => match TaskStatus::parse(raw) {
                Some(status) => Some(status),
                None => {
                    return Ok(ToolResult {
                        success: false,
                        output: String::new(),
                        error: Some(format!(
                            "Invalid status '{raw}'. Must be one of: pending, in_progress, completed"
                        )),
                    });
                }
            },
            None => None,
        };

        let owner = args
            .get("owner")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let add_blocked_by = string_array(&args, "add_blocked_by");
        let add_blocks = string_array(&args, "add_blocks");

        if status.is_none() && owner.is_none() && add_blocked_by.is_empty() && add_blocks.is_empty()
        {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(
                    "At least one of 'status', 'owner', 'add_blocked_by', or 'add_blocks' must be provided"
                        .into(),
                ),
            });
        }

        let request = TaskUpdateRequest {
            status,
            owner,
            add_blocked_by,
            add_blocks,
        };

        match self.registry.update(task_id, request) {
            Ok(view) => Ok(ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&view)?,
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
    use crate::tools::task_create::TaskCreateTool;
    use tempfile::TempDir;

    fn make_registry() -> (TempDir, Arc<TaskRegistry>) {
        let tmp = TempDir::new().unwrap();
        let registry = Arc::new(TaskRegistry::new(tmp.path()));
        (tmp, registry)
    }

    async fn make_registry_with_task(subject: &str) -> (TempDir, Arc<TaskRegistry>, String) {
        let (tmp, registry) = make_registry();
        let create_tool = TaskCreateTool::new(registry.clone());
        let result = create_tool
            .execute(json!({"subject": subject, "description": "details"}))
            .await
            .unwrap();
        let view: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        let id = view["id"].as_str().unwrap().to_string();
        (tmp, registry, id)
    }

    #[test]
    fn name_and_schema() {
        let (_tmp, registry) = make_registry();
        let tool = TaskUpdateTool::new(registry);
        assert_eq!(tool.name(), "task_update");
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"], json!(["task_id"]));
    }

    #[tokio::test]
    async fn updates_status() {
        let (_tmp, registry, id) = make_registry_with_task("Task A").await;
        let tool = TaskUpdateTool::new(registry);

        let result = tool
            .execute(json!({"task_id": id, "status": "in_progress"}))
            .await
            .unwrap();
        assert!(result.success);
        let view: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(view["status"], "in_progress");
    }

    #[tokio::test]
    async fn updates_owner() {
        let (_tmp, registry, id) = make_registry_with_task("Task A").await;
        let tool = TaskUpdateTool::new(registry);

        let result = tool
            .execute(json!({"task_id": id, "owner": "researcher"}))
            .await
            .unwrap();
        assert!(result.success);
        let view: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(view["owner"], "researcher");
    }

    #[tokio::test]
    async fn adds_blocked_by_dependency() {
        let (_tmp, registry) = make_registry();
        let create_tool = TaskCreateTool::new(registry.clone());
        let a = create_tool
            .execute(json!({"subject": "A", "description": "a"}))
            .await
            .unwrap();
        let a_id = serde_json::from_str::<serde_json::Value>(&a.output).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        let b = create_tool
            .execute(json!({"subject": "B", "description": "b"}))
            .await
            .unwrap();
        let b_id = serde_json::from_str::<serde_json::Value>(&b.output).unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();

        let tool = TaskUpdateTool::new(registry);
        let result = tool
            .execute(json!({"task_id": a_id, "add_blocked_by": [b_id]}))
            .await
            .unwrap();
        assert!(result.success);
        let view: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(view["blocked"], true);
        assert_eq!(view["available"], false);
    }

    #[tokio::test]
    async fn invalid_status_returns_failed_result() {
        let (_tmp, registry, id) = make_registry_with_task("Task A").await;
        let tool = TaskUpdateTool::new(registry);

        let result = tool
            .execute(json!({"task_id": id, "status": "bogus"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Invalid status"));
    }

    #[tokio::test]
    async fn unknown_task_returns_failed_result() {
        let (_tmp, registry) = make_registry();
        let tool = TaskUpdateTool::new(registry);
        let result = tool
            .execute(json!({"task_id": "task-999", "status": "completed"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn missing_task_id_errors() {
        let (_tmp, registry) = make_registry();
        let tool = TaskUpdateTool::new(registry);
        let err = tool.execute(json!({"status": "completed"})).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn no_fields_provided_returns_failed_result() {
        let (_tmp, registry, id) = make_registry_with_task("Task A").await;
        let tool = TaskUpdateTool::new(registry);
        let result = tool.execute(json!({"task_id": id})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("must be provided"));
    }
}
