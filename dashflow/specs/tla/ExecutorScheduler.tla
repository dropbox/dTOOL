---------------------------- MODULE ExecutorScheduler ----------------------------
(***************************************************************************
 * TLA+ Specification for DashFlow Work-Stealing Scheduler
 *
 * This specification models the executor's parallel task scheduling algorithm.
 * It captures:
 * - Task distribution across workers (local vs remote)
 * - Selection strategies (RoundRobin, LeastLoaded, Random)
 * - Local queue threshold decisions
 * - Work-stealing mechanics
 * - Failure and fallback handling
 *
 * Phase: TLA-002 (Part 30: TLA+ Protocol Verification)
 * Author: Worker #2347
 * Date: 2026-01-03
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

(***************************************************************************
 * CONSTANTS - Scheduler Configuration
 ***************************************************************************)
CONSTANTS
    Tasks,              \* Set of task IDs to execute
    Workers,            \* Set of worker IDs (remote workers)
    LocalQueueThreshold,\* Threshold for local vs distributed execution
    SelectionStrategy,  \* "RoundRobin", "LeastLoaded", or "Random"
    EnableStealing,     \* Whether work-stealing is enabled
    MaxStealAttempts    \* Maximum steal attempts per cycle

(***************************************************************************
 * VARIABLES - Scheduler State
 ***************************************************************************)
VARIABLES
    \* Task states
    taskState,          \* Function: Task -> {"pending", "assigned", "executing", "completed", "failed"}
    taskAssignment,     \* Function: Task -> Worker \cup {"local", "unassigned"}

    \* Worker states
    workerLoad,         \* Function: Worker -> Nat (current load)
    workerQueue,        \* Function: Worker -> Seq(Task) (queued tasks)
    workerAvailable,    \* Function: Worker -> BOOLEAN

    \* Local execution state
    localQueue,         \* Sequence of tasks pending local execution
    localExecuting,     \* Set of tasks currently executing locally

    \* Round-robin state
    roundRobinIndex,    \* Current worker index for round-robin

    \* Metrics
    tasksSubmitted,     \* Total tasks submitted
    tasksExecutedLocal, \* Tasks executed locally
    tasksExecutedRemote,\* Tasks executed on remote workers

    \* Scheduling phase
    phase               \* "submit", "assign", "execute", "complete"

vars == <<taskState, taskAssignment, workerLoad, workerQueue, workerAvailable,
          localQueue, localExecuting, roundRobinIndex, tasksSubmitted,
          tasksExecutedLocal, tasksExecutedRemote, phase>>

(***************************************************************************
 * TYPE INVARIANTS
 ***************************************************************************)

TaskStates == {"pending", "assigned", "executing", "completed", "failed"}
Assignments == Workers \cup {"local", "unassigned"}
Strategies == {"RoundRobin", "LeastLoaded", "Random"}
Phases == {"submit", "assign", "execute", "complete"}

TypeInvariant ==
    /\ taskState \in [Tasks -> TaskStates]
    /\ taskAssignment \in [Tasks -> Assignments]
    /\ workerLoad \in [Workers -> Nat]
    /\ \A w \in Workers : workerQueue[w] \in Seq(Tasks)
    /\ workerAvailable \in [Workers -> BOOLEAN]
    /\ localQueue \in Seq(Tasks)
    /\ localExecuting \subseteq Tasks
    /\ roundRobinIndex \in 0..(Cardinality(Workers))
    /\ tasksSubmitted \in Nat
    /\ tasksExecutedLocal \in Nat
    /\ tasksExecutedRemote \in Nat
    /\ phase \in Phases

(***************************************************************************
 * HELPER OPERATORS
 ***************************************************************************)

\* Range of a sequence (set of all elements)
Range(s) == {s[i] : i \in 1..Len(s)}

\* Count of pending tasks
PendingTasks == {t \in Tasks : taskState[t] = "pending"}

\* Count of completed tasks
CompletedTasks == {t \in Tasks : taskState[t] = "completed"}

\* Get available workers
AvailableWorkers == {w \in Workers : workerAvailable[w]}

\* Check if should distribute (local queue exceeds threshold AND workers available)
ShouldDistribute ==
    /\ Len(localQueue) >= LocalQueueThreshold
    /\ AvailableWorkers /= {}

\* Get worker with minimum load
MinLoadWorker ==
    IF AvailableWorkers = {} THEN "none"
    ELSE CHOOSE w \in AvailableWorkers :
         \A w2 \in AvailableWorkers : workerLoad[w] <= workerLoad[w2]

\* Convert set to sequence (for workers in round-robin)
RECURSIVE SetToSeqHelper(_, _)
SetToSeqHelper(S, acc) ==
    IF S = {} THEN acc
    ELSE LET x == CHOOSE x \in S : TRUE
         IN SetToSeqHelper(S \ {x}, Append(acc, x))

SetToSeq(S) == SetToSeqHelper(S, <<>>)

\* Get worker at index for round-robin (using sequence of workers)
WorkerSeq == SetToSeq(Workers)

GetWorkerRoundRobin(idx) ==
    IF Workers = {} THEN "none"
    ELSE WorkerSeq[(idx % Cardinality(Workers)) + 1]

(***************************************************************************
 * INITIAL STATE
 ***************************************************************************)
Init ==
    /\ taskState = [t \in Tasks |-> "pending"]
    /\ taskAssignment = [t \in Tasks |-> "unassigned"]
    /\ workerLoad = [w \in Workers |-> 0]
    /\ workerQueue = [w \in Workers |-> <<>>]
    /\ workerAvailable = [w \in Workers |-> TRUE]
    /\ localQueue = <<>>
    /\ localExecuting = {}
    /\ roundRobinIndex = 0
    /\ tasksSubmitted = 0
    /\ tasksExecutedLocal = 0
    /\ tasksExecutedRemote = 0
    /\ phase = "submit"

(***************************************************************************
 * TRANSITIONS - Submit Phase
 ***************************************************************************)

\* Submit a pending task to the local queue
SubmitTask(t) ==
    /\ phase = "submit"
    /\ taskState[t] = "pending"
    /\ taskState' = [taskState EXCEPT ![t] = "assigned"]
    /\ localQueue' = Append(localQueue, t)
    /\ tasksSubmitted' = tasksSubmitted + 1
    /\ UNCHANGED <<taskAssignment, workerLoad, workerQueue, workerAvailable,
                   localExecuting, roundRobinIndex, tasksExecutedLocal,
                   tasksExecutedRemote, phase>>

\* All tasks submitted, move to assign phase
FinishSubmit ==
    /\ phase = "submit"
    /\ PendingTasks = {}
    /\ phase' = "assign"
    /\ UNCHANGED <<taskState, taskAssignment, workerLoad, workerQueue,
                   workerAvailable, localQueue, localExecuting, roundRobinIndex,
                   tasksSubmitted, tasksExecutedLocal, tasksExecutedRemote>>

(***************************************************************************
 * TRANSITIONS - Assign Phase (Selection Strategies)
 ***************************************************************************)

\* Assign task using Round-Robin strategy
AssignRoundRobin(t) ==
    /\ phase = "assign"
    /\ taskState[t] = "assigned"
    /\ taskAssignment[t] = "unassigned"
    /\ ShouldDistribute
    /\ LET worker == GetWorkerRoundRobin(roundRobinIndex)
       IN /\ worker /= "none"
          /\ taskAssignment' = [taskAssignment EXCEPT ![t] = worker]
          /\ workerQueue' = [workerQueue EXCEPT ![worker] = Append(@, t)]
          /\ workerLoad' = [workerLoad EXCEPT ![worker] = @ + 1]
          /\ roundRobinIndex' = (roundRobinIndex + 1) % Cardinality(Workers)
          /\ \* Remove from local queue
             LET idx == CHOOSE i \in 1..Len(localQueue) : localQueue[i] = t
             IN localQueue' = SubSeq(localQueue, 1, idx-1) \o SubSeq(localQueue, idx+1, Len(localQueue))
    /\ UNCHANGED <<taskState, workerAvailable, localExecuting, tasksSubmitted,
                   tasksExecutedLocal, tasksExecutedRemote, phase>>

\* Assign task using Least-Loaded strategy
AssignLeastLoaded(t) ==
    /\ phase = "assign"
    /\ taskState[t] = "assigned"
    /\ taskAssignment[t] = "unassigned"
    /\ ShouldDistribute
    /\ LET worker == MinLoadWorker
       IN /\ worker /= "none"
          /\ taskAssignment' = [taskAssignment EXCEPT ![t] = worker]
          /\ workerQueue' = [workerQueue EXCEPT ![worker] = Append(@, t)]
          /\ workerLoad' = [workerLoad EXCEPT ![worker] = @ + 1]
          /\ \* Remove from local queue
             LET idx == CHOOSE i \in 1..Len(localQueue) : localQueue[i] = t
             IN localQueue' = SubSeq(localQueue, 1, idx-1) \o SubSeq(localQueue, idx+1, Len(localQueue))
    /\ UNCHANGED <<taskState, workerAvailable, localExecuting, roundRobinIndex,
                   tasksSubmitted, tasksExecutedLocal, tasksExecutedRemote, phase>>

\* Assign task to local execution (threshold not exceeded or no workers)
AssignLocal(t) ==
    /\ phase = "assign"
    /\ taskState[t] = "assigned"
    /\ taskAssignment[t] = "unassigned"
    /\ ~ShouldDistribute
    /\ taskAssignment' = [taskAssignment EXCEPT ![t] = "local"]
    /\ UNCHANGED <<taskState, workerLoad, workerQueue, workerAvailable,
                   localQueue, localExecuting, roundRobinIndex, tasksSubmitted,
                   tasksExecutedLocal, tasksExecutedRemote, phase>>

\* Assignment complete, move to execute phase
FinishAssign ==
    /\ phase = "assign"
    /\ \A t \in Tasks : taskAssignment[t] /= "unassigned"
    /\ phase' = "execute"
    /\ UNCHANGED <<taskState, taskAssignment, workerLoad, workerQueue,
                   workerAvailable, localQueue, localExecuting, roundRobinIndex,
                   tasksSubmitted, tasksExecutedLocal, tasksExecutedRemote>>

(***************************************************************************
 * TRANSITIONS - Execute Phase
 ***************************************************************************)

\* Start local execution of a task
StartLocalExecution(t) ==
    /\ phase = "execute"
    /\ taskState[t] = "assigned"
    /\ taskAssignment[t] = "local"
    /\ t \in Range(localQueue)
    /\ t \notin localExecuting
    /\ taskState' = [taskState EXCEPT ![t] = "executing"]
    /\ localExecuting' = localExecuting \cup {t}
    /\ UNCHANGED <<taskAssignment, workerLoad, workerQueue, workerAvailable,
                   localQueue, roundRobinIndex, tasksSubmitted,
                   tasksExecutedLocal, tasksExecutedRemote, phase>>

\* Complete local execution
CompleteLocalExecution(t) ==
    /\ phase = "execute"
    /\ taskState[t] = "executing"
    /\ taskAssignment[t] = "local"
    /\ t \in localExecuting
    /\ taskState' = [taskState EXCEPT ![t] = "completed"]
    /\ localExecuting' = localExecuting \ {t}
    /\ tasksExecutedLocal' = tasksExecutedLocal + 1
    /\ \* Remove from local queue
       LET idx == CHOOSE i \in 1..Len(localQueue) : localQueue[i] = t
       IN localQueue' = SubSeq(localQueue, 1, idx-1) \o SubSeq(localQueue, idx+1, Len(localQueue))
    /\ UNCHANGED <<taskAssignment, workerLoad, workerQueue, workerAvailable,
                   roundRobinIndex, tasksSubmitted, tasksExecutedRemote, phase>>

\* Start remote worker execution
StartWorkerExecution(t, w) ==
    /\ phase = "execute"
    /\ taskState[t] = "assigned"
    /\ taskAssignment[t] = w
    /\ w \in Workers
    /\ workerAvailable[w]
    /\ Len(workerQueue[w]) > 0
    /\ Head(workerQueue[w]) = t
    /\ taskState' = [taskState EXCEPT ![t] = "executing"]
    /\ workerQueue' = [workerQueue EXCEPT ![w] = Tail(@)]
    /\ UNCHANGED <<taskAssignment, workerLoad, workerAvailable, localQueue,
                   localExecuting, roundRobinIndex, tasksSubmitted,
                   tasksExecutedLocal, tasksExecutedRemote, phase>>

\* Complete remote worker execution
CompleteWorkerExecution(t, w) ==
    /\ phase = "execute"
    /\ taskState[t] = "executing"
    /\ taskAssignment[t] = w
    /\ w \in Workers
    /\ taskState' = [taskState EXCEPT ![t] = "completed"]
    /\ workerLoad' = [workerLoad EXCEPT ![w] = @ - 1]
    /\ tasksExecutedRemote' = tasksExecutedRemote + 1
    /\ UNCHANGED <<taskAssignment, workerQueue, workerAvailable, localQueue,
                   localExecuting, roundRobinIndex, tasksSubmitted,
                   tasksExecutedLocal, phase>>

\* Worker becomes unavailable (failure simulation)
WorkerFails(w) ==
    /\ phase = "execute"
    /\ workerAvailable[w]
    /\ workerAvailable' = [workerAvailable EXCEPT ![w] = FALSE]
    \* Tasks assigned to this worker need to be re-assigned to local
    /\ LET failedTasks == {t \in Tasks : taskAssignment[t] = w /\ taskState[t] \in {"assigned", "executing"}}
       IN /\ taskState' = [t \in Tasks |->
                           IF t \in failedTasks THEN "assigned"
                           ELSE taskState[t]]
          /\ taskAssignment' = [t \in Tasks |->
                                IF t \in failedTasks THEN "local"
                                ELSE taskAssignment[t]]
          /\ localQueue' = localQueue \o SetToSeq(failedTasks)
    /\ workerLoad' = [workerLoad EXCEPT ![w] = 0]
    /\ workerQueue' = [workerQueue EXCEPT ![w] = <<>>]
    /\ UNCHANGED <<localExecuting, roundRobinIndex, tasksSubmitted,
                   tasksExecutedLocal, tasksExecutedRemote, phase>>

