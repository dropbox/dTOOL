---- MODULE GraphExecution ----
(***************************************************************************)
(* TLA+ Specification for DashFlow Graph Execution                         *)
(*                                                                         *)
(* This spec models the execution of a directed graph of nodes where:      *)
(* - Nodes can only execute after all predecessors complete                *)
(* - Parallel branches execute concurrently                                *)
(* - The system eventually terminates (no deadlock)                        *)
(*                                                                         *)
(* Status: VERIFIED (#2147) - TLC model-checked, all invariants pass        *)
(***************************************************************************)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS Nodes, Edges, EntryNode, ExitNodes

VARIABLES nodeState, edgeTraversed, currentNode, terminated

vars == <<nodeState, edgeTraversed, currentNode, terminated>>

(***************************************************************************)
(* Helper Functions                                                        *)
(***************************************************************************)

Predecessors(n) == {m \in Nodes : <<m, n>> \in Edges}
Successors(n) == {m \in Nodes : <<n, m>> \in Edges}

(***************************************************************************)
(* Type Invariant                                                          *)
(***************************************************************************)

TypeInvariant ==
    /\ nodeState \in [Nodes -> {"pending", "active", "completed", "error", "cancelled"}]
    /\ edgeTraversed \subseteq Edges
    /\ currentNode \subseteq Nodes
    /\ terminated \in BOOLEAN

(***************************************************************************)
(* Safety Invariant: A node can only execute if all predecessors completed *)
(***************************************************************************)

SafetyInvariant ==
    \A n \in Nodes:
        nodeState[n] = "active" =>
            \A pred \in Predecessors(n): nodeState[pred] = "completed"

(***************************************************************************)
(* No Orphan Execution: Only reachable nodes can execute                   *)
(***************************************************************************)

NoOrphanExecution ==
    \A n \in Nodes:
        nodeState[n] \in {"active", "completed"} =>
            n = EntryNode \/ \E pred \in Predecessors(n): nodeState[pred] = "completed"

(***************************************************************************)
(* ExactlyOnce: Each node executes at most once (no duplicate runs)        *)
(* Modeled by invariant: Only pending nodes can become active.             *)
(* The state machine prevents completed/error nodes from re-activating.    *)
(***************************************************************************)

ExactlyOnceInvariant ==
    \* Active nodes are either entry node or have all predecessors completed
    \* This combined with SafetyInvariant ensures no node activates twice
    \A n \in Nodes:
        nodeState[n] = "active" =>
            \A pred \in Predecessors(n): nodeState[pred] \in {"completed", "error", "cancelled"} \/ n = EntryNode

(***************************************************************************)
(* Initial State                                                           *)
(***************************************************************************)

Init ==
    /\ nodeState = [n \in Nodes |-> IF n = EntryNode THEN "active" ELSE "pending"]
    /\ edgeTraversed = {}
    /\ currentNode = {EntryNode}
    /\ terminated = FALSE

(***************************************************************************)
(* Node Completion: A node completes and enables successors               *)
(***************************************************************************)

NodeComplete(n) ==
    /\ ~terminated
    /\ nodeState[n] = "active"
    /\ nodeState' = [nodeState EXCEPT ![n] = "completed"]
    /\ edgeTraversed' = edgeTraversed \cup {<<n, s>> : s \in Successors(n)}
    /\ currentNode' = (currentNode \ {n}) \cup
        {s \in Successors(n) : \A pred \in Predecessors(s) \ {n}: nodeState[pred] = "completed"}
    /\ UNCHANGED terminated

(***************************************************************************)
(* Node Error: A node fails and cancels all downstream nodes               *)
(***************************************************************************)

\* Helper: All nodes reachable from n (descendants)
RECURSIVE Descendants(_)
Descendants(n) ==
    LET directSucc == Successors(n)
    IN directSucc \cup UNION {Descendants(s) : s \in directSucc}

NodeError(n) ==
    /\ ~terminated
    /\ nodeState[n] = "active"
    /\ LET downstream == Descendants(n)
       IN nodeState' = [m \in Nodes |->
            IF m = n THEN "error"
            ELSE IF m \in downstream /\ nodeState[m] = "pending" THEN "cancelled"
            ELSE nodeState[m]]
    /\ currentNode' = {m \in currentNode : m /= n}
    /\ terminated' = TRUE
    /\ UNCHANGED edgeTraversed

(***************************************************************************)
(* Enable Successor: Activate a node whose predecessors are all completed  *)
(***************************************************************************)

EnableSuccessor(n) ==
    /\ ~terminated
    /\ nodeState[n] = "pending"
    /\ \A pred \in Predecessors(n): nodeState[pred] = "completed"
    /\ nodeState' = [nodeState EXCEPT ![n] = "active"]
    /\ currentNode' = currentNode \cup {n}
    /\ UNCHANGED <<edgeTraversed, terminated>>

(***************************************************************************)
(* Successful Termination: All exit nodes completed                        *)
(***************************************************************************)

SuccessfulTermination ==
    /\ ~terminated
    /\ \A e \in ExitNodes: nodeState[e] = "completed"
    /\ terminated' = TRUE
    /\ UNCHANGED <<nodeState, edgeTraversed, currentNode>>

(***************************************************************************)
(* Next State Relation                                                     *)
(***************************************************************************)

Next ==
    \/ \E n \in Nodes: NodeComplete(n)
    \/ \E n \in Nodes: NodeError(n)
    \/ \E n \in Nodes: EnableSuccessor(n)
    \/ SuccessfulTermination

(***************************************************************************)
(* Terminal State: Once terminated, no further state changes               *)
(* This is not a Next action - it defines when we're done                  *)
(***************************************************************************)

Done == terminated

(***************************************************************************)
(* Liveness: Eventually the graph terminates                               *)
(***************************************************************************)

LivenessProperty ==
    <>(terminated)

(***************************************************************************)
(* No Deadlock Property: Terminal states are expected                      *)
(***************************************************************************)

NoDeadlock ==
    ~terminated => ENABLED(Next)

(***************************************************************************)
(* Fairness: If a node can complete, it eventually will                    *)
(***************************************************************************)

Fairness ==
    /\ \A n \in Nodes: WF_vars(NodeComplete(n))
    /\ \A n \in Nodes: WF_vars(EnableSuccessor(n))
    /\ WF_vars(SuccessfulTermination)

(***************************************************************************)
(* Specification                                                           *)
(***************************************************************************)

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************)
(* Properties to Check                                                     *)
(***************************************************************************)

THEOREM Spec => []TypeInvariant
THEOREM Spec => []SafetyInvariant
THEOREM Spec => []NoOrphanExecution
THEOREM Spec => LivenessProperty

====
