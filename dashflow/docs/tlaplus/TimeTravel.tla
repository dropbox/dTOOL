---- MODULE TimeTravel ----
(***************************************************************************)
(* TLA+ Specification for DashFlow Time-Travel State Reconstruction        *)
(*                                                                         *)
(* This spec models cursor-based time travel where:                        *)
(* - Cursor moves monotonically forward during execution                   *)
(* - Time travel can jump to any past sequence number                      *)
(* - State reconstruction is deterministic                                 *)
(* - Sequence numbers are never reused                                     *)
(*                                                                         *)
(* Status: DRAFT - Actively model-checked in docs/tlaplus/                 *)
(***************************************************************************)

EXTENDS Naturals, Sequences, FiniteSets

CONSTANTS MaxSeq

VARIABLES
    cursor,           \* Current sequence number
    highWaterMark,    \* Highest sequence ever reached
    stateHistory,     \* Function from seq -> state value
    currentState,     \* Current state value
    mode              \* "executing" or "time_traveling"

vars == <<cursor, highWaterMark, stateHistory, currentState, mode>>

(***************************************************************************)
(* Type Invariant                                                          *)
(***************************************************************************)

TypeInvariant ==
    /\ cursor \in 0..MaxSeq
    /\ highWaterMark \in 0..MaxSeq
    /\ stateHistory \in [0..highWaterMark -> Nat]
    /\ currentState \in Nat
    /\ mode \in {"executing", "time_traveling"}

(***************************************************************************)
(* Cursor Consistency: Cursor never exceeds high water mark                *)
(***************************************************************************)

CursorConsistency ==
    cursor <= highWaterMark

(***************************************************************************)
(* Monotonic High Water Mark: High water mark only increases               *)
(***************************************************************************)

MonotonicHighWaterMarkStep ==
    highWaterMark' >= highWaterMark

MonotonicHighWaterMark ==
    [][MonotonicHighWaterMarkStep]_vars

(***************************************************************************)
(* State Determinism: Same sequence always gives same state                *)
(***************************************************************************)

StateDeterminism ==
    cursor > 0 => currentState = stateHistory[cursor]

(***************************************************************************)
(* No Sequence Reuse: Once a sequence is used, its state is immutable      *)
(***************************************************************************)

NoSequenceReuseStep ==
    \A seq \in 1..highWaterMark:
        seq \in DOMAIN stateHistory => (stateHistory')[seq] = stateHistory[seq]

NoSequenceReuse ==
    [][NoSequenceReuseStep]_vars

(***************************************************************************)
(* Initial State                                                           *)
(***************************************************************************)

Init ==
    /\ cursor = 0
    /\ highWaterMark = 0
    /\ stateHistory = [s \in {0} |-> 0]
    /\ currentState = 0
    /\ mode = "executing"

(***************************************************************************)
(* Execute Step: Move forward and record new state                         *)
(***************************************************************************)

ExecuteStep(delta) ==
    /\ mode = "executing"
    /\ cursor = highWaterMark  \* Can only execute at head
    /\ cursor < MaxSeq
    /\ LET newSeq == cursor + 1
           newState == currentState + delta
       IN /\ cursor' = newSeq
          /\ highWaterMark' = newSeq
          /\ stateHistory' = [s \in 0..newSeq |->
                IF s = newSeq THEN newState ELSE stateHistory[s]]
          /\ currentState' = newState
          /\ UNCHANGED mode

(***************************************************************************)
(* Time Travel: Jump to any past sequence                                  *)
(***************************************************************************)

TimeTravel(targetSeq) ==
    /\ targetSeq \in 1..highWaterMark
    /\ targetSeq # cursor
    /\ cursor' = targetSeq
    /\ currentState' = stateHistory[targetSeq]
    /\ mode' = "time_traveling"
    /\ UNCHANGED <<highWaterMark, stateHistory>>

(***************************************************************************)
(* Resume Execution: Return to head and continue executing                 *)
(***************************************************************************)

ResumeExecution ==
    /\ mode = "time_traveling"
    /\ cursor' = highWaterMark
    /\ currentState' = stateHistory[highWaterMark]
    /\ mode' = "executing"
    /\ UNCHANGED <<highWaterMark, stateHistory>>

(***************************************************************************)
(* Fork From Past: Create new branch from historical point                 *)
(***************************************************************************)

ForkFromPast(delta) ==
    /\ mode = "time_traveling"
    /\ cursor < highWaterMark
    /\ highWaterMark < MaxSeq
    /\ LET newSeq == highWaterMark + 1
           newState == currentState + delta
       IN /\ cursor' = newSeq
          /\ highWaterMark' = newSeq
          /\ stateHistory' = [s \in 0..newSeq |->
                IF s = newSeq THEN newState
                ELSE stateHistory[s]]
          /\ currentState' = newState
          /\ mode' = "executing"

(***************************************************************************)
(* Next State Relation                                                     *)
(***************************************************************************)

Next ==
    \/ \E delta \in 1..10: ExecuteStep(delta)
    \/ \E seq \in 1..MaxSeq: TimeTravel(seq)
    \/ ResumeExecution
    \/ \E delta \in 1..10: ForkFromPast(delta)

(***************************************************************************)
(* Reconstruction Property: Can always reconstruct state at any seq        *)
(***************************************************************************)

ReconstructionProperty ==
    \A seq \in 1..highWaterMark:
        stateHistory[seq] \in Nat

(***************************************************************************)
(* Temporal Properties                                                     *)
(***************************************************************************)

\* If we time travel, we eventually return to executing or stay traveling
EventualProgress ==
    mode = "time_traveling" ~> (mode = "executing" \/ cursor = highWaterMark)

(***************************************************************************)
(* Fairness: Allow progress                                                *)
(***************************************************************************)

Fairness ==
    /\ WF_vars(ResumeExecution)
    /\ \A delta \in 1..10: WF_vars(ExecuteStep(delta))

(***************************************************************************)
(* Specification                                                           *)
(***************************************************************************)

Spec == Init /\ [][Next]_vars /\ Fairness

(***************************************************************************)
(* Properties to Check                                                     *)
(***************************************************************************)

THEOREM Spec => []TypeInvariant
THEOREM Spec => []CursorConsistency
THEOREM Spec => []StateDeterminism
THEOREM Spec => []ReconstructionProperty
THEOREM Spec => MonotonicHighWaterMark

====
