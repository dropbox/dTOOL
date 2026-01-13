//! Integration tests for the agent orchestration module.
//!
//! These tests verify the TLA+ safety invariants are maintained across
//! complex multi-agent scenarios.

use super::*;

/// Test scenario: Multiple agents with different capabilities.
#[test]
fn test_capability_routing() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 5,
        max_terminals: 5,
        max_queue_size: 20,
        max_executions: 5,
    });

    // Spawn agents with different capabilities
    let shell_agent = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let net_agent = orch.spawn_agent(&[Capability::Net]).unwrap();
    let _multi_agent = orch
        .spawn_agent(&[Capability::Shell, Capability::File, Capability::Net])
        .unwrap();

    // Queue commands requiring different capabilities
    let shell_cmd = Command::shell(CommandId(0), "echo hello");
    let shell_cmd_id = orch.queue_command(shell_cmd).unwrap();

    let net_cmd = Command::builder(CommandType::Network)
        .approved()
        .payload("curl example.com")
        .build(CommandId(0));
    let net_cmd_id = orch.queue_command(net_cmd).unwrap();

    // Shell command should be assignable to shell_agent or multi_agent
    orch.assign_command(shell_agent, shell_cmd_id).unwrap();

    // Net command should be assignable to net_agent or multi_agent
    orch.assign_command(net_agent, net_cmd_id).unwrap();

    // Verify invariants
    assert!(orch.verify_invariants());
}

/// Test scenario: Dependency chain execution.
#[test]
fn test_dependency_chain() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 3,
        max_terminals: 3,
        max_queue_size: 10,
        max_executions: 3,
    });

    let agent = orch.spawn_agent(&[Capability::Shell]).unwrap();

    // Create dependency chain: cmd1 -> cmd2 -> cmd3
    let cmd1 = Command::shell(CommandId(0), "step1");
    let cmd1_id = orch.queue_command(cmd1).unwrap();

    let cmd2 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd1_id)
        .payload("step2")
        .build(CommandId(0));
    let cmd2_id = orch.queue_command(cmd2).unwrap();

    let cmd3 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd2_id)
        .payload("step3")
        .build(CommandId(0));
    let cmd3_id = orch.queue_command(cmd3).unwrap();

    // Only cmd1 should be ready initially
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&cmd1_id));

    // Execute cmd1
    orch.assign_command(agent, cmd1_id).unwrap();
    orch.begin_execution(agent).unwrap();
    orch.complete_execution(agent, 0).unwrap();
    orch.reset_agent(agent).unwrap();

    // Now cmd2 should be ready
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&cmd2_id));

    // Execute cmd2
    orch.assign_command(agent, cmd2_id).unwrap();
    orch.begin_execution(agent).unwrap();
    orch.complete_execution(agent, 0).unwrap();
    orch.reset_agent(agent).unwrap();

    // Now cmd3 should be ready
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&cmd3_id));

    assert!(orch.verify_invariants());
}

/// Test scenario: Parallel execution up to terminal limit.
#[test]
fn test_parallel_execution() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 10,
        max_terminals: 3,
        max_queue_size: 20,
        max_executions: 10,
    });

    // Spawn 5 agents
    for _ in 0..5 {
        orch.spawn_agent(&[Capability::Shell]).unwrap();
    }

    // Queue 5 independent commands
    for i in 0..5 {
        let cmd = Command::shell(CommandId(0), &format!("cmd{}", i));
        orch.queue_command(cmd).unwrap();
    }

    // Run auto-assignment
    let assigned = orch.auto_assign();
    assert_eq!(assigned, 5); // All commands assigned

    // Run auto-execution
    let started = orch.auto_execute();
    assert_eq!(started, 3); // Only 3 terminals available

    // Verify exactly 3 running
    assert_eq!(orch.active_execution_count(), 3);
    let (_, in_use) = orch.terminal_stats();
    assert_eq!(in_use, 3);
    assert!(orch.verify_invariants());
}

/// Test scenario: Failure and recovery.
#[test]
fn test_failure_recovery() {
    let mut orch = Orchestrator::with_defaults();

    let agent = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let cmd_id = orch
        .queue_command(Command::shell(CommandId(0), "fail"))
        .unwrap();

    // Start execution
    orch.assign_command(agent, cmd_id).unwrap();
    orch.begin_execution(agent).unwrap();

    // Fail execution
    orch.fail_execution(agent, "Command failed").unwrap();

    // Agent should be in Failed state
    assert_eq!(orch.get_agent(agent).unwrap().state, AgentState::Failed);

    // Terminal should be released
    let (available, _) = orch.terminal_stats();
    assert_eq!(available, 5);

    // Command should NOT be in completed set
    assert!(!orch.completed_commands().contains(&cmd_id));

    // Reset and try again
    orch.reset_agent(agent).unwrap();

    // Can queue a new command
    let cmd2_id = orch
        .queue_command(Command::shell(CommandId(0), "retry"))
        .unwrap();
    orch.assign_command(agent, cmd2_id).unwrap();
    orch.begin_execution(agent).unwrap();
    orch.complete_execution(agent, 0).unwrap();

    assert!(orch.completed_commands().contains(&cmd2_id));
    assert!(orch.verify_invariants());
}