\* Work stealing: idle worker steals from busy worker
WorkSteal(idleWorker, busyWorker, t) ==
    /\ EnableStealing
    /\ phase = "execute"
    /\ workerAvailable[idleWorker]
    /\ workerAvailable[busyWorker]
    /\ idleWorker /= busyWorker
    /\ workerLoad[idleWorker] = 0
    /\ workerLoad[busyWorker] > 1
    /\ t \in Range(workerQueue[busyWorker])
    /\ taskState[t] = "assigned"  \* Not yet executing
    \* Move task from busy to idle worker
    /\ LET idx == CHOOSE i \in 1..Len(workerQueue[busyWorker]) : workerQueue[busyWorker][i] = t
       IN workerQueue' = [workerQueue EXCEPT
                          ![busyWorker] = SubSeq(@, 1, idx-1) \o SubSeq(@, idx+1, Len(@)),
                          ![idleWorker] = Append(@, t)]
    /\ taskAssignment' = [taskAssignment EXCEPT ![t] = idleWorker]
    /\ workerLoad' = [workerLoad EXCEPT ![busyWorker] = @ - 1, ![idleWorker] = @ + 1]
    /\ UNCHANGED <<taskState, workerAvailable, localQueue, localExecuting,
                   roundRobinIndex, tasksSubmitted, tasksExecutedLocal,
                   tasksExecutedRemote, phase>>

