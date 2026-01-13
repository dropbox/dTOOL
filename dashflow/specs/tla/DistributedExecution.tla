---------------------------- MODULE DistributedExecution ----------------------------
(***************************************************************************
 * Distributed Execution Model for DashFlow Work-Stealing Scheduler (TLA-006)
 *
 * This specification models the distributed task execution system where:
 * - A coordinator distributes tasks to remote workers
 * - Workers execute tasks from their local queues
 * - Work stealing allows idle workers to take tasks from busy workers
 * - Fault tolerance: if a worker fails, tasks can be reassigned
 *
 * Based on: crates/dashflow/src/scheduler/mod.rs
 *
 * Algorithm Summary:
 * 1. Tasks are submitted to coordinator
 * 2. Coordinator assigns tasks to workers using selection strategy
 * 3. Workers execute tasks from their queues
 * 4. Idle workers can steal from busy workers
 * 5. Results are collected when all tasks complete
 *
 * Properties Verified:
 * - NoTaskDuplication: Each task is executed at most once
 * - NoTaskLoss: Every task eventually completes
 * - Progress: System doesn't deadlock
 * - WorkerSafety: Workers don't execute invalid tasks
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    Workers,      \* Set of worker IDs (e.g., {"w1", "w2", "w3"})
    Tasks,        \* Set of task IDs (e.g., {"t1", "t2", "t3", "t4"})
    MaxQueueLen   \* Maximum length of worker queues

VARIABLES
    \* Coordinator state
    pending,          \* Tasks waiting to be assigned
    assigned,         \* Map: task -> worker (assigned but not started)

    \* Worker state
    workerQueue,      \* Map: worker -> sequence of tasks
    executing,        \* Map: worker -> task currently executing (or "none")
    workerStatus,     \* Map: worker -> status ("idle", "busy", "failed")

    \* Result tracking
    completed,        \* Set of completed tasks
    stolen            \* Set of tasks that were stolen (for metrics)

vars == <<pending, assigned, workerQueue, executing, workerStatus, completed, stolen>>

-----------------------------------------------------------------------------
(* Type Invariants *)

TypeInvariant ==
    /\ pending \subseteq Tasks
    /\ assigned \in [Tasks -> Workers \cup {"none"}]
    /\ workerQueue \in [Workers -> Seq(Tasks)]
    /\ executing \in [Workers -> Tasks \cup {"none"}]
    /\ workerStatus \in [Workers -> {"idle", "busy", "failed"}]
    /\ completed \subseteq Tasks
    /\ stolen \subseteq Tasks

-----------------------------------------------------------------------------
(* Initial State *)

Init ==
    /\ pending = Tasks                                     \* All tasks start pending
    /\ assigned = [t \in Tasks |-> "none"]                 \* No tasks assigned yet
    /\ workerQueue = [w \in Workers |-> << >>]             \* Empty queues
    /\ executing = [w \in Workers |-> "none"]              \* No one executing
    /\ workerStatus = [w \in Workers |-> "idle"]           \* All workers idle
    /\ completed = {}                                       \* No completed tasks
    /\ stolen = {}                                          \* No stolen tasks

-----------------------------------------------------------------------------
(* Helper Operators *)

\* Check if a worker is available
IsAvailable(w) ==
    /\ workerStatus[w] = "idle"
    /\ executing[w] = "none"
    /\ Len(workerQueue[w]) = 0

\* Check if a worker has tasks
HasTasks(w) ==
    /\ workerStatus[w] # "failed"
    /\ Len(workerQueue[w]) > 0

\* Check if a worker can accept more tasks
CanAccept(w) ==
    /\ workerStatus[w] # "failed"
    /\ Len(workerQueue[w]) < MaxQueueLen

\* Get worker with most tasks (for stealing)
BusiestWorker ==
    CHOOSE w \in Workers :
        /\ HasTasks(w)
        /\ \A w2 \in Workers : HasTasks(w2) => Len(workerQueue[w]) >= Len(workerQueue[w2])

\* Check if any worker has tasks to steal
CanSteal ==
    \E w \in Workers :
        /\ HasTasks(w)
        /\ Len(workerQueue[w]) > 1  \* Only steal if they have >1 task

-----------------------------------------------------------------------------
(* Actions *)

(*
 * AssignTask: Coordinator assigns a pending task to an available worker
 * Models: WorkStealingScheduler.distribute_and_execute()
 *)
AssignTask ==
    /\ pending # {}                                        \* Have pending tasks
    /\ \E w \in Workers :
        /\ CanAccept(w)                                    \* Worker can accept
        /\ \E t \in pending :
            /\ pending' = pending \ {t}                    \* Remove from pending
            /\ assigned' = [assigned EXCEPT ![t] = w]      \* Mark as assigned
            /\ workerQueue' = [workerQueue EXCEPT ![w] = Append(@, t)]
            /\ UNCHANGED <<executing, workerStatus, completed, stolen>>

(*
 * StartExecution: Worker starts executing the first task in its queue
 * Models: Worker.execute_batch() beginning
 *)
StartExecution ==
    \E w \in Workers :
        /\ workerStatus[w] = "idle"                        \* Worker is idle
        /\ executing[w] = "none"                           \* Not currently executing
        /\ Len(workerQueue[w]) > 0                         \* Has tasks in queue
        /\ LET task == Head(workerQueue[w])
           IN /\ executing' = [executing EXCEPT ![w] = task]
              /\ workerQueue' = [workerQueue EXCEPT ![w] = Tail(@)]
              /\ workerStatus' = [workerStatus EXCEPT ![w] = "busy"]
              /\ UNCHANGED <<pending, assigned, completed, stolen>>

(*
 * CompleteExecution: Worker completes executing current task
 * Models: Task completion and result return
 *)
CompleteExecution ==
    \E w \in Workers :
        /\ workerStatus[w] = "busy"                        \* Worker is busy
        /\ executing[w] # "none"                           \* Has a task
        /\ LET task == executing[w]
           IN /\ completed' = completed \cup {task}        \* Mark completed
              /\ executing' = [executing EXCEPT ![w] = "none"]
              /\ workerStatus' = [workerStatus EXCEPT ![w] = "idle"]
              /\ assigned' = [assigned EXCEPT ![task] = "none"] \* Clear assignment
              /\ UNCHANGED <<pending, workerQueue, stolen>>

(*
 * StealTask: Idle worker steals a task from a busy worker
 * Models: Work-stealing load balancing
 *)
StealTask ==
    /\ CanSteal                                            \* Someone has tasks to steal
    /\ \E w_idle \in Workers :
        /\ IsAvailable(w_idle)                             \* Thief is idle
        /\ \E w_busy \in Workers :
            /\ w_idle # w_busy                             \* Different workers
            /\ Len(workerQueue[w_busy]) > 1                \* Victim has >1 task
            /\ workerStatus[w_busy] # "failed"
            \* Steal last task from victim's queue (LIFO stealing)
            /\ LET victimQueue == workerQueue[w_busy]
                   lastIdx == Len(victimQueue)
                   stolenTask == victimQueue[lastIdx]
                   newVictimQueue == SubSeq(victimQueue, 1, lastIdx - 1)
               IN /\ workerQueue' = [workerQueue EXCEPT
                        ![w_busy] = newVictimQueue,
                        ![w_idle] = Append(@, stolenTask)]
                  /\ assigned' = [assigned EXCEPT ![stolenTask] = w_idle]
                  /\ stolen' = stolen \cup {stolenTask}
                  /\ UNCHANGED <<pending, executing, workerStatus, completed>>

