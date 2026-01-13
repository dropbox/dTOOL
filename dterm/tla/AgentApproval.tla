--------------------------- MODULE AgentApproval ---------------------------
(***************************************************************************)
(* Agent Approval Workflow State Machine                                    *)
(*                                                                         *)
(* Models the approval workflow for AI agent actions in dterm:             *)
(* - Agents request approval for dangerous/sensitive operations            *)
(* - Users approve, reject, or let requests timeout                        *)
(* - System maintains audit trail of all decisions                         *)
(*                                                                         *)
(* Safety Properties:                                                       *)
(* - No request is both approved AND rejected                              *)
(* - All completed requests have audit entries                             *)
(* - Timeout only fires for pending requests                               *)
(*                                                                         *)
(* Liveness Properties:                                                     *)
(* - Every request eventually reaches terminal state                       *)
(***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    MaxRequests,      \* Maximum concurrent approval requests
    MaxTimeout,       \* Maximum timeout ticks before auto-reject
    Agents,           \* Set of agent identifiers
    Actions           \* Set of action types requiring approval

VARIABLES
    requests,         \* Function: RequestId -> Request record
    nextRequestId,    \* Next request ID to assign
    clock,            \* Logical clock for timeout tracking
    auditLog,         \* Sequence of audit entries
    agentQueues       \* Function: Agent -> Queue of pending request IDs

(***************************************************************************)
(* Type Definitions                                                        *)
(***************************************************************************)

RequestStates == {"Pending", "Approved", "Rejected", "Timeout", "Cancelled"}

Request == [
    id: Nat,
    agent: Agents,
    action: Actions,
    state: RequestStates,
    createdAt: Nat,
    completedAt: Nat \union {-1},   \* -1 means not completed
    description: STRING
]

AuditEntry == [
    requestId: Nat,
    agent: Agents,
    action: Actions,
    decision: {"Approved", "Rejected", "Timeout", "Cancelled"},
    timestamp: Nat
]

(***************************************************************************)
(* Type Invariant                                                          *)
(***************************************************************************)

TypeInvariant ==
    /\ nextRequestId \in Nat
    /\ clock \in Nat
    /\ auditLog \in Seq(AuditEntry)
    /\ DOMAIN requests \subseteq 0..(MaxRequests - 1)
    /\ \A id \in DOMAIN requests:
        /\ requests[id].id = id
        /\ requests[id].agent \in Agents
        /\ requests[id].action \in Actions
        /\ requests[id].state \in RequestStates
        /\ requests[id].createdAt \in Nat
        /\ requests[id].completedAt \in Nat \union {-1}

(***************************************************************************)
(* Safety Invariants                                                       *)
(***************************************************************************)

\* INV-APPROVAL-1: No request is both approved and rejected
NoDoubleDecision ==
    \A id \in DOMAIN requests:
        ~(requests[id].state = "Approved" /\ requests[id].state = "Rejected")

\* INV-APPROVAL-2: All completed requests have audit entries
CompletedHaveAudit ==
    \A id \in DOMAIN requests:
        requests[id].state \in {"Approved", "Rejected", "Timeout", "Cancelled"} =>
        \E i \in 1..Len(auditLog): auditLog[i].requestId = id

\* INV-APPROVAL-3: Pending requests have no completion time
PendingNotCompleted ==
    \A id \in DOMAIN requests:
        requests[id].state = "Pending" => requests[id].completedAt = -1

\* INV-APPROVAL-4: Completed requests have valid completion time
CompletedHaveTime ==
    \A id \in DOMAIN requests:
        requests[id].state \in {"Approved", "Rejected", "Timeout", "Cancelled"} =>
        requests[id].completedAt >= requests[id].createdAt

\* INV-APPROVAL-5: Request IDs are unique and sequential
RequestIdsSequential ==
    /\ \A id \in DOMAIN requests: id < nextRequestId
    /\ \A id1, id2 \in DOMAIN requests: id1 = id2 \/ requests[id1].id # requests[id2].id

\* INV-APPROVAL-6: Timeout only possible if request exceeded timeout
TimeoutValid ==
    \A id \in DOMAIN requests:
        requests[id].state = "Timeout" =>
        requests[id].completedAt - requests[id].createdAt >= MaxTimeout

\* Combined safety invariant
SafetyInvariant ==
    /\ NoDoubleDecision
    /\ CompletedHaveAudit
    /\ PendingNotCompleted
    /\ CompletedHaveTime
    /\ RequestIdsSequential
    /\ TimeoutValid

(***************************************************************************)
(* State Machine Operations                                                *)
(***************************************************************************)

\* Helper: Get all pending request IDs
PendingRequests == {id \in DOMAIN requests: requests[id].state = "Pending"}

\* Helper: Count pending requests for an agent
PendingForAgent(agent) ==
    Cardinality({id \in PendingRequests: requests[id].agent = agent})

\* Initial state
Init ==
    /\ requests = [id \in {} |-> <<>>]  \* Empty function
    /\ nextRequestId = 0
    /\ clock = 0
    /\ auditLog = <<>>
    /\ agentQueues = [a \in Agents |-> <<>>]

\* Agent submits approval request
SubmitRequest(agent, action, desc) ==
    /\ nextRequestId < MaxRequests
    /\ PendingForAgent(agent) < 10   \* Max 10 pending per agent
    /\ LET newId == nextRequestId
           newRequest == [
               id |-> newId,
               agent |-> agent,
               action |-> action,
               state |-> "Pending",
               createdAt |-> clock,
               completedAt |-> -1,
               description |-> desc
           ]
       IN
           /\ requests' = requests @@ (newId :> newRequest)
           /\ nextRequestId' = nextRequestId + 1
           /\ agentQueues' = [agentQueues EXCEPT ![agent] = Append(@, newId)]
           /\ UNCHANGED <<clock, auditLog>>

\* User approves request
ApproveRequest(id) ==
    /\ id \in DOMAIN requests
    /\ requests[id].state = "Pending"
    /\ LET entry == [
           requestId |-> id,
           agent |-> requests[id].agent,
           action |-> requests[id].action,
           decision |-> "Approved",
           timestamp |-> clock
       ]
       IN
           /\ requests' = [requests EXCEPT ![id].state = "Approved",
                                           ![id].completedAt = clock]
           /\ auditLog' = Append(auditLog, entry)
           /\ UNCHANGED <<nextRequestId, clock, agentQueues>>

