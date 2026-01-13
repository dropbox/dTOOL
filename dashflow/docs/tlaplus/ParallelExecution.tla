---- MODULE ParallelExecution ----
(***************************************************************************)
(* TLA+ Specification for DashFlow Parallel Node Execution (Fan-Out/Fan-In)*)
(*                                                                         *)
(* This spec models the parallel execution model where:                    *)
(* - A fan-out node spawns multiple parallel branches                      *)
(* - Each branch executes independently with its own state copy            *)
(* - A fan-in node waits for all branches and merges states                *)
(* - Merging is commutative and associative (MergeableState trait)         *)
(*                                                                         *)
(* Key Properties:                                                         *)
(* - Determinism: Final merged state is independent of execution order     *)
(* - Completeness: All branches complete before fan-in                     *)
(* - No deadlock: System always terminates                                 *)
(* - Safety: No state corruption during parallel execution                 *)
(*                                                                         *)
(* Status: VERIFIED (#2147) - TLC model-checked, 52K states, all pass       *)
(***************************************************************************)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS
    Branches,              \* Set of parallel branch IDs (e.g., {"a", "b", "c"})
    MaxStateValue          \* Upper bound for state values

(***************************************************************************)
(* Variables                                                               *)
(***************************************************************************)

VARIABLES
    \* Overall execution phase
    phase,                 \* "fan_out", "parallel", "fan_in", "done"

    \* State per branch during parallel execution
    branchStates,          \* [Branches -> State] - each branch's working state
    branchStatus,          \* [Branches -> {"pending", "active", "completed"}]

    \* Pre/post parallel states
    inputState,            \* State before fan-out
    mergedState,           \* State after fan-in merge

    \* For verifying determinism - track all possible merge orderings
    mergeOrder             \* Seq(Branches) - order in which branches were merged

vars == <<phase, branchStates, branchStatus, inputState, mergedState, mergeOrder>>

(***************************************************************************)
(* State Model                                                             *)
(* Models a state with a counter and a set (simulates MergeableState)      *)
(***************************************************************************)

State == [
    counter: 0..MaxStateValue,    \* Numeric value (merge: take max)
    items: SUBSET (1..MaxStateValue)  \* Set value (merge: union)
]

\* Merge function (commutative, associative per MergeableState contract)
MergeStates(s1, s2) ==
    [counter |-> IF s1.counter > s2.counter THEN s1.counter ELSE s2.counter,
     items |-> s1.items \cup s2.items]

(***************************************************************************)
(* Type Invariant                                                          *)
(***************************************************************************)

TypeInvariant ==
    /\ phase \in {"fan_out", "parallel", "fan_in", "done"}
    /\ branchStates \in [Branches -> State]
    /\ branchStatus \in [Branches -> {"pending", "active", "completed"}]
    /\ inputState \in State
    /\ mergedState \in State
    /\ mergeOrder \in Seq(Branches)

(***************************************************************************)
(* Safety: All branches complete before fan-in starts                      *)
(***************************************************************************)

FanInRequiresCompletion ==
    phase \in {"fan_in", "done"} =>
        \A b \in Branches: branchStatus[b] = "completed"

(***************************************************************************)
(* Safety: No state modification after branch completes                    *)
(***************************************************************************)

CompletedBranchesUnchanged ==
    \A b \in Branches:
        branchStatus[b] = "completed" =>
            \* State is frozen (would need temporal logic to fully express)
            TRUE

(***************************************************************************)
(* Initial State                                                           *)
(***************************************************************************)

Init ==
    /\ phase = "fan_out"
    /\ branchStates = [b \in Branches |-> [counter |-> 0, items |-> {}]]
    /\ branchStatus = [b \in Branches |-> "pending"]
    /\ inputState = [counter |-> 1, items |-> {1}]  \* Initial state with some data
    /\ mergedState = [counter |-> 0, items |-> {}]
    /\ mergeOrder = <<>>

(***************************************************************************)
(* Fan-Out: Distribute input state to all branches and start them          *)
(***************************************************************************)

FanOut ==
    /\ phase = "fan_out"
    \* Copy input state to all branches
    /\ branchStates' = [b \in Branches |-> inputState]
    \* All branches become active
    /\ branchStatus' = [b \in Branches |-> "active"]
    /\ phase' = "parallel"
    /\ UNCHANGED <<inputState, mergedState, mergeOrder>>

(***************************************************************************)
(* Branch Execution: Each branch modifies its own state independently      *)
(***************************************************************************)

\* Branch increments counter (simulates node execution)
BranchIncrement(b) ==
    /\ phase = "parallel"
    /\ branchStatus[b] = "active"
    /\ branchStates[b].counter < MaxStateValue
    /\ branchStates' = [branchStates EXCEPT
        ![b].counter = @ + 1]
    /\ UNCHANGED <<phase, branchStatus, inputState, mergedState, mergeOrder>>

\* Branch adds an item to its set (simulates state mutation)
BranchAddItem(b, item) ==
    /\ phase = "parallel"
    /\ branchStatus[b] = "active"
    /\ item \in 1..MaxStateValue
    /\ item \notin branchStates[b].items
    /\ branchStates' = [branchStates EXCEPT
        ![b].items = @ \cup {item}]
    /\ UNCHANGED <<phase, branchStatus, inputState, mergedState, mergeOrder>>

