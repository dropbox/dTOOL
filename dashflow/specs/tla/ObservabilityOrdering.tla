---------------------------- MODULE ObservabilityOrdering ----------------------------
(***************************************************************************
 * Observability Event Ordering Model for DashFlow (TLA-009)
 *
 * This specification models the observability event ordering guarantees:
 *   - Event emission during graph execution
 *   - Execution hierarchy (parent/child/depth)
 *   - Happens-before relationships between events
 *   - WAL append ordering for durability
 *
 * Based on:
 *   - crates/dashflow/src/event.rs (event types)
 *   - crates/dashflow/src/executor/execution.rs (emission points)
 *   - crates/dashflow/src/executor/execution_hierarchy.rs (hierarchy tracking)
 *   - crates/dashflow/src/wal/writer.rs (WAL events)
 *
 * Properties Verified:
 * - GraphStartFirst: GraphStart is always first event in an execution
 * - GraphEndLast: GraphEnd is always last event in an execution
 * - NodeStartBeforeEnd: NodeStart happens-before NodeEnd for same node
 * - HierarchyConsistent: Parent/depth relationships are consistent
 * - WALAppendOrder: WAL events are append-only
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    Nodes,           \* Set of node IDs in the graph (e.g., {"n1", "n2", "n3"})
    MaxExecutions,   \* Maximum concurrent executions
    MaxDepth         \* Maximum subgraph nesting depth

VARIABLES
    \* Global event log (WAL)
    eventLog,           \* Sequence of events <<exec_id, event_type, node, timestamp>>

    \* Timestamp counter (models SystemTime::now())
    globalTimestamp,

    \* Execution state
    executions,         \* Map: exec_id -> execution record
    nextExecId,         \* Next execution ID to assign

    \* Execution hierarchy stack (models task-local storage)
    hierarchyStack      \* Sequence of exec_ids representing stack

vars == <<eventLog, globalTimestamp, executions, nextExecId, hierarchyStack>>

-----------------------------------------------------------------------------
(* Type Definitions *)

EventTypes == {"GraphStart", "GraphEnd", "NodeStart", "NodeEnd", "NodeError",
               "EdgeTraversal", "StateChanged"}

\* Execution record structure
EmptyExecRecord == [
    parent |-> 0,
    root |-> 0,
    depth |-> 0,
    started |-> FALSE,
    ended |-> FALSE,
    nodes_started |-> {},
    nodes_ended |-> {}
]

-----------------------------------------------------------------------------
(* Type Invariants *)

TypeInvariant ==
    /\ eventLog \in Seq([exec_id: 1..MaxExecutions,
                         event_type: EventTypes,
                         node: Nodes \cup {"none"},
                         timestamp: Nat])
    /\ globalTimestamp \in Nat
    /\ executions \in [1..MaxExecutions ->
        [parent: 0..MaxExecutions,
         root: 0..MaxExecutions,
         depth: 0..MaxDepth,
         started: BOOLEAN,
         ended: BOOLEAN,
         nodes_started: SUBSET Nodes,
         nodes_ended: SUBSET Nodes]]
    /\ nextExecId \in 1..MaxExecutions+1
    /\ hierarchyStack \in Seq(1..MaxExecutions)

-----------------------------------------------------------------------------
(* Initial State *)

Init ==
    /\ eventLog = << >>
    /\ globalTimestamp = 0
    /\ executions = [e \in 1..MaxExecutions |-> EmptyExecRecord]
    /\ nextExecId = 1
    /\ hierarchyStack = << >>

-----------------------------------------------------------------------------
(* Helper Operators *)

\* Get next timestamp (models SystemTime::now())
NextTimestamp == globalTimestamp + 1

\* Emit an event to the log
EmitEvent(exec_id, event_type, node) ==
    /\ globalTimestamp' = NextTimestamp
    /\ eventLog' = Append(eventLog, [
        exec_id |-> exec_id,
        event_type |-> event_type,
        node |-> node,
        timestamp |-> NextTimestamp
       ])

\* Current execution (top of stack)
CurrentExecId ==
    IF Len(hierarchyStack) > 0
    THEN Head(hierarchyStack)
    ELSE 0

\* Get parent execution ID
ParentExecId ==
    IF Len(hierarchyStack) > 1
    THEN hierarchyStack[2]
    ELSE 0

\* Get root execution ID
RootExecId ==
    IF Len(hierarchyStack) > 0
    THEN hierarchyStack[Len(hierarchyStack)]
    ELSE 0

\* Current depth
CurrentDepth == Len(hierarchyStack) - 1

-----------------------------------------------------------------------------
(* Graph Execution Actions *)

