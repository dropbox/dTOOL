//! Prompt templates for `ClickUp` tools

pub const CLICKUP_TASK_CREATE_PROMPT: &str = r#"
    This tool is a wrapper around clickup's create_task API, useful when you need to create a CLICKUP task.
    The input to this tool is a dictionary specifying the fields of the CLICKUP task, and will be passed into clickup's CLICKUP `create_task` function.
    Only add fields described by the user.
    Use the following mapping in order to map the user's priority to the clickup priority: {
            Urgent = 1,
            High = 2,
            Normal = 3,
            Low = 4,
        }. If the user passes in "urgent" replace the priority value as 1.

    Here are a few task descriptions and corresponding input examples:
    Task: create a task called "Daily report"
    Example Input: {"name": "Daily report"}
    Task: Make an open task called "ClickUp toolkit refactor" with description "Refactor the clickup toolkit to use dataclasses for parsing", with status "open"
    Example Input: {"name": "ClickUp toolkit refactor", "description": "Refactor the clickup toolkit to use dataclasses for parsing", "status": "Open"}
    Task: create a task with priority 3 called "New Task Name" with description "New Task Description", with status "open"
    Example Input: {"name": "New Task Name", "description": "New Task Description", "status": "Open", "priority": 3}
    Task: Add a task called "Bob's task" and assign it to Bob (user id: 81928627)
    Example Input: {"name": "Bob's task", "description": "Task for Bob", "assignees": [81928627]}
    "#;

pub const CLICKUP_LIST_CREATE_PROMPT: &str = r#"
    This tool is a wrapper around clickup's create_list API, useful when you need to create a CLICKUP list.
    The input to this tool is a dictionary specifying the fields of a clickup list, and will be passed to clickup's create_list function.
    Only add fields described by the user.
    Use the following mapping in order to map the user's priority to the clickup priority: {
        Urgent = 1,
        High = 2,
        Normal = 3,
        Low = 4,
    }. If the user passes in "urgent" replace the priority value as 1.

    Here are a few list descriptions and corresponding input examples:
    Description: make a list with name "General List"
    Example Input: {"name": "General List"}
    Description: add a new list ("TODOs") with low priority
    Example Input: {"name": "General List", "priority": 4}
    Description: create a list with name "List name", content "List content", priority 2, and status "red"
    Example Input: {"name": "List name", "content": "List content", "priority": 2, "status": "red"}
"#;

pub const CLICKUP_FOLDER_CREATE_PROMPT: &str = r#"
    This tool is a wrapper around clickup's create_folder API, useful when you need to create a CLICKUP folder.
    The input to this tool is a dictionary specifying the fields of a clickup folder, and will be passed to clickup's create_folder function.
    For example, to create a folder with name "Folder name" you would pass in the following dictionary:
    {
        "name": "Folder name",
    }
"#;

pub const CLICKUP_GET_TASK_PROMPT: &str = r#"
    This tool is a wrapper around clickup's API,
    Do NOT use to get a task specific attribute. Use get task attribute instead.
    useful when you need to get a specific task for the user. Given the task id you want to create a request similar to the following dictionary:
    payload = {"task_id": "86a0t44tq"}
    "#;

pub const CLICKUP_GET_TASK_ATTRIBUTE_PROMPT: &str = r#"
    This tool is a wrapper around clickup's API,
    useful when you need to get a specific attribute from a task. Given the task id and desired attribute create a request similar to the following dictionary:
    payload = {"task_id": "<task_id_to_update>", "attribute_name": "<attribute_name_to_update>"}

    Here are some example queries their corresponding payloads:
    Get the name of task 23jn23kjn -> {"task_id": "23jn23kjn", "attribute_name": "name"}
    What is the priority of task 86a0t44tq? -> {"task_id": "86a0t44tq", "attribute_name": "priority"}
    Output the description of task sdc9ds9jc -> {"task_id": "sdc9ds9jc", "attribute_name": "description"}
    Who is assigned to task bgjfnbfg0 -> {"task_id": "bgjfnbfg0", "attribute_name": "assignee"}
    Which is the status of task kjnsdcjc? -> {"task_id": "kjnsdcjc", "attribute_name": "description"}
    How long is the time estimate of task sjncsd999? -> {"task_id": "sjncsd999", "attribute_name": "time_estimate"}
    Is task jnsd98sd archived?-> {"task_id": "jnsd98sd", "attribute_name": "archive"}
    "#;