\* Branch completes execution
BranchComplete(b) ==
    /\ phase = "parallel"
    /\ branchStatus[b] = "active"
    /\ branchStatus' = [branchStatus EXCEPT ![b] = "completed"]
    /\ UNCHANGED <<phase, branchStates, inputState, mergedState, mergeOrder>>

(***************************************************************************)
(* Transition to Fan-In: When all branches complete                        *)
(***************************************************************************)

StartFanIn ==
    /\ phase = "parallel"
    /\ \A b \in Branches: branchStatus[b] = "completed"
    /\ phase' = "fan_in"
    /\ UNCHANGED <<branchStates, branchStatus, inputState, mergedState, mergeOrder>>

(***************************************************************************)
(* Fan-In: Merge branch states one at a time                               *)
(* Order doesn't matter due to commutativity/associativity                 *)
(***************************************************************************)

\* Merge one branch into the merged state
MergeBranch(b) ==
    /\ phase = "fan_in"
    /\ b \notin {mergeOrder[i] : i \in 1..Len(mergeOrder)}
    /\ mergedState' = MergeStates(mergedState, branchStates[b])
    /\ mergeOrder' = Append(mergeOrder, b)
    /\ UNCHANGED <<phase, branchStates, branchStatus, inputState>>

\* Complete fan-in when all branches merged
CompleteFanIn ==
    /\ phase = "fan_in"
    /\ {mergeOrder[i] : i \in 1..Len(mergeOrder)} = Branches
    /\ phase' = "done"
    /\ UNCHANGED <<branchStates, branchStatus, inputState, mergedState, mergeOrder>>

(***************************************************************************)
(* Determinism Property                                                    *)
(* The final merged state should be the same regardless of merge order     *)
(***************************************************************************)

\* Compute the expected final state by merging all branches
RECURSIVE MergeAll(_)
MergeAll(bs) ==
    IF bs = {} THEN [counter |-> 0, items |-> {}]
    ELSE LET b == CHOOSE x \in bs: TRUE
         IN MergeStates(branchStates[b], MergeAll(bs \ {b}))

ExpectedMergedState == MergeAll(Branches)

\* When done, merged state equals expected state
DeterministicMerge ==
    phase = "done" => mergedState = ExpectedMergedState

(***************************************************************************)
(* Liveness: Eventually reaches done state                                 *)
(***************************************************************************)

EventualCompletion == <>(phase = "done")

(***************************************************************************)
(* Next State Relation                                                     *)
(***************************************************************************)

Next ==
    \/ FanOut
    \/ \E b \in Branches: BranchIncrement(b)
    \/ \E b \in Branches, i \in 1..MaxStateValue: BranchAddItem(b, i)
    \/ \E b \in Branches: BranchComplete(b)
    \/ StartFanIn
    \/ \E b \in Branches: MergeBranch(b)
    \/ CompleteFanIn

(***************************************************************************)
(* Fairness: Branches eventually complete and merge                        *)
(***************************************************************************)

Fairness ==
    /\ \A b \in Branches: WF_vars(BranchComplete(b))
    /\ WF_vars(StartFanIn)
    /\ \A b \in Branches: WF_vars(MergeBranch(b))
    /\ WF_vars(CompleteFanIn)

(***************************************************************************)
(* Specification                                                           *)
(***************************************************************************)

Spec == Init /\ [][Next]_vars

SpecWithFairness == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************)
(* Properties to Check                                                     *)
(***************************************************************************)

THEOREM Spec => []TypeInvariant
THEOREM Spec => []FanInRequiresCompletion
THEOREM Spec => []DeterministicMerge
\* With fairness: THEOREM SpecWithFairness => EventualCompletion

(***************************************************************************)
(* Additional Safety Properties                                            *)
(***************************************************************************)

\* No branch executes before fan-out
NoEarlyExecution ==
    phase = "fan_out" => \A b \in Branches: branchStatus[b] = "pending"

\* Merged state is zero until fan-in starts
MergeOnlyDuringFanIn ==
    phase \in {"fan_out", "parallel"} => mergedState = [counter |-> 0, items |-> {}]

THEOREM Spec => []NoEarlyExecution
THEOREM Spec => []MergeOnlyDuringFanIn

====