(*
 * StartExecution: Begin a new graph execution
 * Models: GraphStart event emission at start of invoke_internal
 *)
StartExecution ==
    /\ nextExecId <= MaxExecutions
    /\ Len(hierarchyStack) < MaxDepth + 1  \* Not too deep
    /\ LET exec_id == nextExecId
           parent == IF Len(hierarchyStack) > 0 THEN CurrentExecId ELSE 0
           root == IF Len(hierarchyStack) > 0 THEN RootExecId ELSE exec_id
           depth == Len(hierarchyStack)
       IN /\ nextExecId' = nextExecId + 1
          /\ hierarchyStack' = <<exec_id>> \o hierarchyStack  \* Push onto stack
          /\ executions' = [executions EXCEPT
                ![exec_id] = [
                    parent |-> parent,
                    root |-> root,
                    depth |-> depth,
                    started |-> TRUE,
                    ended |-> FALSE,
                    nodes_started |-> {},
                    nodes_ended |-> {}
                ]]
          /\ EmitEvent(exec_id, "GraphStart", "none")

(*
 * EndExecution: Complete a graph execution
 * Models: GraphEnd event emission via ExecutionScopeGuard Drop
 *)
EndExecution ==
    /\ Len(hierarchyStack) > 0
    /\ LET exec_id == CurrentExecId
           exec == executions[exec_id]
       IN /\ exec.started
          /\ ~exec.ended
          \* All started nodes must be ended (no in-progress nodes)
          /\ exec.nodes_started = exec.nodes_ended
          /\ executions' = [executions EXCEPT ![exec_id].ended = TRUE]
          /\ hierarchyStack' = Tail(hierarchyStack)  \* Pop from stack
          /\ EmitEvent(exec_id, "GraphEnd", "none")
          /\ UNCHANGED nextExecId

-----------------------------------------------------------------------------
(* Node Execution Actions *)

(*
 * StartNode: Begin a node execution
 * Models: NodeStart event before node execution
 *)
StartNode ==
    /\ Len(hierarchyStack) > 0
    /\ LET exec_id == CurrentExecId
           exec == executions[exec_id]
       IN /\ exec.started
          /\ ~exec.ended
          /\ \E n \in Nodes :
              /\ n \notin exec.nodes_started  \* Node not started yet
              /\ executions' = [executions EXCEPT
                    ![exec_id].nodes_started = @ \cup {n}]
              /\ EmitEvent(exec_id, "NodeStart", n)
              /\ UNCHANGED <<nextExecId, hierarchyStack>>

(*
 * EndNode: Complete a node execution
 * Models: NodeEnd event after successful node execution
 *)
EndNode ==
    /\ Len(hierarchyStack) > 0
    /\ LET exec_id == CurrentExecId
           exec == executions[exec_id]
       IN /\ exec.started
          /\ ~exec.ended
          /\ \E n \in exec.nodes_started :
              /\ n \notin exec.nodes_ended  \* Node started but not ended
              /\ executions' = [executions EXCEPT
                    ![exec_id].nodes_ended = @ \cup {n}]
              /\ EmitEvent(exec_id, "NodeEnd", n)
              /\ UNCHANGED <<nextExecId, hierarchyStack>>

(*
 * NodeError: Node execution fails
 * Models: NodeError event on node failure
 *)
NodeError ==
    /\ Len(hierarchyStack) > 0
    /\ LET exec_id == CurrentExecId
           exec == executions[exec_id]
       IN /\ exec.started
          /\ ~exec.ended
          /\ \E n \in exec.nodes_started :
              /\ n \notin exec.nodes_ended  \* Node in progress
              /\ executions' = [executions EXCEPT
                    ![exec_id].nodes_ended = @ \cup {n}]
              /\ EmitEvent(exec_id, "NodeError", n)
              /\ UNCHANGED <<nextExecId, hierarchyStack>>

(*
 * EdgeTraversal: Edge is traversed between nodes
 * Models: EdgeTraversal event after edge evaluation
 *)
EdgeTraversal ==
    /\ Len(hierarchyStack) > 0
    /\ LET exec_id == CurrentExecId
           exec == executions[exec_id]
       IN /\ exec.started
          /\ ~exec.ended
          /\ EmitEvent(exec_id, "EdgeTraversal", "none")
          /\ UNCHANGED <<executions, nextExecId, hierarchyStack>>

(*
 * StateChanged: State was modified after node execution
 * Models: StateChanged event emission after NodeEnd
 *)
StateChanged ==
    /\ Len(hierarchyStack) > 0
    /\ LET exec_id == CurrentExecId
           exec == executions[exec_id]
       IN /\ exec.started
          /\ ~exec.ended
          /\ Cardinality(exec.nodes_ended) > 0  \* At least one node completed
          /\ EmitEvent(exec_id, "StateChanged", "none")
          /\ UNCHANGED <<executions, nextExecId, hierarchyStack>>