/// Test scenario: Cancellation during assignment.
#[test]
fn test_cancel_assigned() {
    let mut orch = Orchestrator::with_defaults();

    let agent = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let cmd_id = orch
        .queue_command(Command::shell(CommandId(0), "cmd"))
        .unwrap();

    // Assign but don't execute
    orch.assign_command(agent, cmd_id).unwrap();
    assert_eq!(orch.get_agent(agent).unwrap().state, AgentState::Assigned);

    // Cancel
    orch.cancel_execution(agent).unwrap();
    assert_eq!(orch.get_agent(agent).unwrap().state, AgentState::Cancelled);

    // No execution should exist for this agent
    assert!(orch.find_execution_by_agent(agent).is_none());

    assert!(orch.verify_invariants());
}

/// Test scenario: Cancellation during execution.
#[test]
fn test_cancel_executing() {
    let mut orch = Orchestrator::with_defaults();

    let agent = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let cmd_id = orch
        .queue_command(Command::shell(CommandId(0), "cmd"))
        .unwrap();

    orch.assign_command(agent, cmd_id).unwrap();
    let exec_id = orch.begin_execution(agent).unwrap();

    // Verify executing
    assert_eq!(orch.get_agent(agent).unwrap().state, AgentState::Executing);
    let (_, in_use) = orch.terminal_stats();
    assert_eq!(in_use, 1);

    // Cancel
    orch.cancel_execution(agent).unwrap();

    // Agent cancelled, terminal released
    assert_eq!(orch.get_agent(agent).unwrap().state, AgentState::Cancelled);
    let (available, _) = orch.terminal_stats();
    assert_eq!(available, 5);

    // Execution should be cancelled
    assert_eq!(
        orch.get_execution(exec_id).unwrap().state,
        ExecutionState::Cancelled
    );

    assert!(orch.verify_invariants());
}

/// Test scenario: Diamond dependency pattern.
///
/// ```text
///     cmd1
///    /    \
/// cmd2    cmd3
///    \    /
///     cmd4
/// ```
#[test]
fn test_diamond_dependencies() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 3,
        max_terminals: 3,
        max_queue_size: 10,
        max_executions: 3,
    });

    // Create diamond
    let cmd1 = Command::shell(CommandId(0), "root");
    let cmd1_id = orch.queue_command(cmd1).unwrap();

    let cmd2 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd1_id)
        .payload("left")
        .build(CommandId(0));
    let cmd2_id = orch.queue_command(cmd2).unwrap();

    let cmd3 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd1_id)
        .payload("right")
        .build(CommandId(0));
    let cmd3_id = orch.queue_command(cmd3).unwrap();

    let cmd4 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd2_id)
        .depends_on(cmd3_id)
        .payload("join")
        .build(CommandId(0));
    let cmd4_id = orch.queue_command(cmd4).unwrap();

    // Spawn agents
    let agent1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let agent2 = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let _agent3 = orch.spawn_agent(&[Capability::Shell]).unwrap();

    // Only cmd1 ready
    assert_eq!(orch.ready_commands().len(), 1);

    // Execute cmd1
    orch.assign_command(agent1, cmd1_id).unwrap();
    orch.begin_execution(agent1).unwrap();
    orch.complete_execution(agent1, 0).unwrap();
    orch.reset_agent(agent1).unwrap();

    // Now cmd2 and cmd3 should be ready
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 2);
    assert!(ready.contains(&cmd2_id));
    assert!(ready.contains(&cmd3_id));

    // Execute cmd2 and cmd3 in parallel
    orch.assign_command(agent1, cmd2_id).unwrap();
    orch.assign_command(agent2, cmd3_id).unwrap();
    orch.begin_execution(agent1).unwrap();
    orch.begin_execution(agent2).unwrap();

    // cmd4 not ready yet
    let ready = orch.ready_commands();
    assert!(!ready.contains(&cmd4_id));

    // Complete both
    orch.complete_execution(agent1, 0).unwrap();
    orch.complete_execution(agent2, 0).unwrap();
    orch.reset_agent(agent1).unwrap();
    orch.reset_agent(agent2).unwrap();

    // Now cmd4 should be ready
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&cmd4_id));

    assert!(orch.verify_invariants());
}

/// Test scenario: Approval workflow integration.
#[test]
fn test_approval_workflow() {
    let mut orch = Orchestrator::with_defaults();

    let agent = orch.spawn_agent(&[Capability::Shell]).unwrap();

    // Queue unapproved command
    let cmd = Command::builder(CommandType::Shell)
        .payload("dangerous command")
        .build(CommandId(0));
    let cmd_id = orch.queue_command(cmd).unwrap();

    // Not in ready commands (unapproved)
    assert!(orch.ready_commands().is_empty());

    // Cannot assign unapproved
    assert!(matches!(
        orch.assign_command(agent, cmd_id),
        Err(OrchestratorError::NotApproved)
    ));

    // Approve
    orch.approve_command(cmd_id).unwrap();

    // Now in ready commands
    assert!(orch.ready_commands().contains(&cmd_id));

    // Can assign
    assert!(orch.assign_command(agent, cmd_id).is_ok());

    assert!(orch.verify_invariants());
}

