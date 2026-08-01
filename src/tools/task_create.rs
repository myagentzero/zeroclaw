//! Task creation tool.
//!
//! Implements the `task_create` tool for adding a new task to the
//! persistent, SQLite-backed [`TaskRegistry`](super::task_registry::TaskRegistry).

use super::task_registry::TaskRegistry;
use super::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{Map, json};
use std::sync::Arc;

/// Tool that creates a new orchestration task.
pub struct TaskCreateTool {
    registry: Arc<TaskRegistry>,
}

impl TaskCreateTool {
    pub fn new(registry: Arc<TaskRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "task_create"
    }

    fn description(&self) -> &str {
        "Create a persistent task (subject, description, status, owner, blockedBy/blocks deps). \
         Use task_update to add dependencies or claim ownership."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["subject", "description"],
            "properties": {
                "subject": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Imperative title, e.g. 'Implement xyz feature'"
                },
                "description": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Requirements and acceptance criteria"
                },
                "active_form": {
                    "type": "string",
                    "description": "Present-continuous form while in progress, e.g. 'Implementing xyz feature'"
                },
                "metadata": {
                    "type": "object",
                    "description": "Key-value pairs for tracking (feature, phase, priority, etc.)"
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let subject = args
            .get("subject")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .ok_or_else(|| anyhow::anyhow!("Missing 'subject' parameter"))?;
        if subject.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'subject' parameter must not be empty".into()),
            });
        }

        let description = args
            .get("description")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .ok_or_else(|| anyhow::anyhow!("Missing 'description' parameter"))?;
        if description.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("'description' parameter must not be empty".into()),
            });
        }

        let active_form = args
            .get("active_form")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let metadata = args
            .get("metadata")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_else(Map::new);

        match self.registry.create(
            subject.to_string(),
            description.to_string(),
            active_form,
            metadata,
        ) {
            Ok(task) => Ok(ToolResult {
                success: true,
                output: serde_json::to_string_pretty(&task)?,
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
    use tempfile::TempDir;

    fn make_tool() -> (TempDir, TaskCreateTool) {
        let tmp = TempDir::new().unwrap();
        let tool = TaskCreateTool::new(Arc::new(TaskRegistry::new(tmp.path())));
        (tmp, tool)
    }

    #[test]
    fn name_and_schema() {
        let (_tmp, tool) = make_tool();
        assert_eq!(tool.name(), "task_create");
        let schema = tool.parameters_schema();
        assert_eq!(schema["required"], json!(["subject", "description"]));
        assert_eq!(schema["additionalProperties"], json!(false));
    }

    #[test]
    fn description_not_empty() {
        let (_tmp, tool) = make_tool();
        assert!(!tool.description().is_empty());
    }

    #[tokio::test]
    async fn creates_task_with_required_fields() {
        let (_tmp, tool) = make_tool();
        let result = tool
            .execute(json!({"subject": "Do the thing", "description": "Details here"}))
            .await
            .unwrap();
        assert!(result.success);

        let view: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(view["id"], "task-1");
        assert_eq!(view["subject"], "Do the thing");
        assert_eq!(view["status"], "pending");
        assert_eq!(view["available"], true);
    }

    #[tokio::test]
    async fn creates_task_with_active_form_and_metadata() {
        let (_tmp, tool) = make_tool();
        let result = tool
            .execute(json!({
                "subject": "Implement JWT authentication middleware",
                "description": "Add JWT validation to the API routes.",
                "active_form": "Implementing JWT authentication",
                "metadata": {"feature": "auth", "phase": "2.1"}
            }))
            .await
            .unwrap();
        assert!(result.success);

        let view: serde_json::Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(view["active_form"], "Implementing JWT authentication");
        assert_eq!(view["metadata"]["feature"], "auth");
    }

    #[tokio::test]
    async fn sequential_ids_increment() {
        let (_tmp, tool) = make_tool();
        let r1 = tool
            .execute(json!({"subject": "A", "description": "a"}))
            .await
            .unwrap();
        let r2 = tool
            .execute(json!({"subject": "B", "description": "b"}))
            .await
            .unwrap();

        let v1: serde_json::Value = serde_json::from_str(&r1.output).unwrap();
        let v2: serde_json::Value = serde_json::from_str(&r2.output).unwrap();
        assert_eq!(v1["id"], "task-1");
        assert_eq!(v2["id"], "task-2");
    }

    #[tokio::test]
    async fn missing_subject_errors() {
        let (_tmp, tool) = make_tool();
        let err = tool.execute(json!({"description": "d"})).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn empty_subject_returns_failed_result() {
        let (_tmp, tool) = make_tool();
        let result = tool
            .execute(json!({"subject": "  ", "description": "d"}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("subject"));
    }

    #[tokio::test]
    async fn missing_description_errors() {
        let (_tmp, tool) = make_tool();
        let err = tool.execute(json!({"subject": "s"})).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn empty_description_returns_failed_result() {
        let (_tmp, tool) = make_tool();
        let result = tool
            .execute(json!({"subject": "s", "description": ""}))
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.error.unwrap().contains("description"));
    }
}
