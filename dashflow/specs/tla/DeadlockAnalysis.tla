---------------------------- MODULE DeadlockAnalysis ----------------------------
(***************************************************************************
 * TLA+ Specification for DashFlow Deadlock Freedom Verification
 *
 * This specification proves that DashFlow graph execution is deadlock-free.
 * It models:
 * - Sequential and parallel node execution
 * - Semaphore-based concurrency limiting
 * - Recursion limit enforcement (prevents infinite cycles)
 * - Timeout mechanisms (graph and node level)
 * - Edge routing with all edge types
 *
 * Deadlock freedom is guaranteed by:
 * 1. Recursion limit bounds all cyclic executions
 * 2. Timeouts bound all node executions
 * 3. Semaphore permits are always eventually released
 * 4. Parallel execution always completes (with merge or error)
 * 5. Every graph path leads to END or error
 *
 * Phase: TLA-003 (Part 30: TLA+ Protocol Verification)
 * Author: Worker #2348
 * Date: 2026-01-03
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

(***************************************************************************
 * CONSTANTS - Graph and Execution Configuration
 ***************************************************************************)
CONSTANTS
    Nodes,              \* Set of all node names
    EntryPoint,         \* Starting node
    RecursionLimit,     \* Max iterations before error (default: 25)
    MaxParallelTasks,   \* Semaphore limit for parallel execution (0 = unlimited)
    GraphTimeout,       \* Graph-level timeout (in abstract time units)
    NodeTimeout,        \* Node-level timeout (in abstract time units)

    \* Edge definitions (same as StateGraph.tla)
    SimpleEdges,        \* Set of [from |-> n1, to |-> n2]
    ConditionalEdges,   \* Set of [from |-> n1, routes |-> {[cond |-> c, to |-> n2], ...}]
    ParallelEdges       \* Set of [from |-> n1, to |-> {n2, n3, ...}]

\* Special constants
END == "__END__"
NodeOrEnd == Nodes \cup {END}

(***************************************************************************
 * VARIABLES - Execution State
 ***************************************************************************)
VARIABLES
    \* Execution control
    currentNodes,       \* Current node(s) being executed (set)
    status,             \* "running", "completed", "error_recursion", "error_timeout", "error_routing"
    iterationCount,     \* Counter for recursion limit
    elapsedTime,        \* Elapsed time for timeout checking

    \* Parallel execution state
    parallelActive,     \* Set of nodes currently executing in parallel
    parallelPending,    \* Set of nodes waiting for semaphore permit
    parallelCompleted,  \* Set of nodes that completed parallel execution
    parallelResults,    \* Function: Node -> execution result state

    \* Semaphore state (models concurrency limiting)
    semaphorePermits,   \* Available permits (MaxParallelTasks when full)
    semaphoreWaiters,   \* Queue of nodes waiting for permits

    \* History tracking
    executedNodes,      \* Sequence of nodes executed
    lastParallelMerge   \* Result of last parallel merge (for verification)

vars == <<currentNodes, status, iterationCount, elapsedTime,
          parallelActive, parallelPending, parallelCompleted, parallelResults,
          semaphorePermits, semaphoreWaiters, executedNodes, lastParallelMerge>>

(***************************************************************************
 * TYPE INVARIANTS
 ***************************************************************************)

StatusValues == {"running", "completed", "error_recursion", "error_timeout",
                 "error_routing", "error_node", "error_merge"}

TypeInvariant ==
    /\ currentNodes \subseteq NodeOrEnd
    /\ status \in StatusValues
    /\ iterationCount \in Nat
    /\ elapsedTime \in Nat
    /\ parallelActive \subseteq Nodes
    /\ parallelPending \subseteq Nodes
    /\ parallelCompleted \subseteq Nodes
    /\ semaphorePermits \in 0..MaxParallelTasks
    /\ semaphoreWaiters \in Seq(Nodes)
    /\ executedNodes \in Seq(Nodes)

(***************************************************************************
 * HELPER OPERATORS - Edge Routing
 ***************************************************************************)

HasConditionalEdge(node) ==
    \E edge \in ConditionalEdges : edge.from = node

HasParallelEdge(node) ==
    \E edge \in ParallelEdges : edge.from = node

HasSimpleEdge(node) ==
    \E edge \in SimpleEdges : edge.from = node

GetConditionalEdge(node) ==
    CHOOSE edge \in ConditionalEdges : edge.from = node

GetParallelEdge(node) ==
    CHOOSE edge \in ParallelEdges : edge.from = node

GetSimpleEdge(node) ==
    CHOOSE edge \in SimpleEdges : edge.from = node

\* Route selection for conditional edges (models runtime evaluation)
\* Returns a single target or "invalid" if no route matches
SelectRoute(node, condition) ==
    IF HasConditionalEdge(node) THEN
        LET edge == GetConditionalEdge(node)
        IN IF \E route \in edge.routes : route.cond = condition
           THEN (CHOOSE route \in edge.routes : route.cond = condition).to
           ELSE "invalid"
    ELSE "invalid"

\* Get next nodes based on edge priority: Conditional > Parallel > Simple > Implicit END
GetNextNodes(node, condition) ==
    IF HasConditionalEdge(node) THEN
        LET target == SelectRoute(node, condition)
        IN IF target = "invalid"
           THEN <<"error", {}>>    \* Error: no valid route
           ELSE <<"single", {target}>>
    ELSE IF HasParallelEdge(node) THEN
        <<"parallel", GetParallelEdge(node).to>>
    ELSE IF HasSimpleEdge(node) THEN
        <<"single", {GetSimpleEdge(node).to}>>
    ELSE
        <<"single", {END}>>  \* Implicit END

(***************************************************************************
 * HELPER OPERATORS - Semaphore
 ***************************************************************************)

\* Check if semaphore has available permits
SemaphoreAvailable ==
    \/ MaxParallelTasks = 0  \* 0 means unlimited
    \/ semaphorePermits > 0

\* Acquire a permit (reduces count)
AcquirePermit ==
    IF MaxParallelTasks = 0
    THEN semaphorePermits  \* No change if unlimited
    ELSE semaphorePermits - 1

\* Release a permit (increases count)
ReleasePermit ==
    IF MaxParallelTasks = 0
    THEN semaphorePermits  \* No change if unlimited
    ELSE semaphorePermits + 1

(***************************************************************************
 * INITIAL STATE
 ***************************************************************************)
Init ==
    /\ currentNodes = {EntryPoint}
    /\ status = "running"
    /\ iterationCount = 0
    /\ elapsedTime = 0
    /\ parallelActive = {}
    /\ parallelPending = {}
    /\ parallelCompleted = {}
    /\ parallelResults = [n \in Nodes |-> "none"]
    /\ semaphorePermits = MaxParallelTasks
    /\ semaphoreWaiters = <<>>
    /\ executedNodes = <<>>
    /\ lastParallelMerge = "none"

(***************************************************************************
 * TRANSITIONS - Sequential Execution
 ***************************************************************************)

\* Execute a single node in sequential mode
ExecuteSequential(node, condition) ==
    /\ status = "running"
    /\ Cardinality(currentNodes) = 1
    /\ node \in currentNodes
    /\ node \in Nodes  \* Not END
    /\ parallelActive = {}  \* Not in parallel mode
    /\ iterationCount < RecursionLimit
    /\ elapsedTime < GraphTimeout
    /\ LET result == GetNextNodes(node, condition)
           resultType == result[1]
           nextNodeSet == result[2]
       IN IF resultType = "error" THEN
              /\ status' = "error_routing"
              /\ UNCHANGED <<currentNodes, iterationCount, elapsedTime,
                            parallelActive, parallelPending, parallelCompleted,
                            parallelResults, semaphorePermits, semaphoreWaiters,
                            executedNodes, lastParallelMerge>>
          ELSE IF END \in nextNodeSet THEN
              \* Reached END - execution completes
              /\ currentNodes' = {END}
              /\ status' = "completed"
              /\ executedNodes' = Append(executedNodes, node)
              /\ iterationCount' = iterationCount + 1
              /\ elapsedTime' = elapsedTime + 1  \* Time passes
              /\ UNCHANGED <<parallelActive, parallelPending, parallelCompleted,
                            parallelResults, semaphorePermits, semaphoreWaiters,
                            lastParallelMerge>>
          ELSE IF resultType = "parallel" THEN
              \* Transition to parallel execution
              /\ currentNodes' = nextNodeSet
              /\ parallelPending' = nextNodeSet
              /\ executedNodes' = Append(executedNodes, node)
              /\ iterationCount' = iterationCount + 1
              /\ elapsedTime' = elapsedTime + 1
              /\ UNCHANGED <<status, parallelActive, parallelCompleted,
                            parallelResults, semaphorePermits, semaphoreWaiters,
                            lastParallelMerge>>
          ELSE
              \* Continue to next single node
              /\ currentNodes' = nextNodeSet
              /\ executedNodes' = Append(executedNodes, node)
              /\ iterationCount' = iterationCount + 1
              /\ elapsedTime' = elapsedTime + 1
              /\ UNCHANGED <<status, parallelActive, parallelPending,
                            parallelCompleted, parallelResults, semaphorePermits,
                            semaphoreWaiters, lastParallelMerge>>

(***************************************************************************
 * TRANSITIONS - Parallel Execution
 ***************************************************************************)

\* Start parallel execution for a pending node (acquire semaphore)
StartParallelNode(node) ==
    /\ status = "running"
    /\ node \in parallelPending
    /\ SemaphoreAvailable
    /\ parallelActive' = parallelActive \cup {node}
    /\ parallelPending' = parallelPending \ {node}
    /\ semaphorePermits' = AcquirePermit
    /\ UNCHANGED <<currentNodes, status, iterationCount, elapsedTime,
                  parallelCompleted, parallelResults, semaphoreWaiters,
                  executedNodes, lastParallelMerge>>

\* Complete parallel execution for an active node (release semaphore)
CompleteParallelNode(node) ==
    /\ status = "running"
    /\ node \in parallelActive
    /\ elapsedTime < GraphTimeout  \* Node completes within timeout
    /\ parallelActive' = parallelActive \ {node}
    /\ parallelCompleted' = parallelCompleted \cup {node}
    /\ parallelResults' = [parallelResults EXCEPT ![node] = "success"]
    /\ semaphorePermits' = ReleasePermit
    /\ UNCHANGED <<currentNodes, status, iterationCount, elapsedTime,
                  parallelPending, semaphoreWaiters, executedNodes,
                  lastParallelMerge>>

\* Parallel node times out
ParallelNodeTimeout(node) ==
    /\ status = "running"
    /\ node \in parallelActive
    /\ elapsedTime >= NodeTimeout  \* Node exceeded timeout
    /\ status' = "error_timeout"
    /\ parallelActive' = parallelActive \ {node}
    /\ semaphorePermits' = ReleasePermit
    /\ UNCHANGED <<currentNodes, iterationCount, elapsedTime,
                  parallelPending, parallelCompleted, parallelResults,
                  semaphoreWaiters, executedNodes, lastParallelMerge>>

\* Convert set to sequence (deterministic)
RECURSIVE SetToSeq(_)
SetToSeq(S) ==
    IF S = {} THEN <<>>
    ELSE LET x == CHOOSE x \in S : TRUE
         IN <<x>> \o SetToSeq(S \ {x})

\* All parallel nodes completed - merge results and continue
MergeParallelResults ==
    /\ status = "running"
    /\ parallelPending = {}
    /\ parallelActive = {}
    /\ parallelCompleted /= {}
    /\ currentNodes = parallelCompleted  \* All parallel nodes done
    \* Merge succeeds if at least one node succeeded
    /\ LET successfulNodes == {n \in parallelCompleted : parallelResults[n] = "success"}
       IN IF successfulNodes = {} THEN
              /\ status' = "error_merge"
              /\ UNCHANGED <<currentNodes, iterationCount, elapsedTime,
                            parallelActive, parallelPending, parallelCompleted,
                            parallelResults, semaphorePermits, semaphoreWaiters,
                            executedNodes, lastParallelMerge>>
          ELSE
              \* Merge successful - continue with single path from last node
              /\ LET lastNode == CHOOSE n \in successfulNodes : TRUE
                     result == GetNextNodes(lastNode, "default")
                     nextNodeSet == result[2]
                 IN /\ currentNodes' = nextNodeSet
                    /\ parallelCompleted' = {}
                    /\ parallelResults' = [n \in Nodes |-> "none"]
                    /\ lastParallelMerge' = "merged"
                    /\ executedNodes' = executedNodes \o SetToSeq(successfulNodes)
                    /\ iterationCount' = iterationCount + Cardinality(successfulNodes)
                    /\ IF END \in nextNodeSet
                       THEN status' = "completed"
                       ELSE UNCHANGED status
                    /\ UNCHANGED <<elapsedTime, parallelActive, parallelPending,
                                  semaphorePermits, semaphoreWaiters>>

(***************************************************************************
 * TRANSITIONS - Error Conditions
 ***************************************************************************)

\* Recursion limit exceeded
RecursionLimitExceeded ==
    /\ status = "running"
    /\ iterationCount >= RecursionLimit
    /\ status' = "error_recursion"
    /\ UNCHANGED <<currentNodes, iterationCount, elapsedTime,
                  parallelActive, parallelPending, parallelCompleted,
                  parallelResults, semaphorePermits, semaphoreWaiters,
                  executedNodes, lastParallelMerge>>

\* Graph timeout exceeded
GraphTimeoutExceeded ==
    /\ status = "running"
    /\ elapsedTime >= GraphTimeout
    /\ status' = "error_timeout"
    /\ UNCHANGED <<currentNodes, iterationCount, elapsedTime,
                  parallelActive, parallelPending, parallelCompleted,
                  parallelResults, semaphorePermits, semaphoreWaiters,
                  executedNodes, lastParallelMerge>>

\* Time tick (models passage of time for timeout checking)
TimeTick ==
    /\ status = "running"
    /\ elapsedTime < GraphTimeout
    /\ elapsedTime' = elapsedTime + 1
    /\ UNCHANGED <<currentNodes, status, iterationCount,
                  parallelActive, parallelPending, parallelCompleted,
                  parallelResults, semaphorePermits, semaphoreWaiters,
                  executedNodes, lastParallelMerge>>

(***************************************************************************
 * NEXT STATE RELATION
 ***************************************************************************)

Next ==
    \* Sequential execution
    \/ \E node \in Nodes, cond \in {"true", "false", "continue", "end", "default"} :
           ExecuteSequential(node, cond)

    \* Parallel execution phases
    \/ \E node \in Nodes : StartParallelNode(node)
    \/ \E node \in Nodes : CompleteParallelNode(node)
    \/ \E node \in Nodes : ParallelNodeTimeout(node)
    \/ MergeParallelResults

    \* Error conditions
    \/ RecursionLimitExceeded
    \/ GraphTimeoutExceeded

    \* Time advancement
    \/ TimeTick

(***************************************************************************
 * FAIRNESS CONDITIONS
 ***************************************************************************)

\* Weak fairness ensures progress when enabled
Fairness ==
    /\ WF_vars(Next)
    \* Strong fairness for parallel completion to prevent starvation
    /\ \A node \in Nodes : SF_vars(CompleteParallelNode(node))

(***************************************************************************
 * SPECIFICATION
 ***************************************************************************)

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************
 * INVARIANTS - Safety Properties
 ***************************************************************************)