/// Test scenario: INV-ORCH-1 enforcement (no double assignment).
#[test]
fn test_no_double_assignment() {
    let mut orch = Orchestrator::with_defaults();

    let agent1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let agent2 = orch.spawn_agent(&[Capability::Shell]).unwrap();

    let cmd_id = orch
        .queue_command(Command::shell(CommandId(0), "cmd"))
        .unwrap();

    // First assignment succeeds
    orch.assign_command(agent1, cmd_id).unwrap();

    // Second assignment to different agent should fail
    let result = orch.assign_command(agent2, cmd_id);
    assert!(result.is_err());

    assert!(orch.verify_invariants());
}

/// Test scenario: Resource exhaustion and recovery.
#[test]
fn test_resource_exhaustion() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 2,
        max_terminals: 2,
        max_queue_size: 10,
        max_executions: 10,
    });

    // Fill up agents
    let a1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let a2 = orch.spawn_agent(&[Capability::Shell]).unwrap();
    assert!(matches!(
        orch.spawn_agent(&[Capability::Shell]),
        Err(OrchestratorError::MaxAgentsReached)
    ));

    // Fill up terminals
    let c1 = orch
        .queue_command(Command::shell(CommandId(0), "c1"))
        .unwrap();
    let c2 = orch
        .queue_command(Command::shell(CommandId(0), "c2"))
        .unwrap();
    let c3 = orch
        .queue_command(Command::shell(CommandId(0), "c3"))
        .unwrap();

    orch.assign_command(a1, c1).unwrap();
    orch.assign_command(a2, c2).unwrap();
    orch.begin_execution(a1).unwrap();
    orch.begin_execution(a2).unwrap();

    // No agents or terminals available
    assert!(orch.idle_agents().next().is_none());
    assert!(!orch.has_available_terminals());

    // Auto-assign does nothing
    assert_eq!(orch.auto_assign(), 0);

    // Complete one execution
    orch.complete_execution(a1, 0).unwrap();
    orch.reset_agent(a1).unwrap();

    // Now can assign c3
    orch.assign_command(a1, c3).unwrap();
    orch.begin_execution(a1).unwrap();

    assert!(orch.verify_invariants());
}

// =============================================================================
// Extended Integration Tests (Iteration 250)
// =============================================================================

/// Test scenario: Complex multi-agent capability-based routing.
///
/// Multiple agents with overlapping capabilities and commands that require
/// different capability combinations.
#[test]
fn test_complex_capability_routing() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 10,
        max_terminals: 10,
        max_queue_size: 50,
        max_executions: 10,
    });

    // Create agents with varying capability profiles
    let shell_only = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let net_only = orch.spawn_agent(&[Capability::Net]).unwrap();
    let file_only = orch.spawn_agent(&[Capability::File]).unwrap();
    let _shell_file = orch
        .spawn_agent(&[Capability::Shell, Capability::File])
        .unwrap();
    let _shell_net = orch
        .spawn_agent(&[Capability::Shell, Capability::Net])
        .unwrap();
    let all_caps = orch
        .spawn_agent(&[
            Capability::Shell,
            Capability::File,
            Capability::Net,
            Capability::Git,
        ])
        .unwrap();

    // Queue commands requiring different capabilities
    let cmd_shell = Command::shell(CommandId(0), "echo test");
    let cmd_shell_id = orch.queue_command(cmd_shell).unwrap();

    let cmd_net = Command::builder(CommandType::Network)
        .approved()
        .payload("curl localhost")
        .build(CommandId(0));
    let cmd_net_id = orch.queue_command(cmd_net).unwrap();

    let cmd_file = Command::builder(CommandType::FileOp)
        .approved()
        .payload("cat file")
        .build(CommandId(0));
    let cmd_file_id = orch.queue_command(cmd_file).unwrap();

    let cmd_git = Command::builder(CommandType::Git)
        .approved()
        .payload("git status")
        .build(CommandId(0));
    let cmd_git_id = orch.queue_command(cmd_git).unwrap();

    // Auto-assign will route to first capable idle agent
    // Because HashMap iteration order is arbitrary and the all_caps agent
    // might be found first for multiple commands, we don't assert exact count.
    // Instead we manually assign to verify capability checking works.

    // Manually assign to specific agents to test capability routing
    orch.assign_command(shell_only, cmd_shell_id).unwrap();
    orch.assign_command(net_only, cmd_net_id).unwrap();
    orch.assign_command(file_only, cmd_file_id).unwrap();
    orch.assign_command(all_caps, cmd_git_id).unwrap();

    // Verify each command went to a capable agent
    let shell_agent = orch
        .agents()
        .find(|a| a.current_command_id == Some(cmd_shell_id))
        .unwrap();
    assert!(shell_agent.capabilities.contains(&Capability::Shell));

    let net_agent = orch
        .agents()
        .find(|a| a.current_command_id == Some(cmd_net_id))
        .unwrap();
    assert!(net_agent.capabilities.contains(&Capability::Net));

    let file_agent = orch
        .agents()
        .find(|a| a.current_command_id == Some(cmd_file_id))
        .unwrap();
    assert!(file_agent.capabilities.contains(&Capability::File));

    let git_agent = orch
        .agents()
        .find(|a| a.current_command_id == Some(cmd_git_id))
        .unwrap();
    assert!(git_agent.capabilities.contains(&Capability::Git));

    // Invariants hold after complex routing
    assert!(orch.verify_invariants());
}