\* All tasks complete, move to complete phase
FinishExecute ==
    /\ phase = "execute"
    /\ \A t \in Tasks : taskState[t] = "completed"
    /\ phase' = "complete"
    /\ UNCHANGED <<taskState, taskAssignment, workerLoad, workerQueue,
                   workerAvailable, localQueue, localExecuting, roundRobinIndex,
                   tasksSubmitted, tasksExecutedLocal, tasksExecutedRemote>>

(***************************************************************************
 * NEXT STATE RELATION
 ***************************************************************************)

Next ==
    \* Submit phase
    \/ \E t \in Tasks : SubmitTask(t)
    \/ FinishSubmit

    \* Assign phase - strategy-dependent
    \/ /\ SelectionStrategy = "RoundRobin"
       /\ \E t \in Tasks : AssignRoundRobin(t)
    \/ /\ SelectionStrategy = "LeastLoaded"
       /\ \E t \in Tasks : AssignLeastLoaded(t)
    \/ \E t \in Tasks : AssignLocal(t)
    \/ FinishAssign

    \* Execute phase
    \/ \E t \in Tasks : StartLocalExecution(t)
    \/ \E t \in Tasks : CompleteLocalExecution(t)
    \/ \E t \in Tasks, w \in Workers : StartWorkerExecution(t, w)
    \/ \E t \in Tasks, w \in Workers : CompleteWorkerExecution(t, w)
    \/ \E w \in Workers : WorkerFails(w)
    \/ \E w1, w2 \in Workers, t \in Tasks : WorkSteal(w1, w2, t)
    \/ FinishExecute