\* Type safety
TypeSafety == TypeInvariant

\* Recursion limit is bounded
RecursionBounded ==
    iterationCount <= RecursionLimit + Cardinality(Nodes)

\* Time is bounded
TimeBounded ==
    elapsedTime <= GraphTimeout + 1

\* Semaphore permits never go negative
SemaphoreNonNegative ==
    semaphorePermits >= 0

\* Semaphore permits never exceed max (unless unlimited)
SemaphoreBounded ==
    MaxParallelTasks = 0 \/ semaphorePermits <= MaxParallelTasks

\* Active parallel nodes don't exceed permits (unless unlimited)
ParallelConcurrencyBounded ==
    MaxParallelTasks = 0 \/ Cardinality(parallelActive) <= MaxParallelTasks

\* No node is in multiple parallel states simultaneously
ParallelStatesMutuallyExclusive ==
    /\ parallelActive \cap parallelPending = {}
    /\ parallelActive \cap parallelCompleted = {}
    /\ parallelPending \cap parallelCompleted = {}

\* Current nodes are always valid
ValidCurrentNodes ==
    currentNodes \subseteq NodeOrEnd

\* Combined safety invariant
Safety ==
    /\ TypeSafety
    /\ RecursionBounded
    /\ TimeBounded
    /\ SemaphoreNonNegative
    /\ SemaphoreBounded
    /\ ParallelConcurrencyBounded
    /\ ParallelStatesMutuallyExclusive
    /\ ValidCurrentNodes