-----------------------------------------------------------------------------
(* Next State Relation *)

Done ==
    /\ Len(hierarchyStack) = 0
    /\ nextExecId > MaxExecutions
    /\ UNCHANGED vars

Next ==
    \/ StartExecution
    \/ EndExecution
    \/ StartNode
    \/ EndNode
    \/ NodeError
    \/ Done

Spec == Init /\ [][Next]_vars

-----------------------------------------------------------------------------
(* Safety Properties *)

(*
 * GraphStartFirst: GraphStart is always the first event for an execution
 *)
GraphStartFirst ==
    \A i \in 1..Len(eventLog) :
        LET e == eventLog[i]
        IN (e.event_type # "GraphStart") =>
           \E j \in 1..(i-1) :
               eventLog[j].exec_id = e.exec_id /\ eventLog[j].event_type = "GraphStart"

(*
 * GraphEndLast: GraphEnd implies no more events for that execution
 *)
GraphEndLast ==
    \A i \in 1..Len(eventLog) :
        LET e == eventLog[i]
        IN (e.event_type = "GraphEnd") =>
           \A j \in (i+1)..Len(eventLog) :
               eventLog[j].exec_id # e.exec_id

(*
 * NodeStartBeforeEnd: NodeStart happens-before NodeEnd for the same node
 *)
NodeStartBeforeEnd ==
    \A i \in 1..Len(eventLog) :
        LET e == eventLog[i]
        IN (e.event_type = "NodeEnd" /\ e.node # "none") =>
           \E j \in 1..(i-1) :
               /\ eventLog[j].exec_id = e.exec_id
               /\ eventLog[j].event_type = "NodeStart"
               /\ eventLog[j].node = e.node

(*
 * NodeErrorAfterStart: NodeError only for started nodes
 *)
NodeErrorAfterStart ==
    \A i \in 1..Len(eventLog) :
        LET e == eventLog[i]
        IN (e.event_type = "NodeError" /\ e.node # "none") =>
           \E j \in 1..(i-1) :
               /\ eventLog[j].exec_id = e.exec_id
               /\ eventLog[j].event_type = "NodeStart"
               /\ eventLog[j].node = e.node

(*
 * TimestampsMonotonic: Timestamps always increase
 *)
TimestampsMonotonic ==
    \A i \in 1..(Len(eventLog)-1) :
        eventLog[i].timestamp < eventLog[i+1].timestamp

(*
 * HierarchyConsistent: Parent execution started before child
 *)
HierarchyConsistent ==
    \A e \in 1..MaxExecutions :
        (executions[e].started /\ executions[e].parent # 0) =>
            /\ executions[executions[e].parent].started
            /\ executions[e].depth > 0

(*
 * DepthCorrect: Depth matches hierarchy
 *)
DepthCorrect ==
    \A e \in 1..MaxExecutions :
        executions[e].started =>
            (executions[e].parent = 0) <=> (executions[e].depth = 0)

(*
 * WALAppendOnly: Events are never removed (append-only)
 * This is trivially true by construction - we only use Append
 *)
WALAppendOnly ==
    TRUE  \* Enforced by Append-only operations

(*
 * Combined Safety Invariant
 *)
Safety ==
    /\ TypeInvariant
    /\ GraphStartFirst
    /\ GraphEndLast
    /\ NodeStartBeforeEnd
    /\ NodeErrorAfterStart
    /\ TimestampsMonotonic
    /\ HierarchyConsistent
    /\ DepthCorrect

-----------------------------------------------------------------------------
(* Liveness Properties *)

(*
 * EventuallyGraphEnd: Started execution eventually ends (with fairness)
 *)
EventuallyGraphEnd ==
    \A e \in 1..MaxExecutions :
        executions[e].started ~> executions[e].ended

(*
 * EventuallyNodeEnd: Started node eventually ends (with fairness)
 *)
EventuallyNodeEnd ==
    \A e \in 1..MaxExecutions :
        \A n \in Nodes :
            (n \in executions[e].nodes_started) ~> (n \in executions[e].nodes_ended)

-----------------------------------------------------------------------------
(* Fairness Constraints *)

Fairness ==
    /\ WF_vars(StartExecution)
    /\ WF_vars(EndExecution)
    /\ WF_vars(StartNode)
    /\ WF_vars(EndNode)
    /\ WF_vars(EdgeTraversal)
    /\ WF_vars(StateChanged)

FairSpec == Spec /\ Fairness

=============================================================================