(***************************************************************************
 * FAIRNESS CONDITIONS
 ***************************************************************************)

\* Weak fairness on main transitions
Fairness ==
    /\ WF_vars(Next)

(***************************************************************************
 * SPECIFICATION
 ***************************************************************************)

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************
 * INVARIANTS - Safety Properties
 ***************************************************************************)

\* Each task is assigned to at most one location (no double-assignment)
NoDoubleAssignment ==
    \A t \in Tasks :
        \/ taskAssignment[t] = "unassigned"
        \/ taskAssignment[t] = "local"
        \/ taskAssignment[t] \in Workers

\* Tasks in worker queues are assigned to that worker
WorkerQueueConsistency ==
    \A w \in Workers :
        \A i \in 1..Len(workerQueue[w]) :
            taskAssignment[workerQueue[w][i]] = w

\* Local executing tasks are assigned locally
LocalExecutingConsistency ==
    \A t \in localExecuting :
        taskAssignment[t] = "local"

\* Task count invariant
TaskCountInvariant ==
    tasksExecutedLocal + tasksExecutedRemote <= Cardinality(Tasks)

\* No completed task is re-executed
NoReexecution ==
    \A t \in Tasks :
        taskState[t] = "completed" =>
            /\ t \notin localExecuting
            /\ \A w \in Workers : t \notin Range(workerQueue[w])