pub const CLICKUP_GET_ALL_TEAMS_PROMPT: &str = r"
    This tool is a wrapper around clickup's API, useful when you need to get all teams that the user is a part of.
    To get a list of all the teams there is no necessary request parameters.
    ";

pub const CLICKUP_GET_LIST_PROMPT: &str = r#"
    This tool is a wrapper around clickup's API,
    useful when you need to get a specific list for the user. Given the list id you want to create a request similar to the following dictionary:
    payload = {"list_id": "901300608424"}
    "#;

pub const CLICKUP_GET_FOLDERS_PROMPT: &str = r#"
    This tool is a wrapper around clickup's API,
    useful when you need to get a specific folder for the user. Given the user's workspace id you want to create a request similar to the following dictionary:
    payload = {"folder_id": "90130119692"}
    "#;

pub const CLICKUP_GET_SPACES_PROMPT: &str = r#"
    This tool is a wrapper around clickup's API,
    useful when you need to get all the spaces available to a user. Given the user's workspace id you want to create a request similar to the following dictionary:
    payload = {"team_id": "90130119692"}
    "#;

pub const CLICKUP_UPDATE_TASK_PROMPT: &str = r#"
    This tool is a wrapper around clickup's API,
    useful when you need to update a specific attribute of a task. Given the task id, desired attribute to change and the new value you want to create a request similar to the following dictionary:
    payload = {"task_id": "<task_id_to_update>", "attribute_name": "<attribute_name_to_update>", "value": "<value_to_update_to>"}

    Here are some example queries their corresponding payloads:
    Change the name of task 23jn23kjn to new task name -> {"task_id": "23jn23kjn", "attribute_name": "name", "value": "new task name"}
    Update the priority of task 86a0t44tq to 1 -> {"task_id": "86a0t44tq", "attribute_name": "priority", "value": 1}
    Re-write the description of task sdc9ds9jc to 'a new task description' -> {"task_id": "sdc9ds9jc", "attribute_name": "description", "value": "a new task description"}
    Forward the status of task kjnsdcjc to done -> {"task_id": "kjnsdcjc", "attribute_name": "description", "status": "done"}
    Increase the time estimate of task sjncsd999 to 3h -> {"task_id": "sjncsd999", "attribute_name": "time_estimate", "value": 8000}
    Archive task jnsd98sd -> {"task_id": "jnsd98sd", "attribute_name": "archive", "value": true}
    *IMPORTANT*: Pay attention to the exact syntax above and the correct use of quotes.
    For changing priority and time estimates, we expect integers (int).
    For name, description and status we expect strings (str).
    For archive, we expect a boolean (bool).
    "#;