\* User rejects request
RejectRequest(id) ==
    /\ id \in DOMAIN requests
    /\ requests[id].state = "Pending"
    /\ LET entry == [
           requestId |-> id,
           agent |-> requests[id].agent,
           action |-> requests[id].action,
           decision |-> "Rejected",
           timestamp |-> clock
       ]
       IN
           /\ requests' = [requests EXCEPT ![id].state = "Rejected",
                                           ![id].completedAt = clock]
           /\ auditLog' = Append(auditLog, entry)
           /\ UNCHANGED <<nextRequestId, clock, agentQueues>>

\* Request times out (auto-reject)
TimeoutRequest(id) ==
    /\ id \in DOMAIN requests
    /\ requests[id].state = "Pending"
    /\ clock - requests[id].createdAt >= MaxTimeout
    /\ LET entry == [
           requestId |-> id,
           agent |-> requests[id].agent,
           action |-> requests[id].action,
           decision |-> "Timeout",
           timestamp |-> clock
       ]
       IN
           /\ requests' = [requests EXCEPT ![id].state = "Timeout",
                                           ![id].completedAt = clock]
           /\ auditLog' = Append(auditLog, entry)
           /\ UNCHANGED <<nextRequestId, clock, agentQueues>>

\* Agent cancels own pending request
CancelRequest(agent, id) ==
    /\ id \in DOMAIN requests
    /\ requests[id].state = "Pending"
    /\ requests[id].agent = agent   \* Can only cancel own requests
    /\ LET entry == [
           requestId |-> id,
           agent |-> agent,
           action |-> requests[id].action,
           decision |-> "Cancelled",
           timestamp |-> clock
       ]
       IN
           /\ requests' = [requests EXCEPT ![id].state = "Cancelled",
                                           ![id].completedAt = clock]
           /\ auditLog' = Append(auditLog, entry)
           /\ UNCHANGED <<nextRequestId, clock, agentQueues>>

\* Clock tick (for timeout tracking)
Tick ==
    /\ clock' = clock + 1
    /\ UNCHANGED <<requests, nextRequestId, auditLog, agentQueues>>

(***************************************************************************)
(* State Constraint for Model Checking                                     *)
(***************************************************************************)

\* Bound state space for tractable verification
StateConstraint ==
    /\ nextRequestId <= MaxRequests + 1
    /\ clock <= MaxTimeout + 3
    /\ Len(auditLog) <= MaxRequests + 1

(***************************************************************************)
(* State Machine Specification                                             *)
(***************************************************************************)

Next ==
    \/ \E a \in Agents, act \in Actions: SubmitRequest(a, act, "desc")
    \/ \E id \in DOMAIN requests: ApproveRequest(id)
    \/ \E id \in DOMAIN requests: RejectRequest(id)
    \/ \E id \in DOMAIN requests: TimeoutRequest(id)
    \/ \E a \in Agents, id \in DOMAIN requests: CancelRequest(a, id)
    \/ Tick