/// Test scenario: Complex dependency DAG with multiple roots.
///
/// ```text
///  cmd1   cmd2   cmd3  (roots)
///    \    /  \    /
///    cmd4    cmd5
///       \    /
///        cmd6
/// ```
#[test]
fn test_complex_dependency_dag() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 6,
        max_terminals: 6,
        max_queue_size: 20,
        max_executions: 6,
    });

    // Create DAG structure
    let cmd1 = Command::shell(CommandId(0), "root1");
    let cmd1_id = orch.queue_command(cmd1).unwrap();

    let cmd2 = Command::shell(CommandId(0), "root2");
    let cmd2_id = orch.queue_command(cmd2).unwrap();

    let cmd3 = Command::shell(CommandId(0), "root3");
    let cmd3_id = orch.queue_command(cmd3).unwrap();

    // cmd4 depends on cmd1 and cmd2
    let cmd4 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd1_id)
        .depends_on(cmd2_id)
        .payload("middle1")
        .build(CommandId(0));
    let cmd4_id = orch.queue_command(cmd4).unwrap();

    // cmd5 depends on cmd2 and cmd3
    let cmd5 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd2_id)
        .depends_on(cmd3_id)
        .payload("middle2")
        .build(CommandId(0));
    let cmd5_id = orch.queue_command(cmd5).unwrap();

    // cmd6 depends on cmd4 and cmd5 (final)
    let cmd6 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd4_id)
        .depends_on(cmd5_id)
        .payload("final")
        .build(CommandId(0));
    let cmd6_id = orch.queue_command(cmd6).unwrap();

    // Spawn agents
    for _ in 0..6 {
        orch.spawn_agent(&[Capability::Shell]).unwrap();
    }

    // Phase 1: Only roots should be ready
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 3);
    assert!(ready.contains(&cmd1_id));
    assert!(ready.contains(&cmd2_id));
    assert!(ready.contains(&cmd3_id));
    assert!(!ready.contains(&cmd4_id));
    assert!(!ready.contains(&cmd5_id));
    assert!(!ready.contains(&cmd6_id));

    // Execute all roots in parallel
    orch.step();
    assert_eq!(orch.active_execution_count(), 3);

    // Complete roots
    let executing_agents: Vec<_> = orch
        .agents()
        .filter(|a| a.state == AgentState::Executing)
        .map(|a| a.id)
        .collect();
    for agent_id in executing_agents {
        orch.complete_execution(agent_id, 0).unwrap();
        orch.reset_agent(agent_id).unwrap();
    }

    // Phase 2: Middle commands should now be ready
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 2);
    assert!(ready.contains(&cmd4_id));
    assert!(ready.contains(&cmd5_id));
    assert!(!ready.contains(&cmd6_id));

    // Execute middle commands
    orch.step();
    assert_eq!(orch.active_execution_count(), 2);

    // Complete middle commands
    let executing_agents: Vec<_> = orch
        .agents()
        .filter(|a| a.state == AgentState::Executing)
        .map(|a| a.id)
        .collect();
    for agent_id in executing_agents {
        orch.complete_execution(agent_id, 0).unwrap();
        orch.reset_agent(agent_id).unwrap();
    }

    // Phase 3: Final command should now be ready
    let ready = orch.ready_commands();
    assert_eq!(ready.len(), 1);
    assert!(ready.contains(&cmd6_id));

    assert!(orch.verify_invariants());
}

/// Test scenario: Rapid concurrent execution with terminal cycling.
///
/// Multiple rounds of execution with agents completing at different times,
/// testing terminal pool churn.
#[test]
fn test_rapid_concurrent_execution() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 10,
        max_terminals: 3, // Limited terminals
        max_queue_size: 100,
        max_executions: 50,
    });

    // Spawn many agents
    for _ in 0..10 {
        orch.spawn_agent(&[Capability::Shell]).unwrap();
    }

    // Queue many commands
    for i in 0..20 {
        let cmd = Command::shell(CommandId(0), &format!("cmd{}", i));
        orch.queue_command(cmd).unwrap();
    }

    // Run multiple rounds of execution
    let mut completed = 0;
    let mut rounds = 0;
    let max_rounds = 100; // Safety limit

    while completed < 20 && rounds < max_rounds {
        rounds += 1;

        // Auto-assign and execute
        orch.step();

        // Invariants must hold at every step
        assert!(orch.verify_invariants());

        // Complete some executions (simulate staggered completion)
        let executing: Vec<_> = orch
            .agents()
            .filter(|a| a.state == AgentState::Executing)
            .map(|a| a.id)
            .collect();

        // Complete first executing agent (if any)
        if let Some(&agent_id) = executing.first() {
            orch.complete_execution(agent_id, 0).unwrap();
            orch.reset_agent(agent_id).unwrap();
            completed += 1;
        }

        assert!(orch.verify_invariants());
    }

    assert_eq!(completed, 20, "Should complete all commands");
    assert!(orch.completed_commands().len() >= 20);
    assert!(orch.verify_invariants());
}