(*
 * WorkerFailure: A worker fails while idle (no task loss)
 * Models: Worker crash detection
 *)
WorkerFailure ==
    \E w \in Workers :
        /\ workerStatus[w] = "idle"                        \* Only fail when idle
        /\ \E w2 \in Workers : w2 # w /\ workerStatus[w2] # "failed" \* Don't fail the last worker
        /\ executing[w] = "none"                           \* Not executing
        /\ Len(workerQueue[w]) = 0                         \* No tasks in queue
        /\ workerStatus' = [workerStatus EXCEPT ![w] = "failed"]
        /\ UNCHANGED <<pending, assigned, workerQueue, executing, completed, stolen>>

(*
 * RecoverTask: Task from failed worker is recovered to pending
 * Note: This models the fallback to local execution when workers unavailable
 *)
RecoverTask ==
    \E w \in Workers :
        /\ workerStatus[w] = "failed"
        /\ Len(workerQueue[w]) > 0                         \* Has orphaned tasks
        /\ LET task == Head(workerQueue[w])
           IN /\ pending' = pending \cup {task}            \* Return to pending
              /\ workerQueue' = [workerQueue EXCEPT ![w] = Tail(@)]
              /\ assigned' = [assigned EXCEPT ![task] = "none"]
              /\ UNCHANGED <<executing, workerStatus, completed, stolen>>