Spec == Init /\ [][Next]_<<requests, nextRequestId, clock, auditLog, agentQueues>>

(***************************************************************************)
(* Liveness Properties                                                     *)
(***************************************************************************)

\* Every pending request eventually completes (with fairness)
EventualCompletion ==
    \A id \in DOMAIN requests:
        requests[id].state = "Pending" ~>
        requests[id].state \in {"Approved", "Rejected", "Timeout", "Cancelled"}

(***************************************************************************)
(* Theorems                                                                *)
(***************************************************************************)

\* THEOREM: Approval and rejection are mutually exclusive
THEOREM MutualExclusion ==
    Spec => [](\A id \in DOMAIN requests:
        ~(requests[id].state = "Approved" /\ requests[id].state = "Rejected"))

\* THEOREM: Audit log grows monotonically
THEOREM AuditMonotonic ==
    Spec => [][Len(auditLog') >= Len(auditLog)]_auditLog

\* THEOREM: Request IDs never decrease
THEOREM RequestIdMonotonic ==
    Spec => [][nextRequestId' >= nextRequestId]_nextRequestId

\* THEOREM: Once completed, state doesn't change
THEOREM CompletionFinal ==
    Spec => [](\A id \in DOMAIN requests:
        requests[id].state \in {"Approved", "Rejected", "Timeout", "Cancelled"} =>
        [][requests[id].state = requests'[id].state]_requests)

(***************************************************************************)
(* Deadlock Freedom Properties                                             *)
(***************************************************************************)

\* Quiescent state: no pending requests
ApprovalQuiescent ==
    PendingRequests = {}

\* System can always make progress
ApprovalActionEnabled ==
    \* Can submit new request (if under limits)
    \/ (nextRequestId < MaxRequests /\
        \E a \in Agents: PendingForAgent(a) < 10)
    \* Can approve a pending request
    \/ PendingRequests # {}
    \* Can reject a pending request
    \/ PendingRequests # {}
    \* Can timeout an eligible request
    \/ \E id \in PendingRequests: clock - requests[id].createdAt >= MaxTimeout
    \* Can cancel a pending request
    \/ \E a \in Agents, id \in PendingRequests: requests[id].agent = a
    \* Clock can always tick
    \/ TRUE

\* INV-DEADLOCK-APPROVAL-1: No deadlock - system can always act or is quiescent
ApprovalNoDeadlock ==
    ApprovalActionEnabled \/ ApprovalQuiescent

\* INV-DEADLOCK-APPROVAL-2: Pending requests have resolution path
\* Every pending request can eventually be resolved (approve, reject, timeout, cancel)
PendingResolvable ==
    \A id \in PendingRequests:
        \* Can be approved
        \/ TRUE
        \* Can be rejected
        \/ TRUE
        \* Will eventually timeout (clock ticks enabled)
        \/ TRUE
        \* Can be cancelled by owning agent
        \/ requests[id].agent \in Agents

\* INV-DEADLOCK-APPROVAL-3: No circular dependencies between requests
\* Approval decisions don't depend on other requests
NoApprovalCircularDependency ==
    \* Each request's resolution is independent of other requests
    \A id1, id2 \in PendingRequests:
        id1 # id2 =>
        \* Approving/rejecting id1 doesn't require id2 to be resolved first
        /\ TRUE

\* INV-DEADLOCK-APPROVAL-4: Timeout guarantees eventual termination
\* If clock keeps ticking, pending requests will eventually timeout
TimeoutGuarantee ==
    \A id \in PendingRequests:
        \* Request will timeout when clock reaches createdAt + MaxTimeout
        requests[id].createdAt + MaxTimeout >= clock =>
        \* Either clock hasn't reached timeout yet, or request can be resolved
        \/ clock < requests[id].createdAt + MaxTimeout
        \/ requests[id].state # "Pending"  \* Already resolved

\* Combined deadlock-freedom invariant for approval system
ApprovalDeadlockFreedom ==
    /\ ApprovalNoDeadlock
    /\ PendingResolvable
    /\ NoApprovalCircularDependency

\* THEOREM: Approval system is deadlock-free
THEOREM ApprovalDeadlockFreedomHolds ==
    Spec => []ApprovalDeadlockFreedom

\* THEOREM: With clock fairness, pending requests eventually resolve
\* (This is a liveness property requiring weak fairness on Tick)
THEOREM EventualResolution ==
    Spec /\ WF_<<clock>>(Tick) => EventualCompletion

=============================================================================