/// Test scenario: Terminal pool stress test with rapid acquire/release.
#[test]
fn test_terminal_pool_stress() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 20,
        max_terminals: 5,
        max_queue_size: 100,
        max_executions: 50,
    });

    // Spawn agents
    for _ in 0..20 {
        orch.spawn_agent(&[Capability::Shell]).unwrap();
    }

    // Run stress cycles
    for cycle in 0..10 {
        // Queue 5 commands (equal to terminal count)
        for i in 0..5 {
            let cmd = Command::shell(CommandId(0), &format!("cycle{}_cmd{}", cycle, i));
            orch.queue_command(cmd).unwrap();
        }

        // Execute all at once (should use all terminals)
        orch.step();
        let (_, in_use) = orch.terminal_stats();
        assert!(in_use <= 5);
        assert!(orch.verify_invariants());

        // Complete all
        let executing: Vec<_> = orch
            .agents()
            .filter(|a| a.state == AgentState::Executing)
            .map(|a| a.id)
            .collect();

        for agent_id in executing {
            orch.complete_execution(agent_id, 0).unwrap();
            orch.reset_agent(agent_id).unwrap();
        }

        // All terminals should be available
        let (available, in_use) = orch.terminal_stats();
        assert_eq!(available, 5);
        assert_eq!(in_use, 0);
        assert!(orch.verify_invariants());
    }
}

/// Test scenario: Mixed success/failure execution patterns.
#[test]
fn test_mixed_success_failure_patterns() {
    let mut orch = Orchestrator::with_defaults();

    let _agents: Vec<_> = (0..5)
        .map(|_| orch.spawn_agent(&[Capability::Shell]).unwrap())
        .collect();

    // Queue commands
    let _cmd_ids: Vec<_> = (0..10)
        .map(|i| {
            let cmd = Command::shell(CommandId(0), &format!("cmd{}", i));
            orch.queue_command(cmd).unwrap()
        })
        .collect();

    // Execute with alternating success/failure
    let mut completed = 0;
    let mut failed = 0;

    while completed + failed < 10 {
        orch.step();

        let executing: Vec<_> = orch
            .agents()
            .filter(|a| a.state == AgentState::Executing)
            .map(|a| a.id)
            .collect();

        for (idx, &agent_id) in executing.iter().enumerate() {
            // Alternate between success and failure
            if (completed + failed + idx) % 3 == 0 {
                orch.fail_execution(agent_id, "simulated failure").unwrap();
                failed += 1;
            } else {
                orch.complete_execution(agent_id, 0).unwrap();
                completed += 1;
            }
            orch.reset_agent(agent_id).unwrap();
        }

        assert!(orch.verify_invariants());
    }

    // Should have mix of completed and failed
    assert!(completed > 0);
    assert!(failed > 0);
    assert_eq!(completed + failed, 10);
    assert!(orch.verify_invariants());
}

/// Test scenario: Approval workflow with concurrent requests.
#[test]
fn test_concurrent_approval_workflow() {
    let mut orch = Orchestrator::with_defaults();
    orch.set_require_approval(true);

    let agent1 = orch.spawn_agent(&[Capability::Shell]).unwrap();
    let agent2 = orch.spawn_agent(&[Capability::Shell]).unwrap();

    // Queue unapproved commands
    let cmd1 = Command::builder(CommandType::Shell)
        .payload("dangerous1")
        .build(CommandId(0));
    let cmd1_id = orch.queue_command(cmd1).unwrap();

    let cmd2 = Command::builder(CommandType::Shell)
        .payload("dangerous2")
        .build(CommandId(0));
    let cmd2_id = orch.queue_command(cmd2).unwrap();

    // Neither should be ready
    assert!(orch.ready_commands().is_empty());

    // Request approvals
    let req1 = orch.request_approval(agent1, cmd1_id).unwrap();
    let req2 = orch.request_approval(agent2, cmd2_id).unwrap();

    // Both pending
    assert_eq!(orch.pending_approval_count(), 2);
    assert!(orch.is_request_pending(req1));
    assert!(orch.is_request_pending(req2));

    // Approve first, reject second
    orch.approve_request(req1).unwrap();
    orch.reject_request(req2).unwrap();

    // Check states
    assert!(orch.is_request_approved(req1));
    assert!(!orch.is_request_approved(req2));
    assert_eq!(orch.pending_approval_count(), 0);

    // First command can proceed (after manual approval of command)
    orch.approve_command(cmd1_id).unwrap();
    assert!(orch.ready_commands().contains(&cmd1_id));

    assert!(orch.verify_invariants());
}

/// Test scenario: Dependency chain with middle failure.
///
/// When a command in the middle of a chain fails, dependent commands
/// should remain blocked indefinitely.
#[test]
fn test_dependency_chain_failure_blocking() {
    let mut orch = Orchestrator::with_defaults();

    let agent = orch.spawn_agent(&[Capability::Shell]).unwrap();

    // Create chain: cmd1 -> cmd2 -> cmd3
    let cmd1 = Command::shell(CommandId(0), "step1");
    let cmd1_id = orch.queue_command(cmd1).unwrap();

    let cmd2 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd1_id)
        .payload("step2")
        .build(CommandId(0));
    let cmd2_id = orch.queue_command(cmd2).unwrap();

    let cmd3 = Command::builder(CommandType::Shell)
        .approved()
        .depends_on(cmd2_id)
        .payload("step3")
        .build(CommandId(0));
    let cmd3_id = orch.queue_command(cmd3).unwrap();

    // Execute and complete cmd1
    orch.assign_command(agent, cmd1_id).unwrap();
    orch.begin_execution(agent).unwrap();
    orch.complete_execution(agent, 0).unwrap();
    orch.reset_agent(agent).unwrap();

    // cmd2 should be ready
    assert!(orch.ready_commands().contains(&cmd2_id));
    assert!(!orch.ready_commands().contains(&cmd3_id));

    // Execute and FAIL cmd2
    orch.assign_command(agent, cmd2_id).unwrap();
    orch.begin_execution(agent).unwrap();
    orch.fail_execution(agent, "step2 failed").unwrap();
    orch.reset_agent(agent).unwrap();

    // cmd2 is NOT in completed_commands because it failed
    assert!(!orch.completed_commands().contains(&cmd2_id));

    // cmd3 should NEVER become ready (dependency not satisfied)
    assert!(!orch.ready_commands().contains(&cmd3_id));

    // Even after many steps
    for _ in 0..10 {
        orch.step();
        assert!(!orch.ready_commands().contains(&cmd3_id));
    }

    assert!(orch.verify_invariants());
}