\* Load matches queue length
LoadConsistency ==
    \A w \in Workers :
        workerLoad[w] >= Len(workerQueue[w])

\* Combined safety invariant
Safety ==
    /\ TypeInvariant
    /\ NoDoubleAssignment
    /\ WorkerQueueConsistency
    /\ LocalExecutingConsistency
    /\ TaskCountInvariant
    /\ NoReexecution

(***************************************************************************
 * TEMPORAL PROPERTIES - Liveness
 ***************************************************************************)

\* All tasks eventually complete
AllTasksComplete ==
    <>(phase = "complete")

\* Every submitted task eventually completes
EventualCompletion ==
    \A t \in Tasks : taskState[t] = "pending" ~> taskState[t] = "completed"

\* No indefinite waiting (if task is assigned, it eventually executes)
NoStarvation ==
    \A t \in Tasks : taskState[t] = "assigned" ~> taskState[t] \in {"executing", "completed"}

\* System eventually terminates
EventualTermination ==
    <>(phase = "complete")

(***************************************************************************
 * VERIFICATION PROPERTIES (for TLC model checker)
 ***************************************************************************)

\* Property: Total executed equals total submitted (accounting for metrics)
ExecutionComplete ==
    phase = "complete" =>
        tasksExecutedLocal + tasksExecutedRemote = Cardinality(Tasks)

\* Property: Fair distribution with Round-Robin (within 1 task)
RoundRobinFairness ==
    /\ SelectionStrategy = "RoundRobin"
    /\ phase = "complete"
    /\ Workers /= {}
    => LET maxLoad == CHOOSE m \in 0..Cardinality(Tasks) :
                      /\ \E w \in Workers : workerLoad[w] = m
                      /\ \A w \in Workers : workerLoad[w] <= m
           minLoad == CHOOSE m \in 0..Cardinality(Tasks) :
                      /\ \E w \in Workers : workerLoad[w] = m
                      /\ \A w \in Workers : workerLoad[w] >= m
       IN maxLoad - minLoad <= 1

\* Property: Least-loaded maintains balance
LeastLoadedBalance ==
    /\ SelectionStrategy = "LeastLoaded"
    /\ phase = "execute"
    /\ AvailableWorkers /= {}
    => LET loads == {workerLoad[w] : w \in AvailableWorkers}
       IN \A l1, l2 \in loads : l1 - l2 <= 2

\* TLC state space constraint (keeps checking tractable)
StateConstraint ==
    /\ \A w \in Workers : workerLoad[w] <= 5
    /\ Len(localQueue) <= 10

=============================================================================
\* Modification History
\* Last modified: 2026-01-03 by Worker #2347
\* Created: 2026-01-03 for TLA-002 (Part 30: TLA+ Protocol Verification)
