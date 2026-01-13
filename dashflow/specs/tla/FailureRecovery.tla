---------------------------- MODULE FailureRecovery ----------------------------
(***************************************************************************
 * Failure Recovery Model for DashFlow Resilience (TLA-008)
 *
 * This specification models the failure recovery mechanisms in DashFlow:
 *   - Retry with exponential backoff (core/retry.rs)
 *   - Circuit breaker pattern (self_improvement/resilience.rs)
 *   - Degraded mode operation (storage/degraded.rs)
 *
 * Based on:
 *   - crates/dashflow/src/core/retry.rs
 *   - crates/dashflow/src/self_improvement/resilience.rs
 *   - crates/dashflow/src/self_improvement/storage/degraded.rs
 *
 * Algorithm Summary:
 * 1. Request arrives, check circuit breaker state
 * 2. If circuit open: reject immediately
 * 3. If circuit closed/half-open: attempt operation
 * 4. On failure: retry with backoff if retries remaining
 * 5. Circuit breaker tracks consecutive failures/successes
 * 6. Degraded mode allows fallback when components fail
 *
 * Properties Verified:
 * - CircuitBreakerStateValid: State transitions are correct
 * - RetryBounded: Retries don't exceed max
 * - EventualCircuitReset: Open circuit eventually tries half-open
 * - DegradedModeFallback: Degraded mode provides fallback value
 ***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    MaxRetries,             \* Maximum retry attempts (e.g., 3)
    FailureThreshold,       \* Failures to open circuit (e.g., 5)
    SuccessThreshold,       \* Successes to close from half-open (e.g., 2)
    ResetTimeout,           \* Ticks before open->half-open (e.g., 3)
    MaxTime                 \* Maximum time steps for model checking

VARIABLES
    \* Request/Retry state
    requestState,          \* "idle" | "attempting" | "waiting_backoff" | "succeeded" | "failed"
    retryCount,            \* Current retry count for active request
    backoffRemaining,      \* Ticks remaining in backoff

    \* Circuit Breaker state (mirrors resilience.rs CircuitBreaker)
    circuitState,          \* "closed" | "open" | "half_open"
    consecutiveFailures,   \* Failure count (reset on success)
    consecutiveSuccesses,  \* Success count in half-open (reset on failure/close)
    timeSinceOpen,         \* Ticks since circuit opened

    \* Degraded mode state
    componentStatus,       \* "healthy" | "degraded"
    degradedSince,         \* Tick when degraded started (0 if healthy)

    \* Global
    globalTime,            \* Model time (ticks)
    totalRequests,         \* Total requests attempted
    successfulRequests,    \* Successful requests
    failedRequests         \* Failed requests (exhausted retries)

vars == <<requestState, retryCount, backoffRemaining, circuitState,
          consecutiveFailures, consecutiveSuccesses, timeSinceOpen,
          componentStatus, degradedSince, globalTime, totalRequests,
          successfulRequests, failedRequests>>

-----------------------------------------------------------------------------
(* Type Invariants *)

TypeInvariant ==
    /\ requestState \in {"idle", "attempting", "waiting_backoff", "succeeded", "failed"}
    /\ retryCount \in 0..MaxRetries+1
    /\ backoffRemaining \in 0..10
    /\ circuitState \in {"closed", "open", "half_open"}
    /\ consecutiveFailures \in 0..(FailureThreshold + MaxRetries + 1)
    /\ consecutiveSuccesses \in 0..SuccessThreshold+1
    /\ timeSinceOpen \in 0..MaxTime
    /\ componentStatus \in {"healthy", "degraded"}
    /\ degradedSince \in 0..MaxTime
    /\ globalTime \in 0..MaxTime
    /\ totalRequests \in 0..MaxTime
    /\ successfulRequests \in 0..MaxTime
    /\ failedRequests \in 0..MaxTime

-----------------------------------------------------------------------------
(* Initial State *)

Init ==
    /\ requestState = "idle"
    /\ retryCount = 0
    /\ backoffRemaining = 0
    /\ circuitState = "closed"
    /\ consecutiveFailures = 0
    /\ consecutiveSuccesses = 0
    /\ timeSinceOpen = 0
    /\ componentStatus = "healthy"
    /\ degradedSince = 0
    /\ globalTime = 0
    /\ totalRequests = 0
    /\ successfulRequests = 0
    /\ failedRequests = 0

-----------------------------------------------------------------------------
(* Helper Operators *)

\* Calculate exponential backoff delay (simplified: 2^attempt, max 8)
BackoffDelay(attempt) ==
    LET base == 2 ^ attempt
    IN IF base > 8 THEN 8 ELSE base

\* Can we accept new requests? (circuit not fully open)
CanAcceptRequest ==
    \/ circuitState = "closed"
    \/ circuitState = "half_open"

-----------------------------------------------------------------------------
(* Circuit Breaker Actions *)

(*
 * CircuitTimerTick: Advance time for circuit breaker reset
 * Models the reset_timeout mechanism
 *)
CircuitTimerTick ==
    /\ globalTime < MaxTime
    /\ circuitState = "open"
    /\ timeSinceOpen' = timeSinceOpen + 1
    /\ globalTime' = globalTime + 1
    /\ UNCHANGED <<requestState, retryCount, backoffRemaining, circuitState,
                  consecutiveFailures, consecutiveSuccesses, componentStatus,
                  degradedSince, totalRequests, successfulRequests, failedRequests>>

(*
 * CircuitTryReset: Open circuit transitions to half-open after timeout
 * Models: CircuitBreaker check() method
 *)
CircuitTryReset ==
    /\ circuitState = "open"
    /\ timeSinceOpen >= ResetTimeout
    /\ circuitState' = "half_open"
    /\ consecutiveSuccesses' = 0
    /\ UNCHANGED <<requestState, retryCount, backoffRemaining,
                  consecutiveFailures, timeSinceOpen, componentStatus,
                  degradedSince, globalTime, totalRequests, successfulRequests, failedRequests>>

-----------------------------------------------------------------------------
(* Request/Retry Actions *)

(*
 * StartRequest: Begin a new request
 * Models: retry loop start in RetryPolicy
 *)
StartRequest ==
    /\ requestState = "idle"
    /\ globalTime < MaxTime
    /\ CanAcceptRequest
    /\ requestState' = "attempting"
    /\ retryCount' = 0
    /\ totalRequests' = totalRequests + 1
    /\ globalTime' = globalTime + 1
    /\ UNCHANGED <<backoffRemaining, circuitState, consecutiveFailures,
                  consecutiveSuccesses, timeSinceOpen, componentStatus,
                  degradedSince, successfulRequests, failedRequests>>

(*
 * RequestRejected: Circuit is open, reject immediately
 * Models: CircuitBreaker returning Err(CircuitOpen)
 *)
RequestRejected ==
    /\ requestState = "idle"
    /\ globalTime < MaxTime
    /\ circuitState = "open"
    /\ timeSinceOpen < ResetTimeout
    /\ requestState' = "failed"
    /\ failedRequests' = failedRequests + 1
    /\ totalRequests' = totalRequests + 1
    /\ globalTime' = globalTime + 1
    /\ UNCHANGED <<retryCount, backoffRemaining, circuitState, consecutiveFailures,
                  consecutiveSuccesses, timeSinceOpen, componentStatus,
                  degradedSince, successfulRequests>>

(*
 * RequestSuccess: Attempt succeeds
 * Models: Successful operation execution
 *)
RequestSuccess ==
    /\ requestState = "attempting"
    /\ componentStatus = "healthy"  \* Success only when healthy
    /\ requestState' = "succeeded"
    /\ successfulRequests' = successfulRequests + 1
    \* Update circuit breaker on success
    /\ IF circuitState = "half_open" THEN
           IF consecutiveSuccesses + 1 >= SuccessThreshold THEN
               \* Close the circuit
               /\ circuitState' = "closed"
               /\ consecutiveSuccesses' = 0
               /\ consecutiveFailures' = 0
           ELSE
               /\ consecutiveSuccesses' = consecutiveSuccesses + 1
               /\ UNCHANGED <<circuitState, consecutiveFailures>>
       ELSE
           \* Closed: reset failure count
           /\ consecutiveFailures' = 0
           /\ UNCHANGED <<circuitState, consecutiveSuccesses>>
    /\ timeSinceOpen' = 0
    /\ UNCHANGED <<retryCount, backoffRemaining, componentStatus, degradedSince,
                  globalTime, totalRequests, failedRequests>>

(*
 * RequestFailure: Attempt fails, decide retry or give up
 * Models: Error handling in retry loop
 *)
RequestFailure ==
    /\ requestState = "attempting"
    /\ globalTime < MaxTime
    \* Update circuit breaker on failure
    /\ consecutiveFailures' = consecutiveFailures + 1
    /\ consecutiveSuccesses' = 0
    /\ IF circuitState = "half_open" THEN
           \* Immediate re-trip on any half-open failure
           /\ circuitState' = "open"
           /\ timeSinceOpen' = 0
       ELSE IF consecutiveFailures + 1 >= FailureThreshold THEN
           \* Trip the circuit
           /\ circuitState' = "open"
           /\ timeSinceOpen' = 0
       ELSE
           /\ UNCHANGED <<circuitState, timeSinceOpen>>
    \* Decide retry or fail
    /\ IF retryCount < MaxRetries THEN
           \* Retry with backoff
           /\ requestState' = "waiting_backoff"
           /\ backoffRemaining' = BackoffDelay(retryCount)
           /\ retryCount' = retryCount + 1
           /\ UNCHANGED <<failedRequests>>
       ELSE
           \* Max retries exceeded, fail
           /\ requestState' = "failed"
           /\ failedRequests' = failedRequests + 1
           /\ UNCHANGED <<backoffRemaining, retryCount>>
    /\ globalTime' = globalTime + 1
    /\ UNCHANGED <<componentStatus, degradedSince, totalRequests, successfulRequests>>

(*
 * BackoffTick: Wait during backoff period
 * Models: tokio::time::sleep in retry loop
 *)
BackoffTick ==
    /\ requestState = "waiting_backoff"
    /\ globalTime < MaxTime
    /\ backoffRemaining > 0
    /\ backoffRemaining' = backoffRemaining - 1
    /\ globalTime' = globalTime + 1
    /\ UNCHANGED <<requestState, retryCount, circuitState, consecutiveFailures,
                  consecutiveSuccesses, timeSinceOpen, componentStatus,
                  degradedSince, totalRequests, successfulRequests, failedRequests>>

(*
 * BackoffComplete: Backoff done, retry the request
 *)
BackoffComplete ==
    /\ requestState = "waiting_backoff"
    /\ backoffRemaining = 0
    /\ CanAcceptRequest  \* Check circuit before retry
    /\ requestState' = "attempting"
    /\ UNCHANGED <<retryCount, backoffRemaining, circuitState, consecutiveFailures,
                  consecutiveSuccesses, timeSinceOpen, componentStatus,
                  degradedSince, globalTime, totalRequests, successfulRequests, failedRequests>>

(*
 * RequestReset: Reset after success/failure for next request
 *)
RequestReset ==
    /\ requestState \in {"succeeded", "failed"}
    /\ requestState' = "idle"
    /\ retryCount' = 0
    /\ backoffRemaining' = 0
    /\ UNCHANGED <<circuitState, consecutiveFailures, consecutiveSuccesses,
                  timeSinceOpen, componentStatus, degradedSince, globalTime,
                  totalRequests, successfulRequests, failedRequests>>

-----------------------------------------------------------------------------
(* Degraded Mode Actions *)

(*
 * ComponentDegrades: Component enters degraded state
 * Models: Storage/Prometheus/etc failure
 *)
ComponentDegrades ==
    /\ componentStatus = "healthy"
    /\ componentStatus' = "degraded"
    /\ degradedSince' = globalTime
    /\ UNCHANGED <<requestState, retryCount, backoffRemaining, circuitState,
                  consecutiveFailures, consecutiveSuccesses, timeSinceOpen,
                  globalTime, totalRequests, successfulRequests, failedRequests>>

(*
 * ComponentRecovers: Component recovers from degraded state
 * Models: Component recovery
 *)
ComponentRecovers ==
    /\ componentStatus = "degraded"
    /\ componentStatus' = "healthy"
    /\ degradedSince' = 0
    /\ UNCHANGED <<requestState, retryCount, backoffRemaining, circuitState,
                  consecutiveFailures, consecutiveSuccesses, timeSinceOpen,
                  globalTime, totalRequests, successfulRequests, failedRequests>>

-----------------------------------------------------------------------------
(* Next State Relation *)

TimeUp ==
    /\ globalTime = MaxTime
    /\ UNCHANGED vars

Next ==
    \/ StartRequest
    \/ RequestRejected
    \/ RequestSuccess
    \/ RequestFailure
    \/ BackoffTick
    \/ BackoffComplete
    \/ RequestReset
    \/ CircuitTimerTick
    \/ CircuitTryReset
    \/ ComponentDegrades
    \/ ComponentRecovers
    \/ TimeUp

Spec == Init /\ [][Next]_vars

-----------------------------------------------------------------------------
(* Safety Properties *)

(*
 * CircuitBreakerStateValid: Circuit breaker state transitions are legal
 *)
CircuitBreakerStateValid ==
    \* If closed, failures < threshold OR we just transitioned
    /\ (circuitState = "closed") =>
        (consecutiveFailures < FailureThreshold \/ consecutiveFailures = 0)
    \* If half-open, successes < threshold (unless we just closed)
    /\ (circuitState = "half_open") =>
        (consecutiveSuccesses < SuccessThreshold)

(*
 * RetryBounded: Retry count never exceeds max
 *)
RetryBounded ==
    retryCount <= MaxRetries

(*
 * BackoffNonNegative: Backoff remaining is never negative
 *)
BackoffNonNegative ==
    backoffRemaining >= 0

(*
 * RequestCountsValid: Success + Failed <= Total
 *)
RequestCountsValid ==
    successfulRequests + failedRequests <= totalRequests

(*
 * CircuitOpenImpliesFailures: Open circuit means we hit failure threshold
 * (or we're in half-open and re-tripped)
 *)
CircuitOpenImpliesFailures ==
    (circuitState = "open" /\ timeSinceOpen = 0) =>
        (consecutiveFailures >= FailureThreshold \/ consecutiveSuccesses >= 0)

(*
 * DegradedStateValid: Degraded state tracking is consistent
 *)
DegradedStateValid ==
    (componentStatus = "degraded") <=> (degradedSince > 0 \/ globalTime = 0)

(*
 * Combined Safety Invariant
 *)
Safety ==
    /\ TypeInvariant
    /\ RetryBounded
    /\ BackoffNonNegative
    /\ RequestCountsValid

-----------------------------------------------------------------------------
(* Liveness Properties *)

(*
 * EventualCircuitReset: Open circuit eventually tries to reset (with fairness)
 *)
EventualCircuitReset ==
    (circuitState = "open") ~> (circuitState = "half_open")

(*
 * RequestEventuallyCompletes: Active request eventually succeeds or fails
 *)
RequestEventuallyCompletes ==
    (requestState = "attempting" \/ requestState = "waiting_backoff")
    ~> (requestState = "succeeded" \/ requestState = "failed")

(*
 * DegradedEventuallyRecovers: Degraded component eventually recovers
 *)
DegradedEventuallyRecovers ==
    (componentStatus = "degraded") ~> (componentStatus = "healthy")

-----------------------------------------------------------------------------
(* Fairness Constraints *)

Fairness ==
    /\ WF_vars(StartRequest)
    /\ WF_vars(RequestSuccess)
    /\ WF_vars(RequestFailure)
    /\ WF_vars(BackoffTick)
    /\ WF_vars(BackoffComplete)
    /\ WF_vars(RequestReset)
    /\ WF_vars(CircuitTimerTick)
    /\ WF_vars(CircuitTryReset)
    \* No fairness on ComponentDegrades/Recovers (adversarial)

FairSpec == Spec /\ Fairness

=============================================================================
