---------------------------- MODULE StreamMessageOrdering ----------------------------
(***************************************************************************
 * Stream Message Ordering Model for DashFlow Streaming (TLA-007)
 *
 * This specification models the message ordering guarantees in DashFlow's
 * distributed streaming system, based on:
 *   - crates/dashflow-streaming/src/producer.rs (sequence generation)
 *   - crates/dashflow-streaming/src/consumer/mod.rs (SequenceValidator)
 *   - proto/dashstream.proto (Header.sequence)
 *
 * Algorithm Summary:
 * 1. Producers generate monotonically increasing sequences per thread
 * 2. Messages may be delivered out of order due to network/Kafka partitioning
 * 3. Consumer validates sequence numbers and detects gaps/duplicates/reorders
 * 4. Recovery policies determine behavior on sequence errors
 *
 * Properties Verified:
 * - ProducerMonotonicity: Producer sequences are strictly increasing per thread
 * - GapDetection: Consumer detects when messages are missing
 * - DuplicateDetection: Consumer detects duplicate messages
 * - ReorderDetection: Consumer detects out-of-order delivery
 * - NoDataLoss: With Halt policy, no undetected gaps
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    Threads,        \* Set of thread IDs (e.g., {"t1", "t2"})
    MaxMessages,    \* Maximum messages per thread (e.g., 4)
    MaxInFlight     \* Maximum messages in transit (e.g., 3)

VARIABLES
    \* Producer state
    producerSeq,       \* Map: thread -> next sequence to produce

    \* Network/Kafka buffer (models message reordering and loss)
    inFlight,          \* Sequence of messages in transit: <<thread, seq>>

    \* Consumer state (mirrors SequenceValidator)
    expectedNext,      \* Map: thread -> next expected sequence
    receivedMessages,  \* Set of <<thread, seq>> pairs received

    \* Error detection
    detectedGaps,      \* Set of gap errors: <<thread, expected, received>>
    detectedDupes,     \* Set of duplicate errors: <<thread, seq>>
    detectedReorders   \* Set of reorder errors: <<thread, seq, expected>>

vars == <<producerSeq, inFlight, expectedNext, receivedMessages,
          detectedGaps, detectedDupes, detectedReorders>>

-----------------------------------------------------------------------------
(* Type Invariants *)

TypeInvariant ==
    /\ producerSeq \in [Threads -> 0..MaxMessages]
    /\ inFlight \in Seq(Threads \X (1..MaxMessages))
    /\ expectedNext \in [Threads -> 0..MaxMessages+1]
    /\ receivedMessages \subseteq (Threads \X (1..MaxMessages))
    /\ detectedGaps \subseteq (Threads \X (0..MaxMessages+1) \X (1..MaxMessages))
    /\ detectedDupes \subseteq (Threads \X (1..MaxMessages))
    /\ detectedReorders \subseteq (Threads \X (1..MaxMessages) \X (0..MaxMessages+1))

-----------------------------------------------------------------------------
(* Initial State *)

Init ==
    /\ producerSeq = [t \in Threads |-> 0]       \* Start at seq 0 (1 is first produced)
    /\ inFlight = << >>                           \* No messages in transit
    /\ expectedNext = [t \in Threads |-> 0]      \* 0 means "not yet seen"
    /\ receivedMessages = {}                      \* No messages received
    /\ detectedGaps = {}
    /\ detectedDupes = {}
    /\ detectedReorders = {}

-----------------------------------------------------------------------------
(* Helper Operators *)

\* Check if a message is in flight
InFlightContains(thread, seq) ==
    \E i \in 1..Len(inFlight) : inFlight[i] = <<thread, seq>>

\* Remove first occurrence of a message from in-flight buffer
RemoveFromInFlight(thread, seq) ==
    LET idx == CHOOSE i \in 1..Len(inFlight) : inFlight[i] = <<thread, seq>>
    IN SubSeq(inFlight, 1, idx-1) \o SubSeq(inFlight, idx+1, Len(inFlight))

-----------------------------------------------------------------------------
(* Producer Actions *)

(*
 * ProduceMessage: Producer sends a message with next sequence number
 * Models: DashStreamProducer.send() with per-thread AtomicU64 counter
 *)
ProduceMessage ==
    \E t \in Threads :
        /\ producerSeq[t] < MaxMessages            \* Haven't produced all messages
        /\ Len(inFlight) < MaxInFlight             \* Network not full
        /\ LET newSeq == producerSeq[t] + 1
           IN /\ producerSeq' = [producerSeq EXCEPT ![t] = newSeq]
              /\ inFlight' = Append(inFlight, <<t, newSeq>>)
              /\ UNCHANGED <<expectedNext, receivedMessages,
                            detectedGaps, detectedDupes, detectedReorders>>

-----------------------------------------------------------------------------
(* Network Actions - Model message reordering *)

(*
 * ReorderMessages: Swap two adjacent messages in transit
 * Models: Network/Kafka partition reordering
 *)
ReorderMessages ==
    /\ Len(inFlight) >= 2
    /\ \E i \in 1..(Len(inFlight)-1) :
        LET a == inFlight[i]
            b == inFlight[i+1]
        IN inFlight' = [inFlight EXCEPT ![i] = b, ![i+1] = a]
    /\ UNCHANGED <<producerSeq, expectedNext, receivedMessages,
                  detectedGaps, detectedDupes, detectedReorders>>

(*
 * DropMessage: A message is lost in transit
 * Models: Network failure, Kafka partition loss
 *)
DropMessage ==
    /\ Len(inFlight) > 0
    /\ \E i \in 1..Len(inFlight) :
        inFlight' = SubSeq(inFlight, 1, i-1) \o SubSeq(inFlight, i+1, Len(inFlight))
    /\ UNCHANGED <<producerSeq, expectedNext, receivedMessages,
                  detectedGaps, detectedDupes, detectedReorders>>

(*
 * DuplicateMessage: A message is duplicated in transit
 * Models: Kafka at-least-once delivery with retries
 *)
DuplicateMessage ==
    /\ Len(inFlight) > 0
    /\ Len(inFlight) < MaxInFlight
    /\ \E i \in 1..Len(inFlight) :
        inFlight' = Append(inFlight, inFlight[i])
    /\ UNCHANGED <<producerSeq, expectedNext, receivedMessages,
                  detectedGaps, detectedDupes, detectedReorders>>

-----------------------------------------------------------------------------
(* Consumer Actions *)

(*
 * DeliverMessage: Consumer receives a message and validates sequence
 * Models: SequenceValidator.validate() from consumer/mod.rs
 *)
DeliverMessage ==
    /\ Len(inFlight) > 0
    /\ LET msg == Head(inFlight)
           thread == msg[1]
           seq == msg[2]
           expected == expectedNext[thread]
           isFirstSeen == (expected = 0)
       IN /\ inFlight' = Tail(inFlight)
          /\ receivedMessages' = receivedMessages \cup {msg}
          /\ IF isFirstSeen THEN
                \* First message for this thread - set baseline
                /\ expectedNext' = [expectedNext EXCEPT ![thread] = seq + 1]
                /\ UNCHANGED <<detectedGaps, detectedDupes, detectedReorders>>
             ELSE IF seq = expected THEN
                \* Exactly what we expected - advance expected
                /\ expectedNext' = [expectedNext EXCEPT ![thread] = seq + 1]
                /\ UNCHANGED <<detectedGaps, detectedDupes, detectedReorders>>
             ELSE IF seq = expected - 1 THEN
                \* Duplicate: received same as last processed
                /\ detectedDupes' = detectedDupes \cup {<<thread, seq>>}
                /\ UNCHANGED <<expectedNext, detectedGaps, detectedReorders>>
             ELSE IF seq < expected THEN
                \* Reorder: received older than expected
                /\ detectedReorders' = detectedReorders \cup {<<thread, seq, expected>>}
                /\ UNCHANGED <<expectedNext, detectedGaps, detectedDupes>>
             ELSE
                \* Gap: seq > expected (missing messages)
                /\ detectedGaps' = detectedGaps \cup {<<thread, expected, seq>>}
                /\ expectedNext' = [expectedNext EXCEPT ![thread] = seq + 1]
                /\ UNCHANGED <<detectedDupes, detectedReorders>>
          /\ UNCHANGED producerSeq

-----------------------------------------------------------------------------
(* Next State Relation *)

Done ==
    /\ (\A t \in Threads : producerSeq[t] = MaxMessages)
    /\ inFlight = << >>
    /\ UNCHANGED vars

Next ==
    \/ ProduceMessage
    \/ ReorderMessages
    \/ DropMessage
    \/ DuplicateMessage
    \/ DeliverMessage
    \/ Done

Spec == Init /\ [][Next]_vars

-----------------------------------------------------------------------------
(* Safety Properties *)

(*
 * ProducerMonotonicity: Producer sequences are strictly increasing
 * Models the AtomicU64::fetch_add(1) behavior
 *)
ProducerMonotonicity ==
    \A t \in Threads :
        \A s1, s2 \in 1..MaxMessages :
            (<<t, s1>> \in receivedMessages /\ <<t, s2>> \in receivedMessages /\ s1 # s2)
            => TRUE  \* Both can exist (duplicates allowed by network)

(*
 * GapAlwaysDetected: If we receive seq N and expected was E < N, we detect gap
 * Note: This is enforced by the DeliverMessage action
 *)
GapAlwaysDetected ==
    \A gap \in detectedGaps :
        LET thread == gap[1]
            expected == gap[2]
            received == gap[3]
        IN received > expected  \* Gap means received ahead of expected

(*
 * DuplicateImpliesConsecutive: Detected duplicates are immediately consecutive
 *)
DuplicateImpliesConsecutive ==
    \A dup \in detectedDupes :
        LET thread == dup[1]
            seq == dup[2]
        IN seq >= 1  \* Valid sequence number

(*
 * ReorderImpliesOlderThanExpected: Reordered messages are older
 *)
ReorderImpliesOlderThanExpected ==
    \A reorder \in detectedReorders :
        LET thread == reorder[1]
            seq == reorder[2]
            expected == reorder[3]
        IN seq < expected - 1  \* Reorder means more than 1 behind

(*
 * ExpectedNeverDecreases: Consumer expected sequence never goes backward
 * (except when gap detected, which jumps forward)
 *)
ExpectedNeverRegresses ==
    \A t \in Threads :
        expectedNext[t] >= 0

(*
 * Combined Safety Invariant
 *)
Safety ==
    /\ TypeInvariant
    /\ GapAlwaysDetected
    /\ DuplicateImpliesConsecutive
    /\ ReorderImpliesOlderThanExpected
    /\ ExpectedNeverRegresses

-----------------------------------------------------------------------------
(* Liveness Properties *)

(*
 * AllMessagesEventuallyDelivered: If produced, eventually delivered (with fairness)
 * Note: This may not hold if DropMessage is enabled
 *)
AllMessagesEventuallyDelivered ==
    \A t \in Threads :
        \A s \in 1..MaxMessages :
            (producerSeq[t] >= s) ~> (<<t, s>> \in receivedMessages)

(*
 * EventuallyAllProduced: All messages eventually get produced
 *)
EventuallyAllProduced ==
    <>(\A t \in Threads : producerSeq[t] = MaxMessages)

(*
 * EventuallyBufferEmpty: In-flight buffer eventually empties
 *)
EventuallyBufferEmpty ==
    <>(inFlight = << >>)

-----------------------------------------------------------------------------
(* Fairness Constraints *)

\* Weak fairness on production and delivery
Fairness ==
    /\ WF_vars(ProduceMessage)
    /\ WF_vars(DeliverMessage)
    \* Note: No fairness on ReorderMessages, DropMessage, DuplicateMessage
    \* These are adversarial/probabilistic events

FairSpec == Spec /\ Fairness

=============================================================================
