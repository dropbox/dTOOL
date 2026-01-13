---------------------------- MODULE StateGraph ----------------------------
(***************************************************************************
 * TLA+ Specification for DashFlow StateGraph Execution
 *
 * This specification models the core execution semantics of DashFlow's
 * graph-based state machine. It captures:
 * - Node execution with state transitions
 * - Edge routing (conditional > parallel > simple priority)
 * - Recursion limit enforcement
 * - Termination guarantees
 *
 * Phase: TLA-001 (Part 30: TLA+ Protocol Verification)
 * Author: Worker #2346
 * Date: 2026-01-03
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

(***************************************************************************
 * CONSTANTS - Graph Structure
 ***************************************************************************)
CONSTANTS
    Nodes,          \* Set of all node names (strings)
    EntryPoint,     \* The starting node (must be in Nodes)
    RecursionLimit, \* Maximum iterations before error (default: 25)

    \* Edge definitions
    SimpleEdges,        \* Set of [from |-> n1, to |-> n2] records
    ConditionalEdges,   \* Set of [from |-> n1, routes |-> {[cond |-> c, to |-> n2], ...}]
    ParallelEdges       \* Set of [from |-> n1, to |-> {n2, n3, ...}]

\* Special END constant - represents graph termination
END == "__END__"

(***************************************************************************
 * VARIABLES - Execution State
 ***************************************************************************)
VARIABLES
    currentNodes,   \* Current node(s) being executed (Set of nodes, or {END})
    graphState,     \* Abstract representation of typed state (opaque in TLA+)
    executedNodes,  \* Sequence of nodes executed so far
    iterationCount, \* Counter for recursion limit enforcement
    status          \* Execution status: "running", "completed", "error"

vars == <<currentNodes, graphState, executedNodes, iterationCount, status>>

(***************************************************************************
 * TYPE INVARIANTS
 ***************************************************************************)

\* All possible node locations (including END)
NodeOrEnd == Nodes \cup {END}

\* Valid status values
Status == {"running", "completed", "error_recursion", "error_no_node"}

\* Type invariant for all variables
TypeInvariant ==
    /\ currentNodes \in SUBSET NodeOrEnd
    /\ currentNodes /= {}
    /\ executedNodes \in Seq(Nodes)
    /\ iterationCount \in Nat
    /\ status \in Status

(***************************************************************************
 * HELPER OPERATORS
 ***************************************************************************)

\* Check if a node has any outgoing conditional edge
HasConditionalEdge(node) ==
    \E edge \in ConditionalEdges : edge.from = node

\* Check if a node has any outgoing parallel edge
HasParallelEdge(node) ==
    \E edge \in ParallelEdges : edge.from = node

\* Check if a node has any outgoing simple edge
HasSimpleEdge(node) ==
    \E edge \in SimpleEdges : edge.from = node

\* Get the conditional edge for a node (if exists)
GetConditionalEdge(node) ==
    CHOOSE edge \in ConditionalEdges : edge.from = node

\* Get the parallel edge for a node (if exists)
GetParallelEdge(node) ==
    CHOOSE edge \in ParallelEdges : edge.from = node

\* Get the simple edge for a node (if exists)
GetSimpleEdge(node) ==
    CHOOSE edge \in SimpleEdges : edge.from = node

\* Find next node set based on edge priority:
\* Priority: Conditional > Parallel > Simple
\* If no edge, implicit {END}
\*
\* Returns:
\* - {} on routing error (no matching conditional route)
\* - {END} on termination
\* - {n} for sequential transition
\* - {n1, n2, ...} for parallel fan-out
NextNodes(node, conditionResult) ==
    IF HasConditionalEdge(node) THEN
        \* Conditional edge: use conditionResult to pick route
        LET edge == GetConditionalEdge(node)
            routes == edge.routes
        IN IF \E route \in routes : route.cond = conditionResult
           THEN { (CHOOSE route \in routes : route.cond = conditionResult).to }
           ELSE {}
    ELSE IF HasParallelEdge(node) THEN
        \* Parallel edge: return set of targets
        GetParallelEdge(node).to
    ELSE IF HasSimpleEdge(node) THEN
        \* Simple edge: single target
        { GetSimpleEdge(node).to }
    ELSE
        \* No edge: implicit END
        { END }

(***************************************************************************
 * INITIAL STATE
 ***************************************************************************)
Init ==
    /\ currentNodes = {EntryPoint}
    /\ graphState = "initial"  \* Abstract placeholder
    /\ executedNodes = <<>>
    /\ iterationCount = 0
    /\ status = "running"

(***************************************************************************
 * TRANSITIONS
 ***************************************************************************)

\* Helper to convert set to sequence (deterministic)
RECURSIVE SetToSeq(_)
SetToSeq(S) ==
    IF S = {} THEN <<>>
    ELSE LET x == CHOOSE x \in S : TRUE
         IN <<x>> \o SetToSeq(S \ {x})

\* Execute a single node and transition to next
\* conditionResult models the result of a conditional edge evaluation
ExecuteSingleNode(node, conditionResult) ==
    /\ status = "running"
    /\ node \in Nodes
    /\ currentNodes = {node}
    /\ iterationCount < RecursionLimit
    /\ LET nextSet == NextNodes(node, conditionResult)
       IN IF nextSet = {} THEN
              /\ status' = "error_no_node"
              /\ UNCHANGED <<currentNodes, graphState, executedNodes, iterationCount>>
          ELSE IF nextSet = {END} THEN
              \* Reached END - execution completes
              /\ executedNodes' = Append(executedNodes, node)
              /\ status' = "completed"
              /\ currentNodes' = {END}
              /\ graphState' = "final"
              /\ iterationCount' = iterationCount + 1
          ELSE
              \* Sequential or parallel transition (nextSet is a node set)
              /\ executedNodes' = Append(executedNodes, node)
              /\ currentNodes' = nextSet
              /\ graphState' = "updated"
              /\ iterationCount' = iterationCount + 1
              /\ UNCHANGED status

\* Execute all parallel nodes and merge to single continuation
ExecuteParallelNodes ==
    /\ status = "running"
    /\ currentNodes \subseteq Nodes
    /\ currentNodes /= {}
    /\ Cardinality(currentNodes) > 1
    /\ iterationCount < RecursionLimit
    \* For simplicity, model parallel as executing all then picking one's continuation
    \* In reality, all execute and results merge
    /\ \E node \in currentNodes :
        LET nextSet == NextNodes(node, "default")
        IN IF nextSet = {END} THEN
               /\ executedNodes' = executedNodes \o SetToSeq(currentNodes)
               /\ status' = "completed"
               /\ currentNodes' = {END}
               /\ graphState' = "merged"
               /\ iterationCount' = iterationCount + Cardinality(currentNodes)
           ELSE IF nextSet = {} THEN
               /\ status' = "error_no_node"
               /\ UNCHANGED <<currentNodes, graphState, executedNodes, iterationCount>>
           ELSE
               /\ executedNodes' = executedNodes \o SetToSeq(currentNodes)
               /\ currentNodes' = nextSet
               /\ graphState' = "merged"
               /\ iterationCount' = iterationCount + Cardinality(currentNodes)
               /\ UNCHANGED status

\* Recursion limit exceeded
RecursionLimitExceeded ==
    /\ status = "running"
    /\ iterationCount >= RecursionLimit
    /\ status' = "error_recursion"
    /\ UNCHANGED <<currentNodes, graphState, executedNodes, iterationCount>>

(***************************************************************************
 * NEXT STATE RELATION
 ***************************************************************************)

\* Main next state: either execute single node, parallel nodes, or hit limit
Next ==
    \/ \E node \in Nodes, cond \in {"true", "false", "continue", "end", "default"} :
           ExecuteSingleNode(node, cond)
    \/ ExecuteParallelNodes
    \/ RecursionLimitExceeded

(***************************************************************************
 * FAIRNESS CONDITIONS
 ***************************************************************************)

\* Weak fairness: if execution can continue, it eventually will
Fairness ==
    /\ WF_vars(Next)

(***************************************************************************
 * SPECIFICATION
 ***************************************************************************)

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************
 * INVARIANTS - Safety Properties
 ***************************************************************************)