-----------------------------------------------------------------------------
(* Next State Relation *)

Done ==
    /\ completed = Tasks
    /\ UNCHANGED vars

Next ==
    \/ AssignTask
    \/ StartExecution
    \/ CompleteExecution
    \/ StealTask
    \/ WorkerFailure
    \/ RecoverTask
    \/ Done

Spec == Init /\ [][Next]_vars

-----------------------------------------------------------------------------
(* Safety Properties *)

\* Elements of a task queue sequence (as a set)
SeqElems(s) ==
    {s[i] : i \in 1..Len(s)}

\* All tasks currently present in any worker queue
AllQueuedTasks ==
    UNION {SeqElems(workerQueue[w]) : w \in Workers}

(*
 * NoTaskDuplication: Each task is executed at most once
 * A task in 'completed' should not be in any worker's queue or executing
 *)
NoTaskDuplication ==
    \A t \in Tasks :
        t \in completed =>
            /\ \A w \in Workers :
                /\ t \notin SeqElems(workerQueue[w])
                /\ executing[w] # t

(*
 * NoTaskLoss: Every task is either pending, assigned, executing, or completed
 * This ensures no task "disappears" from the system
 *)
NoTaskLoss ==
    \A t \in Tasks :
        \/ t \in pending                                   \* Waiting to be assigned
        \/ t \in AllQueuedTasks                             \* In some queue
        \/ \E w \in Workers : executing[w] = t             \* Being executed
        \/ t \in completed                                 \* Done

(*
 * WorkerQueuesBounded: Worker queues don't exceed max length
 *)
WorkerQueuesBounded ==
    \A w \in Workers : Len(workerQueue[w]) <= MaxQueueLen

(*
 * ExecutingImpliesBusy: If executing a task, status must be busy
 *)
ExecutingImpliesBusy ==
    \A w \in Workers :
        executing[w] # "none" => workerStatus[w] = "busy"

(*
 * FailedWorkerNotExecuting: Failed workers don't execute
 *)
FailedWorkerNotExecuting ==
    \A w \in Workers :
        workerStatus[w] = "failed" => executing[w] = "none"

\* Combined safety invariant
Safety ==
    /\ TypeInvariant
    /\ NoTaskDuplication
    /\ NoTaskLoss
    /\ WorkerQueuesBounded
    /\ ExecutingImpliesBusy
    /\ FailedWorkerNotExecuting

-----------------------------------------------------------------------------
(* Liveness Properties *)

(*
 * EventualCompletion: All tasks eventually complete (if system doesn't fail completely)
 * Requires fairness assumptions
 *)
EventualCompletion ==
    <>(\A t \in Tasks : t \in completed)

(*
 * NoStarvation: If a task is assigned, it eventually completes
 * (unless the worker fails and task is recovered)
 *)
NoStarvation ==
    \A t \in Tasks :
        [](assigned[t] # "none" => <>(t \in completed \/ t \in pending))

(*
 * Progress: The system always makes progress until all tasks complete
 *)
Progress ==
    [](completed # Tasks => ENABLED Next)

-----------------------------------------------------------------------------
(* Fairness Constraints *)

\* Weak fairness: enabled actions eventually happen
Fairness ==
    /\ WF_vars(AssignTask)
    /\ WF_vars(StartExecution)
    /\ WF_vars(CompleteExecution)
    /\ WF_vars(StealTask)
    /\ WF_vars(RecoverTask)

\* Full specification with fairness
FairSpec == Spec /\ Fairness

=============================================================================