/// Test scenario: Multiple agents competing for same command.
///
/// Verifies INV-ORCH-1 under concurrent assignment attempts.
#[test]
fn test_competing_agents_single_command() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 10,
        max_terminals: 10,
        max_queue_size: 10,
        max_executions: 10,
    });

    // Many agents with same capability
    let agents: Vec<_> = (0..10)
        .map(|_| orch.spawn_agent(&[Capability::Shell]).unwrap())
        .collect();

    // Single command
    let cmd_id = orch
        .queue_command(Command::shell(CommandId(0), "single"))
        .unwrap();

    // Try to assign to all agents - only one should succeed
    let mut successful_assignments = 0;
    let mut successful_agent = None;

    for &agent_id in &agents {
        if orch.assign_command(agent_id, cmd_id).is_ok() {
            successful_assignments += 1;
            successful_agent = Some(agent_id);
        }
    }

    // Exactly one assignment should succeed
    assert_eq!(successful_assignments, 1);

    // The successful agent should have the command
    let agent = orch.get_agent(successful_agent.unwrap()).unwrap();
    assert_eq!(agent.current_command_id, Some(cmd_id));
    assert_eq!(agent.state, AgentState::Assigned);

    assert!(orch.verify_invariants());
}

/// Test scenario: All agents fail, then recover.
#[test]
fn test_mass_failure_and_recovery() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 5,
        max_terminals: 5,
        max_queue_size: 20,
        max_executions: 10,
    });

    // Spawn and assign
    let agents: Vec<_> = (0..5)
        .map(|_| orch.spawn_agent(&[Capability::Shell]).unwrap())
        .collect();

    for (i, &agent_id) in agents.iter().enumerate() {
        let cmd = Command::shell(CommandId(0), &format!("cmd{}", i));
        let cmd_id = orch.queue_command(cmd).unwrap();
        orch.assign_command(agent_id, cmd_id).unwrap();
        orch.begin_execution(agent_id).unwrap();
    }

    // All executing
    assert_eq!(orch.active_execution_count(), 5);
    let (available, in_use) = orch.terminal_stats();
    assert_eq!(in_use, 5);
    assert_eq!(available, 0);

    // All fail
    for &agent_id in &agents {
        orch.fail_execution(agent_id, "mass failure").unwrap();
    }

    // All failed, terminals released
    for &agent_id in &agents {
        let agent = orch.get_agent(agent_id).unwrap();
        assert_eq!(agent.state, AgentState::Failed);
    }
    let (available, in_use) = orch.terminal_stats();
    assert_eq!(in_use, 0);
    assert_eq!(available, 5);

    // Reset all
    for &agent_id in &agents {
        orch.reset_agent(agent_id).unwrap();
    }

    // All idle again
    for &agent_id in &agents {
        let agent = orch.get_agent(agent_id).unwrap();
        assert_eq!(agent.state, AgentState::Idle);
    }

    // Can execute new commands
    for (i, &agent_id) in agents.iter().enumerate() {
        let cmd = Command::shell(CommandId(0), &format!("recovery{}", i));
        let cmd_id = orch.queue_command(cmd).unwrap();
        orch.assign_command(agent_id, cmd_id).unwrap();
        orch.begin_execution(agent_id).unwrap();
        orch.complete_execution(agent_id, 0).unwrap();
        orch.reset_agent(agent_id).unwrap();
    }

    assert_eq!(orch.completed_commands().len(), 5);
    assert!(orch.verify_invariants());
}

/// Test scenario: Command queue saturation.
#[test]
fn test_queue_saturation() {
    let mut orch = Orchestrator::new(OrchestratorConfig {
        max_agents: 5,
        max_terminals: 5,
        max_queue_size: 10,
        max_executions: 5,
    });

    // Fill queue exactly
    for i in 0..10 {
        let cmd = Command::shell(CommandId(0), &format!("cmd{}", i));
        orch.queue_command(cmd).unwrap();
    }

    // Queue should be full
    let cmd_extra = Command::shell(CommandId(0), "overflow");
    assert!(matches!(
        orch.queue_command(cmd_extra),
        Err(OrchestratorError::QueueFull)
    ));

    // Spawn agent and execute some commands to free queue
    let agent = orch.spawn_agent(&[Capability::Shell]).unwrap();

    for _ in 0..3 {
        let cmd_id = orch.ready_commands()[0];
        orch.assign_command(agent, cmd_id).unwrap();
        orch.begin_execution(agent).unwrap();
        orch.complete_execution(agent, 0).unwrap();
        orch.reset_agent(agent).unwrap();
    }

    // Queue should have space now
    let cmd_new = Command::shell(CommandId(0), "after_free");
    assert!(orch.queue_command(cmd_new).is_ok());

    assert!(orch.verify_invariants());
}