\* Recursion limit is always respected
RecursionLimitRespected ==
    iterationCount <= RecursionLimit + 1  \* +1 for final step

\* Current node is always valid or END
ValidCurrentNode ==
    /\ currentNodes \subseteq NodeOrEnd
    /\ currentNodes /= {}

\* Executed nodes are always from valid set
ValidExecutedNodes ==
    \A i \in 1..Len(executedNodes) : executedNodes[i] \in Nodes

\* Combined safety invariant
Safety ==
    /\ TypeInvariant
    /\ RecursionLimitRespected
    /\ ValidCurrentNode
    /\ ValidExecutedNodes

(***************************************************************************
 * TEMPORAL PROPERTIES - Liveness
 ***************************************************************************)

\* Eventually terminates (reaches completed or error)
EventuallyTerminates ==
    <>(status \in {"completed", "error_recursion", "error_no_node"})

\* If status is running, eventually it won't be
NoLivelock ==
    status = "running" ~> status /= "running"

\* Starting from entry point, will eventually reach END or error
ReachesEndOrError ==
    <>(currentNodes = {END} \/ status \in {"error_recursion", "error_no_node"})

(***************************************************************************
 * VERIFICATION PROPERTIES (for TLC model checker)
 ***************************************************************************)

\* Property: No deadlock - the system can always make progress when running
NoDeadlock ==
    status = "running" => ENABLED Next

\* Property: Entry point must be a valid node
ValidEntryPoint ==
    EntryPoint \in Nodes

\* Property: All edges reference valid nodes
ValidEdgeTargets ==
    /\ \A edge \in SimpleEdges :
           edge.from \in Nodes /\ (edge.to \in Nodes \/ edge.to = END)
    /\ \A edge \in ConditionalEdges :
           edge.from \in Nodes /\
           \A route \in edge.routes : (route.to \in Nodes \/ route.to = END)
    /\ \A edge \in ParallelEdges :
           edge.from \in Nodes /\ edge.to \subseteq (Nodes \cup {END})

=============================================================================
\* Modification History
\* Last modified: 2026-01-03 by Worker #2346
\* Created: 2026-01-03 for TLA-001 (Part 30: TLA+ Protocol Verification)