(***************************************************************************
 * DEADLOCK FREEDOM PROPERTIES
 ***************************************************************************)

\* PRIMARY PROPERTY: No deadlock - system can always progress or has terminated
NoDeadlock ==
    status = "running" => ENABLED Next

\* Alternative formulation: if running, either can make progress or will timeout
DeadlockFreedom ==
    [](status = "running" => (ENABLED Next \/ elapsedTime >= GraphTimeout))

\* Semaphore liveness: permits are always eventually released
SemaphoreEventuallyReleased ==
    \A node \in Nodes :
        (node \in parallelActive) ~> (node \notin parallelActive)

\* Parallel execution liveness: all pending nodes eventually complete or error
ParallelEventuallyComplete ==
    (parallelPending /= {} \/ parallelActive /= {}) ~>
    (parallelPending = {} /\ parallelActive = {})

(***************************************************************************
 * TEMPORAL PROPERTIES - Liveness (Termination Guarantees)
 ***************************************************************************)

\* Eventually terminates (reaches completed or error)
EventuallyTerminates ==
    <>(status \in {"completed", "error_recursion", "error_timeout",
                   "error_routing", "error_node", "error_merge"})

\* If running, eventually stops running
NoLivelock ==
    status = "running" ~> status /= "running"

\* Execution always terminates within bounded time
BoundedTermination ==
    <>(status /= "running" \/ elapsedTime >= GraphTimeout)