// =============================================================================
// Phase 12: Domain Integration Tests
// =============================================================================

mod domain_integration {
    use super::*;
    use crate::domain::{
        Domain, DomainError, DomainId, DomainRegistry, DomainResult, DomainState, DomainType, Pane,
        PaneId, SpawnConfig,
    };
    use std::sync::{Arc, Mutex};

    /// Mock pane for testing.
    struct MockPane {
        id: PaneId,
        domain_id: DomainId,
        alive: Mutex<bool>,
        exit_status: Mutex<Option<i32>>,
        output: Mutex<Vec<u8>>,
        size: Mutex<(u16, u16)>,
    }

    impl MockPane {
        fn new(domain_id: DomainId) -> Self {
            Self {
                id: PaneId::new(),
                domain_id,
                alive: Mutex::new(true),
                exit_status: Mutex::new(None),
                output: Mutex::new(Vec::new()),
                size: Mutex::new((80, 24)),
            }
        }

        fn set_exit(&self, code: i32) {
            *self.alive.lock().unwrap() = false;
            *self.exit_status.lock().unwrap() = Some(code);
        }

        fn add_output(&self, data: &[u8]) {
            self.output.lock().unwrap().extend_from_slice(data);
        }
    }

    impl Pane for MockPane {
        fn pane_id(&self) -> PaneId {
            self.id
        }

        fn domain_id(&self) -> DomainId {
            self.domain_id
        }

        fn size(&self) -> (u16, u16) {
            *self.size.lock().unwrap()
        }

        fn resize(&self, cols: u16, rows: u16) -> DomainResult<()> {
            *self.size.lock().unwrap() = (cols, rows);
            Ok(())
        }

        fn write(&self, data: &[u8]) -> DomainResult<usize> {
            Ok(data.len())
        }

        fn read(&self, buf: &mut [u8]) -> DomainResult<usize> {
            let mut output = self.output.lock().unwrap();
            let len = output.len().min(buf.len());
            buf[..len].copy_from_slice(&output[..len]);
            output.drain(..len);
            Ok(len)
        }

        fn is_alive(&self) -> bool {
            *self.alive.lock().unwrap()
        }

        fn exit_status(&self) -> Option<i32> {
            *self.exit_status.lock().unwrap()
        }

        fn kill(&self) -> DomainResult<()> {
            self.set_exit(-9);
            Ok(())
        }
    }

    /// Mock domain for testing.
    struct MockDomain {
        id: DomainId,
        name: String,
        state: Mutex<DomainState>,
        panes: Mutex<Vec<Arc<MockPane>>>,
        spawn_fails: Mutex<bool>,
    }

    impl MockDomain {
        fn new(name: impl Into<String>) -> Self {
            Self {
                id: DomainId::new(),
                name: name.into(),
                state: Mutex::new(DomainState::Attached),
                panes: Mutex::new(Vec::new()),
                spawn_fails: Mutex::new(false),
            }
        }

        fn set_spawn_fails(&self, fails: bool) {
            *self.spawn_fails.lock().unwrap() = fails;
        }
    }

    impl Domain for MockDomain {
        fn domain_id(&self) -> DomainId {
            self.id
        }

        fn domain_name(&self) -> &str {
            &self.name
        }

        fn domain_type(&self) -> DomainType {
            DomainType::Local
        }

        fn state(&self) -> DomainState {
            *self.state.lock().unwrap()
        }

        fn detachable(&self) -> bool {
            false
        }

        fn attach(&self) -> DomainResult<()> {
            *self.state.lock().unwrap() = DomainState::Attached;
            Ok(())
        }

        fn detach(&self) -> DomainResult<()> {
            *self.state.lock().unwrap() = DomainState::Detached;
            Ok(())
        }

        fn spawn_pane(
            &self,
            _cols: u16,
            _rows: u16,
            _config: SpawnConfig,
        ) -> DomainResult<Arc<dyn Pane>> {
            if *self.spawn_fails.lock().unwrap() {
                return Err(DomainError::SpawnFailed("Mock spawn failure".to_string()));
            }
            let pane = Arc::new(MockPane::new(self.id));
            self.panes.lock().unwrap().push(pane.clone());
            Ok(pane)
        }

        fn get_pane(&self, id: PaneId) -> Option<Arc<dyn Pane>> {
            self.panes
                .lock()
                .unwrap()
                .iter()
                .find(|p| p.pane_id() == id)
                .map(|p| p.clone() as Arc<dyn Pane>)
        }

        fn list_panes(&self) -> Vec<Arc<dyn Pane>> {
            self.panes
                .lock()
                .unwrap()
                .iter()
                .map(|p| p.clone() as Arc<dyn Pane>)
                .collect()
        }

        fn remove_pane(&self, id: PaneId) -> Option<Arc<dyn Pane>> {
            let mut panes = self.panes.lock().unwrap();
            if let Some(pos) = panes.iter().position(|p| p.pane_id() == id) {
                Some(panes.remove(pos) as Arc<dyn Pane>)
            } else {
                None
            }
        }
    }