pub const CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT: &str = r#"
    This tool is a wrapper around clickup's API,
    useful when you need to update the assignees of a task. Given the task id, the operation add or remove (rem), and the list of user ids. You want to create a request similar to the following dictionary:
    payload = {"task_id": "<task_id_to_update>", "operation": "<operation, (add or rem)>", "users": [<user_id_1>, <user_id_2>]}

    Here are some example queries their corresponding payloads:
    Add 81928627 and 3987234 as assignees to task 21hw21jn -> {"task_id": "21hw21jn", "operation": "add", "users": [81928627, 3987234]}
    Remove 67823487 as assignee from task jin34ji4 -> {"task_id": "jin34ji4", "operation": "rem", "users": [67823487]}
    *IMPORTANT*: Users id should always be ints.
    "#;

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Prompt Existence and Non-Empty Tests
    // ========================================================================

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_task_create_prompt_not_empty() {
        assert!(!CLICKUP_TASK_CREATE_PROMPT.is_empty());
        assert!(CLICKUP_TASK_CREATE_PROMPT.len() > 100);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_list_create_prompt_not_empty() {
        assert!(!CLICKUP_LIST_CREATE_PROMPT.is_empty());
        assert!(CLICKUP_LIST_CREATE_PROMPT.len() > 100);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_folder_create_prompt_not_empty() {
        assert!(!CLICKUP_FOLDER_CREATE_PROMPT.is_empty());
        assert!(CLICKUP_FOLDER_CREATE_PROMPT.len() > 50);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_get_task_prompt_not_empty() {
        assert!(!CLICKUP_GET_TASK_PROMPT.is_empty());
        assert!(CLICKUP_GET_TASK_PROMPT.len() > 50);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_get_task_attribute_prompt_not_empty() {
        assert!(!CLICKUP_GET_TASK_ATTRIBUTE_PROMPT.is_empty());
        assert!(CLICKUP_GET_TASK_ATTRIBUTE_PROMPT.len() > 100);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_get_all_teams_prompt_not_empty() {
        assert!(!CLICKUP_GET_ALL_TEAMS_PROMPT.is_empty());
        assert!(CLICKUP_GET_ALL_TEAMS_PROMPT.len() > 30);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_get_list_prompt_not_empty() {
        assert!(!CLICKUP_GET_LIST_PROMPT.is_empty());
        assert!(CLICKUP_GET_LIST_PROMPT.len() > 50);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_get_folders_prompt_not_empty() {
        assert!(!CLICKUP_GET_FOLDERS_PROMPT.is_empty());
        assert!(CLICKUP_GET_FOLDERS_PROMPT.len() > 50);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_get_spaces_prompt_not_empty() {
        assert!(!CLICKUP_GET_SPACES_PROMPT.is_empty());
        assert!(CLICKUP_GET_SPACES_PROMPT.len() > 50);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_update_task_prompt_not_empty() {
        assert!(!CLICKUP_UPDATE_TASK_PROMPT.is_empty());
        assert!(CLICKUP_UPDATE_TASK_PROMPT.len() > 200);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_update_task_assignee_prompt_not_empty() {
        assert!(!CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT.is_empty());
        assert!(CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT.len() > 100);
    }

    // ========================================================================
    // Prompt Content Validation Tests
    // ========================================================================

    #[test]
    fn test_task_create_prompt_contains_key_info() {
        let prompt = CLICKUP_TASK_CREATE_PROMPT;
        // Should mention clickup
        assert!(prompt.to_lowercase().contains("clickup"));
        // Should have example inputs
        assert!(prompt.contains("Example Input"));
        // Should mention creating tasks
        assert!(prompt.to_lowercase().contains("task"));
        // Should have priority mapping
        assert!(prompt.contains("Urgent = 1"));
        assert!(prompt.contains("High = 2"));
        assert!(prompt.contains("Normal = 3"));
        assert!(prompt.contains("Low = 4"));
    }

    #[test]
    fn test_list_create_prompt_contains_key_info() {
        let prompt = CLICKUP_LIST_CREATE_PROMPT;
        assert!(prompt.to_lowercase().contains("list"));
        assert!(prompt.contains("Example Input"));
        assert!(prompt.contains("priority"));
    }

    #[test]
    fn test_folder_create_prompt_contains_key_info() {
        let prompt = CLICKUP_FOLDER_CREATE_PROMPT;
        assert!(prompt.to_lowercase().contains("folder"));
        assert!(prompt.contains("name"));
    }

    #[test]
    fn test_get_task_prompt_contains_task_id() {
        let prompt = CLICKUP_GET_TASK_PROMPT;
        assert!(prompt.contains("task_id"));
        assert!(prompt.contains("86a0t44tq")); // Example task ID
    }

    #[test]
    fn test_get_task_attribute_prompt_contains_examples() {
        let prompt = CLICKUP_GET_TASK_ATTRIBUTE_PROMPT;
        assert!(prompt.contains("task_id"));
        assert!(prompt.contains("attribute_name"));
        // Should have examples
        assert!(prompt.contains("name"));
        assert!(prompt.contains("priority"));
        assert!(prompt.contains("description"));
    }

    #[test]
    fn test_update_task_prompt_contains_examples() {
        let prompt = CLICKUP_UPDATE_TASK_PROMPT;
        assert!(prompt.contains("task_id"));
        assert!(prompt.contains("attribute_name"));
        assert!(prompt.contains("value"));
        // Should have various attribute examples
        assert!(prompt.contains("name"));
        assert!(prompt.contains("priority"));
        assert!(prompt.contains("description"));
        assert!(prompt.contains("archive"));
        assert!(prompt.contains("IMPORTANT"));
    }

    #[test]
    fn test_update_task_assignee_prompt_contains_operations() {
        let prompt = CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT;
        assert!(prompt.contains("add"));
        assert!(prompt.contains("rem"));
        assert!(prompt.contains("users"));
        assert!(prompt.contains("task_id"));
        assert!(prompt.contains("operation"));
        // Should mention user IDs are ints
        assert!(prompt.contains("ints"));
    }

    // ========================================================================
    // JSON Example Parsing Tests
    // ========================================================================

    #[test]
    fn test_task_create_prompt_examples_are_valid_json_like() {
        let prompt = CLICKUP_TASK_CREATE_PROMPT;
        // Examples should use proper JSON-like syntax with quotes
        assert!(prompt.contains(r#""name""#));
        assert!(prompt.contains(r#""Daily report""#));
    }

    #[test]
    fn test_update_task_prompt_type_guidance() {
        let prompt = CLICKUP_UPDATE_TASK_PROMPT;
        // Should explain expected types
        assert!(prompt.contains("integers") || prompt.contains("int"));
        assert!(prompt.contains("string") || prompt.contains("str"));
        assert!(prompt.contains("boolean") || prompt.contains("bool"));
    }

    // ========================================================================
    // Prompt Formatting Tests
    // ========================================================================

    #[test]
    fn test_prompts_are_valid_utf8() {
        // All prompts should be valid UTF-8 strings
        let all_prompts = [
            CLICKUP_TASK_CREATE_PROMPT,
            CLICKUP_LIST_CREATE_PROMPT,
            CLICKUP_FOLDER_CREATE_PROMPT,
            CLICKUP_GET_TASK_PROMPT,
            CLICKUP_GET_TASK_ATTRIBUTE_PROMPT,
            CLICKUP_GET_ALL_TEAMS_PROMPT,
            CLICKUP_GET_LIST_PROMPT,
            CLICKUP_GET_FOLDERS_PROMPT,
            CLICKUP_GET_SPACES_PROMPT,
            CLICKUP_UPDATE_TASK_PROMPT,
            CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT,
        ];

        for prompt in all_prompts {
            // Ensure we can iterate over chars without panic
            assert!(prompt.chars().count() > 0);
        }
    }

    #[test]
    fn test_prompts_have_consistent_structure() {
        // Most prompts should mention "wrapper around clickup"
        let api_wrapper_prompts = [
            CLICKUP_TASK_CREATE_PROMPT,
            CLICKUP_LIST_CREATE_PROMPT,
            CLICKUP_FOLDER_CREATE_PROMPT,
            CLICKUP_GET_TASK_PROMPT,
            CLICKUP_GET_TASK_ATTRIBUTE_PROMPT,
            CLICKUP_GET_ALL_TEAMS_PROMPT,
            CLICKUP_GET_LIST_PROMPT,
            CLICKUP_GET_FOLDERS_PROMPT,
            CLICKUP_GET_SPACES_PROMPT,
            CLICKUP_UPDATE_TASK_PROMPT,
            CLICKUP_UPDATE_TASK_ASSIGNEE_PROMPT,
        ];

        for prompt in api_wrapper_prompts {
            // All should reference clickup
            assert!(
                prompt.to_lowercase().contains("clickup"),
                "Prompt should mention clickup"
            );
        }
    }
}