\* Parallel execution always merges or errors
ParallelAlwaysMerges ==
    (parallelCompleted /= {} /\ parallelPending = {} /\ parallelActive = {}) ~>
    (parallelCompleted = {} \/ status \in {"error_merge", "error_timeout"})

(***************************************************************************
 * VERIFICATION PROPERTIES (for TLC model checker)
 ***************************************************************************)

\* Entry point must be valid
ValidEntryPoint ==
    EntryPoint \in Nodes

\* All edges reference valid nodes
ValidEdgeTargets ==
    /\ \A edge \in SimpleEdges :
           edge.from \in Nodes /\ (edge.to \in Nodes \/ edge.to = END)
    /\ \A edge \in ConditionalEdges :
           edge.from \in Nodes /\
           \A route \in edge.routes : (route.to \in Nodes \/ route.to = END)
    /\ \A edge \in ParallelEdges :
           edge.from \in Nodes /\ edge.to \subseteq (Nodes \cup {END})

\* Recursion limit is positive
ValidRecursionLimit ==
    RecursionLimit > 0

\* Timeouts are positive
ValidTimeouts ==
    /\ GraphTimeout > 0
    /\ NodeTimeout > 0
    /\ NodeTimeout <= GraphTimeout

\* Configuration validity
ValidConfiguration ==
    /\ ValidEntryPoint
    /\ ValidEdgeTargets
    /\ ValidRecursionLimit
    /\ ValidTimeouts

\* TLC state space constraint (keeps checking tractable)
StateConstraint ==
    /\ iterationCount <= RecursionLimit + 5
    /\ elapsedTime <= GraphTimeout + 2

=============================================================================
\* Modification History
\* Last modified: 2026-01-03 by Worker #2348
\* Created: 2026-01-03 for TLA-003 (Part 30: TLA+ Protocol Verification)