    #[test]
    fn test_orchestrator_domain_registry() {
        let mut orch = Orchestrator::with_defaults();

        // Initially no domain support
        assert!(!orch.has_domain_support());
        assert!(orch.domain_registry().is_none());
        assert!(orch.default_domain().is_none());

        // Add domain registry
        let registry = Arc::new(DomainRegistry::new());
        let domain = Arc::new(MockDomain::new("test"));
        registry.register(domain.clone());

        orch.set_domain_registry(registry.clone());

        // Now has domain support
        assert!(orch.has_domain_support());
        assert!(orch.domain_registry().is_some());
    }

    #[test]
    fn test_orchestrator_default_domain() {
        let mut orch = Orchestrator::with_defaults();

        // Set default domain directly
        let domain = Arc::new(MockDomain::new("default"));
        orch.set_default_domain(domain.clone());

        assert!(orch.has_domain_support());
        assert!(orch.default_domain().is_some());
        assert_eq!(orch.default_domain().unwrap().domain_name(), "default");
    }

    #[test]
    fn test_terminal_slot_pane_attachment() {
        use crate::agent::TerminalSlot;

        let mut slot = TerminalSlot::new(TerminalSlotId(0));

        // Initially no resources
        assert!(!slot.has_resources());
        assert!(slot.pane().is_none());
        assert!(slot.terminal().is_none());
        assert!(slot.domain_id().is_none());

        // Attach pane
        let domain_id = DomainId::new();
        let pane: Arc<dyn Pane> = Arc::new(MockPane::new(domain_id));
        slot.attach_pane(pane.clone(), domain_id);

        // Now has pane
        assert!(slot.has_resources());
        assert!(slot.pane().is_some());
        assert_eq!(slot.domain_id(), Some(domain_id));

        // Attach terminal
        use crate::terminal::Terminal;
        let terminal = Terminal::new(80, 24);
        slot.attach_terminal(terminal);

        assert!(slot.terminal().is_some());
        assert!(slot.terminal_mut().is_some());

        // Allocate and release clears resources
        slot.allocate(ExecutionId(1)).unwrap();
        assert!(slot.has_resources()); // Still has resources

        slot.release().unwrap();
        assert!(!slot.has_resources()); // Resources cleared
        assert!(slot.pane().is_none());
        assert!(slot.terminal().is_none());
        assert!(slot.domain_id().is_none());
    }

    #[test]
    fn test_terminal_slot_clone_does_not_copy_resources() {
        use crate::agent::TerminalSlot;

        let mut slot = TerminalSlot::new(TerminalSlotId(0));

        // Attach resources
        let domain_id = DomainId::new();
        let pane: Arc<dyn Pane> = Arc::new(MockPane::new(domain_id));
        slot.attach_pane(pane, domain_id);

        use crate::terminal::Terminal;
        slot.attach_terminal(Terminal::new(80, 24));

        // Clone
        let cloned = slot.clone();

        // Cloned slot should NOT have resources
        assert!(!cloned.has_resources());
        assert!(cloned.pane().is_none());
        assert!(cloned.terminal().is_none());

        // But original still has them
        assert!(slot.has_resources());
    }

    #[test]
    fn test_orchestrator_error_spawn_failed() {
        let err = OrchestratorError::SpawnFailed("test error".to_string());
        assert_eq!(err.to_string(), "Pane spawn failed: test error");

        let err = OrchestratorError::NoDomainConfigured;
        assert_eq!(err.to_string(), "No domain configured for pane spawning");
    }

    #[test]
    fn test_domain_error_conversion() {
        let domain_err = DomainError::SpawnFailed("pane creation failed".to_string());
        let orch_err: OrchestratorError = domain_err.into();

        assert!(matches!(orch_err, OrchestratorError::SpawnFailed(_)));
        assert!(orch_err.to_string().contains("pane creation failed"));
    }

    #[test]
    fn test_mock_pane_behavior() {
        let domain_id = DomainId::new();
        let pane = MockPane::new(domain_id);

        // Initial state
        assert!(pane.is_alive());
        assert!(pane.exit_status().is_none());
        assert_eq!(pane.size(), (80, 24));

        // Resize
        pane.resize(120, 40).unwrap();
        assert_eq!(pane.size(), (120, 40));

        // Add output
        pane.add_output(b"Hello, ");
        pane.add_output(b"World!");

        // Read output
        let mut buf = [0u8; 64];
        let len = pane.read(&mut buf).unwrap();
        assert_eq!(&buf[..len], b"Hello, World!");

        // Set exit
        pane.set_exit(0);
        assert!(!pane.is_alive());
        assert_eq!(pane.exit_status(), Some(0));
    }

    #[test]
    fn test_mock_domain_spawn() {
        let domain = MockDomain::new("test");

        // Spawn succeeds by default
        let pane = domain.spawn_pane(80, 24, SpawnConfig::default()).unwrap();
        assert!(pane.is_alive());

        // Can list panes
        assert_eq!(domain.list_panes().len(), 1);

        // Set spawn to fail
        domain.set_spawn_fails(true);
        let result = domain.spawn_pane(80, 24, SpawnConfig::default());
        assert!(result.is_err());
    }
}
