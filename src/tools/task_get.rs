//! Task retrieval tool.
//!
//! Implements the `task_get` tool for fetching the full details of a single
//! task from the persistent, SQLite-backed [`TaskRegistry`](super::task_registry::TaskRegistry).

use super::task_registry::TaskRegistry;
use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;

/// Tool that retrieves full details for a single task.
pub struct TaskGetTool {
    registry: Arc<TaskRegistry>,
}

impl TaskGetTool {
    pub fn new(registry: Arc<TaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "task_get"
    }

    fn description(&self) -> &str {
        "Retrieve full details of a specific task, including its description, \
         status, owner, and dependency relationships (blockedBy/blocks)."
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
                    "description": "The ID of the task to retrieve, e.g. 'task-1'"
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

        match self.registry.get(task_id) {
            Ok(Some(view)) => Ok(ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&view)?,
                error: None,
            }),
            Ok(None) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Task '{task_id}' not found")),
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

    fn make_tool() -> (TempDir, TaskGetTool) {
        let tmp = TempDir::new().unwrap();
        let tool = TaskGetTool::new(Arc::new(TaskRegistry::new(tmp.path())));
        (tmp, tool)
    }

    #[test]
    fn name_and_schema() {
        let (_tmp, tool) = make_tool();
        assert_eq!(tool.name(), "task_get");
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"], json!(["task_id"]));
    }

    #[test]
    fn description_not_empty() {
        let (_tmp, tool) = make_tool();
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn returns_full_task_details() {
        let tmp = TempDir::new().unwrap();
        let registry = Arc::new(TaskRegistry::new(tmp.path()));
        let created = registry
            .create("Do A".into(), "Details A".into(), None, Map::new())
            .unwrap();
        registry
            .update(
                &created.id,
                TaskUpdateRequest {
                    owner: Some("agent-1".into()),
                    ..Default::default()
                },
            )
            .unwrap();

        let tool = TaskGetTool::new(registry);
        let result = tool.execute(json!({"task_id": created.id})).await.unwrap();
        assert!(result.success);

        let view: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(view["id"], created.id);
        assert_eq!(view["subject"], "Do A");
        assert_eq!(view["owner"], "agent-1");
    }

    #[tokio::test]
    async fn unknown_task_returns_failed_result() {
        let (_tmp, tool) = make_tool();
        let result = tool.execute(json!({"task_id": "task-999"})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("not found"));
    }

    #[tokio::test]
    async fn missing_task_id_errors() {
        let (_tmp, tool) = make_tool();
        let err = tool.execute(json!({})).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn empty_task_id_returns_failed_result() {
        let (_tmp, tool) = make_tool();
        let result = tool.execute(json!({"task_id": "  "})).await.unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("must not be empty"));
    }
}
