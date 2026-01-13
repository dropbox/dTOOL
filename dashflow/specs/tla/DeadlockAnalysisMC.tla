------------------------- MODULE DeadlockAnalysisMC -------------------------
(***************************************************************************
 * TLC Model Module for DeadlockAnalysis.tla
 *
 * Provides a concrete small model by instantiating DeadlockAnalysis with
 * operator-defined constants (including record-valued edge sets).
 *
 * Run via run_tlc.sh (it auto-selects *MC.tla when present).
 *
 * Author: Worker #2350
 * Date: 2026-01-03
 ***************************************************************************)

EXTENDS TLC

\* Variables must be explicitly mapped when instantiating a module with VARIABLES.
VARIABLES
    currentNodes,
    status,
    iterationCount,
    elapsedTime,
    parallelActive,
    parallelPending,
    parallelCompleted,
    parallelResults,
    semaphorePermits,
    semaphoreWaiters,
    executedNodes,
    lastParallelMerge

INSTANCE DeadlockAnalysis
    WITH Nodes <- {"start", "process", "done", "retry", "parallel_a", "parallel_b", "merge"},
         EntryPoint <- "start",
         RecursionLimit <- 10,
         MaxParallelTasks <- 2,
         GraphTimeout <- 20,
         NodeTimeout <- 5,
         SimpleEdges <- {
             [from |-> "process", to |-> "done"],
             [from |-> "retry", to |-> "process"],
             [from |-> "merge", to |-> "done"]
         },
         ConditionalEdges <- {
             [from |-> "start", routes |-> {
                 [cond |-> "sequential", to |-> "process"],
                 [cond |-> "parallel", to |-> "parallel_a"]
             }]
         },
         ParallelEdges <- {
             [from |-> "parallel_a", to |-> {"parallel_a", "parallel_b"}],
             [from |-> "parallel_b", to |-> {"merge"}]
         },
         currentNodes <- currentNodes,
         status <- status,
         iterationCount <- iterationCount,
         elapsedTime <- elapsedTime,
         parallelActive <- parallelActive,
         parallelPending <- parallelPending,
         parallelCompleted <- parallelCompleted,
         parallelResults <- parallelResults,
         semaphorePermits <- semaphorePermits,
         semaphoreWaiters <- semaphoreWaiters,
         executedNodes <- executedNodes,
         lastParallelMerge <- lastParallelMerge

=============================================================================
