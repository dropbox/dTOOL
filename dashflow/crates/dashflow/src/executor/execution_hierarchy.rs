//! Execution hierarchy tracking (Observability Phase 3)
//!
//! Provides task-local tracking of nested graph execution IDs so subgraph executions can
//! populate `parent_execution_id`, `root_execution_id`, and `depth` in persisted traces
//! and WAL events.

use std::cell::RefCell;
use std::future::Future;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExecutionHierarchyIds {
    pub(crate) execution_id: String,
    pub(crate) parent_execution_id: Option<String>,
    pub(crate) root_execution_id: Option<String>,
    pub(crate) depth: u32,
}

#[derive(Debug, Clone)]
struct ExecutionFrame {
    execution_id: String,
}

tokio::task_local! {
    static EXECUTION_STACK: RefCell<Vec<ExecutionFrame>>;
}

pub(crate) struct ExecutionScopeGuard {
    _private: (),
}

impl Drop for ExecutionScopeGuard {
    fn drop(&mut self) {
        let _ = EXECUTION_STACK.try_with(|stack| {
            let _ = stack.borrow_mut().pop();
        });
    }
}

pub(crate) async fn with_execution_stack<Fut, T>(future: Fut) -> T
where
    Fut: Future<Output = T>,
{
    if EXECUTION_STACK.try_with(|_| ()).is_ok() {
        future.await
    } else {
        EXECUTION_STACK.scope(RefCell::new(Vec::new()), future).await
    }
}

pub(crate) fn enter_new_execution(execution_id: String) -> (ExecutionHierarchyIds, ExecutionScopeGuard) {
    let (parent_execution_id, root_execution_id, depth) = EXECUTION_STACK.with(|stack| {
        let stack = stack.borrow();
        let parent_execution_id = stack.last().map(|f| f.execution_id.clone());
        let root_execution_id = if stack.is_empty() {
            None
        } else {
            stack.first().map(|f| f.execution_id.clone())
        };
        let depth = stack.len() as u32;
        (parent_execution_id, root_execution_id, depth)
    });

    EXECUTION_STACK.with(|stack| {
        stack
            .borrow_mut()
            .push(ExecutionFrame { execution_id: execution_id.clone() });
    });

    (
        ExecutionHierarchyIds {
            execution_id,
            parent_execution_id,
            root_execution_id,
            depth,
        },
        ExecutionScopeGuard { _private: () },
    )
}

pub(crate) fn current_ids() -> Option<ExecutionHierarchyIds> {
    EXECUTION_STACK
        .try_with(|stack| {
            let stack = stack.borrow();
            let current = stack.last()?.execution_id.clone();
            let parent_execution_id = if stack.len() >= 2 {
                Some(stack[stack.len() - 2].execution_id.clone())
            } else {
                None
            };
            let root_execution_id = if stack.len() >= 2 {
                stack.first().map(|f| f.execution_id.clone())
            } else {
                None
            };
            let depth = stack.len().saturating_sub(1) as u32;
            Some(ExecutionHierarchyIds {
                execution_id: current,
                parent_execution_id,
                root_execution_id,
                depth,
            })
        })
        .ok()
        .flatten()
}

pub(crate) fn capture_stack() -> Option<Vec<String>> {
    EXECUTION_STACK
        .try_with(|stack| stack.borrow().iter().map(|f| f.execution_id.clone()).collect())
        .ok()
}

pub(crate) async fn scope_stack<Fut, T>(stack: Vec<String>, future: Fut) -> T
where
    Fut: Future<Output = T>,
{
    let frames = stack
        .into_iter()
        .map(|execution_id| ExecutionFrame { execution_id })
        .collect();
    EXECUTION_STACK.scope(RefCell::new(frames), future).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_execution_entry() {
        with_execution_stack(async {
            let (ids, _guard) = enter_new_execution("exec-1".to_string());

            assert_eq!(ids.execution_id, "exec-1");
            assert_eq!(ids.parent_execution_id, None);
            assert_eq!(ids.root_execution_id, None);
            assert_eq!(ids.depth, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn test_nested_execution_tracks_parent_and_root() {
        with_execution_stack(async {
            // First execution (root)
            let (ids1, _guard1) = enter_new_execution("root-exec".to_string());
            assert_eq!(ids1.depth, 0);
            assert_eq!(ids1.parent_execution_id, None);
            assert_eq!(ids1.root_execution_id, None);

            // Nested execution (depth 1)
            let (ids2, _guard2) = enter_new_execution("child-exec".to_string());
            assert_eq!(ids2.depth, 1);
            assert_eq!(ids2.parent_execution_id, Some("root-exec".to_string()));
            assert_eq!(ids2.root_execution_id, Some("root-exec".to_string()));

            // Deeply nested execution (depth 2)
            let (ids3, _guard3) = enter_new_execution("grandchild-exec".to_string());
            assert_eq!(ids3.depth, 2);
            assert_eq!(ids3.parent_execution_id, Some("child-exec".to_string()));
            assert_eq!(ids3.root_execution_id, Some("root-exec".to_string()));
        })
        .await;
    }

    #[tokio::test]
    async fn test_current_ids_returns_correct_values() {
        with_execution_stack(async {
            // No executions yet
            assert!(current_ids().is_none());

            let (_ids1, _guard1) = enter_new_execution("exec-a".to_string());
            let current = current_ids().unwrap();
            assert_eq!(current.execution_id, "exec-a");
            assert_eq!(current.depth, 0);

            let (_ids2, _guard2) = enter_new_execution("exec-b".to_string());
            let current = current_ids().unwrap();
            assert_eq!(current.execution_id, "exec-b");
            assert_eq!(current.depth, 1);
            assert_eq!(current.parent_execution_id, Some("exec-a".to_string()));
        })
        .await;
    }

    #[tokio::test]
    async fn test_guard_drop_pops_stack() {
        with_execution_stack(async {
            let (_ids1, _guard1) = enter_new_execution("exec-1".to_string());

            {
                let (_ids2, _guard2) = enter_new_execution("exec-2".to_string());
                let current = current_ids().unwrap();
                assert_eq!(current.execution_id, "exec-2");
                // _guard2 drops here
            }

            // Back to exec-1 after guard2 dropped
            let current = current_ids().unwrap();
            assert_eq!(current.execution_id, "exec-1");
            assert_eq!(current.depth, 0);
        })
        .await;
    }

    #[tokio::test]
    async fn test_capture_and_scope_stack() {
        let captured = with_execution_stack(async {
            let (_ids1, _guard1) = enter_new_execution("exec-a".to_string());
            let (_ids2, _guard2) = enter_new_execution("exec-b".to_string());
            capture_stack()
        })
        .await;

        assert_eq!(captured, Some(vec!["exec-a".to_string(), "exec-b".to_string()]));

        // Restore stack in a new context
        let stack = captured.unwrap();
        let result = scope_stack(stack, async {
            let current = current_ids().unwrap();
            assert_eq!(current.execution_id, "exec-b");
            assert_eq!(current.parent_execution_id, Some("exec-a".to_string()));
            "success"
        })
        .await;

        assert_eq!(result, "success");
    }
}

