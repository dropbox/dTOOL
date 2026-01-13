------------------------- MODULE WALAppendOrdering -------------------------
(***************************************************************************
 * TLA+ Specification for DashFlow WAL Append Ordering
 *
 * This specification models the WAL (Write-Ahead Log) append ordering contract:
 * - Writers serialize via mutex (only one writer active at a time)
 * - Events are appended to a segment in total order
 * - Fsync commits buffered events to durable storage
 * - A crash can occur at any time; on recovery, only fsynced events survive
 * - After recovery, the segment order is preserved
 *
 * This maps to:
 * - crates/dashflow/src/wal/writer.rs:
 *     - WALWriter (mutex-protected)
 *     - Segment::write_line() (append + optional fsync)
 *     - SegmentWriter::write_event() (serialized writes)
 *
 * Phase: TLA-005 (Part 30: TLA+ Protocol Verification)
 * Author: Worker #2351
 * Date: 2026-01-03
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

(***************************************************************************
 * CONSTANTS
 ***************************************************************************)
CONSTANTS
    Writers,        \* Set of writer thread identifiers (e.g., {w1, w2})
    MaxEvents,      \* Maximum events to bound state space
    MaxEventId      \* Maximum event ID to bound state space

(***************************************************************************
 * VARIABLES
 ***************************************************************************)
VARIABLES
    status,         \* "running" | "crashed"
    buffer,         \* Sequence of events in memory buffer (not yet durable)
    durable,        \* Sequence of events persisted to disk (survived fsync)
    mutexHolder,    \* Writer currently holding the mutex (NONE if available)
    pendingEvent,   \* Event being prepared by mutex holder (before append)
    nextEventId,    \* Counter for generating unique event IDs
    writeLog        \* Record of all writes attempted (for verification)

vars == <<status, buffer, durable, mutexHolder, pendingEvent, nextEventId, writeLog>>

(***************************************************************************
 * TYPE DEFINITIONS
 ***************************************************************************)

\* Special value indicating no writer holds mutex
NONE == "NONE"

Status == {"running", "crashed"}

\* An event is a record with writer and ID
Event == [writer: Writers, id: 1..MaxEventId]

\* pendingEvent is always a record; id=0 means "no pending event"
PendingEvent == [writer: Writers, id: 0..MaxEventId]

NoPendingEvent ==
    [writer |-> CHOOSE w \in Writers : TRUE, id |-> 0]

TypeInvariant ==
    /\ status \in Status
    /\ buffer \in Seq(Event)
    /\ durable \in Seq(Event)
    /\ mutexHolder \in Writers \cup {NONE}
    /\ pendingEvent \in PendingEvent
    /\ nextEventId \in 1..(MaxEventId + 1)
    /\ writeLog \in Seq(Event)
    /\ Len(buffer) <= MaxEvents
    /\ Len(durable) <= MaxEvents
    /\ Len(writeLog) <= MaxEventId

(***************************************************************************
 * HELPER OPERATORS
 ***************************************************************************)

\* Check if an event sequence is a prefix of another
IsPrefix(s1, s2) ==
    /\ Len(s1) <= Len(s2)
    /\ \A i \in 1..Len(s1) : s1[i] = s2[i]

\* Check if all elements in sequence are unique by ID
AllUniqueIds(s) ==
    \A i, j \in 1..Len(s) : i # j => s[i].id # s[j].id

(***************************************************************************
 * SAFETY INVARIANTS
 ***************************************************************************)

\* Durable events are always a prefix of buffer (no gaps)
DurableIsPrefix ==
    IsPrefix(durable, buffer)

\* All event IDs in buffer are unique
UniqueEventIds ==
    AllUniqueIds(buffer)

\* All event IDs in durable are unique
UniqueDurableIds ==
    AllUniqueIds(durable)

\* The durable log is append-only (order is preserved)
\* After crash/recovery, durable events are still in same order
AppendOnlyDurable ==
    IF Len(durable) <= 1 THEN TRUE
    ELSE \A i \in 1..(Len(durable) - 1) :
        durable[i].id < durable[i + 1].id

\* Mutex provides mutual exclusion
MutexExclusion ==
    (mutexHolder = NONE) <=> (pendingEvent.id = 0)

\* Events written by a single writer appear in order
SingleWriterOrder ==
    \A w \in Writers :
        LET writerEvents == SelectSeq(buffer, LAMBDA e : e.writer = w)
        IN \A i \in 1..(Len(writerEvents) - 1) :
            writerEvents[i].id < writerEvents[i + 1].id

\* Combined safety invariant
Safety ==
    /\ TypeInvariant
    /\ DurableIsPrefix
    /\ UniqueEventIds
    /\ UniqueDurableIds
    /\ AppendOnlyDurable
    /\ MutexExclusion
    /\ SingleWriterOrder

\* State space constraint for tractable model checking
StateConstraint ==
    /\ Len(buffer) <= MaxEvents
    /\ Len(durable) <= MaxEvents
    /\ nextEventId <= MaxEventId + 1

(***************************************************************************
 * INITIAL STATE
 ***************************************************************************)
Init ==
    /\ status = "running"
    /\ buffer = << >>
    /\ durable = << >>
    /\ mutexHolder = NONE
    /\ pendingEvent = NoPendingEvent
    /\ nextEventId = 1
    /\ writeLog = << >>

(***************************************************************************
 * TRANSITIONS
 ***************************************************************************)

\* A writer acquires the mutex
AcquireMutex(w) ==
    /\ status = "running"
    /\ mutexHolder = NONE
    /\ nextEventId <= MaxEventId
    /\ mutexHolder' = w
    /\ pendingEvent' = [writer |-> w, id |-> nextEventId]
    /\ nextEventId' = nextEventId + 1
    /\ UNCHANGED <<status, buffer, durable, writeLog>>

\* The mutex holder appends event to buffer
AppendToBuffer ==
    /\ status = "running"
    /\ mutexHolder # NONE
    /\ pendingEvent.id # 0
    \* Prevent appending the same pending event more than once
    /\ IF Len(buffer) = 0 THEN TRUE ELSE buffer[Len(buffer)] # pendingEvent
    /\ Len(buffer) < MaxEvents
    /\ buffer' = Append(buffer, pendingEvent)
    /\ writeLog' = Append(writeLog, pendingEvent)
    /\ UNCHANGED <<status, durable, mutexHolder, pendingEvent, nextEventId>>

\* The mutex holder releases the mutex (after append)
ReleaseMutex ==
    /\ status = "running"
    /\ mutexHolder # NONE
    /\ pendingEvent.id # 0
    \* Ensure event was appended before release
    /\ Len(buffer) > 0
    /\ buffer[Len(buffer)] = pendingEvent
    /\ mutexHolder' = NONE
    /\ pendingEvent' = NoPendingEvent
    /\ UNCHANGED <<status, buffer, durable, nextEventId, writeLog>>

\* Fsync commits all buffered events to durable storage
Fsync ==
    /\ status = "running"
    /\ mutexHolder = NONE  \* Only fsync when no active writes
    /\ Len(buffer) > Len(durable)  \* Only if there's something to sync
    /\ durable' = buffer
    /\ UNCHANGED <<status, buffer, mutexHolder, pendingEvent, nextEventId, writeLog>>

\* System crashes - lose in-flight work but keep durable data
Crash ==
    /\ status = "running"
    /\ status' = "crashed"
    /\ buffer' = durable  \* Buffer reverts to last durable state
    /\ mutexHolder' = NONE
    /\ pendingEvent' = NoPendingEvent
    \* Keep nextEventId to avoid ID reuse (in real system, derived from durable log)
    /\ UNCHANGED <<durable, nextEventId, writeLog>>

\* System recovers from crash
Recover ==
    /\ status = "crashed"
    /\ status' = "running"
    \* Buffer is restored from durable storage (already done in Crash)
    /\ UNCHANGED <<buffer, durable, mutexHolder, pendingEvent, nextEventId, writeLog>>

(***************************************************************************
 * NEXT STATE RELATION
 ***************************************************************************)
Next ==
    \/ \E w \in Writers : AcquireMutex(w)
    \/ AppendToBuffer
    \/ ReleaseMutex
    \/ Fsync
    \/ Crash
    \/ Recover

Spec ==
    Init /\ [][Next]_vars

(***************************************************************************
 * LIVENESS PROPERTIES (for eventual checking)
 ***************************************************************************)

\* If a write completes and fsync happens, the event is durable
WritesDurable ==
    \* All events in writeLog that were fsynced are in durable
    \A i \in 1..Len(writeLog) :
        writeLog[i] \in {durable[j] : j \in 1..Len(durable)} \/
        writeLog[i] \notin {buffer[j] : j \in 1..Len(buffer)}

(***************************************************************************
 * KEY THEOREMS
 ***************************************************************************)

\* The durable log always contains events in their original write order
THEOREM DurabilityPreservesOrder ==
    Spec => []DurableIsPrefix

\* Event IDs are always unique in the log
THEOREM NoEventIdCollisions ==
    Spec => []UniqueEventIds

\* A single writer's events appear in ID order
THEOREM SingleWriterTotalOrder ==
    Spec => []SingleWriterOrder

=============================================================================
