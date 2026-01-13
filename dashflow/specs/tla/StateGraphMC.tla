---------------------------- MODULE StateGraphMC ----------------------------
(***************************************************************************
 * TLC Model Module for StateGraph.tla
 *
 * TLC's .cfg parser does not reliably accept complex constant values (e.g.,
 * records) on all installations. This module provides a concrete small model
 * by instantiating StateGraph with operator-defined constants.
 *
 * Run via run_tlc.sh (it auto-selects *MC.tla when present).
 *
 * Author: Worker #2350
 * Date: 2026-01-03
 ***************************************************************************)

EXTENDS TLC

\* Variables must be explicitly mapped when instantiating a module with VARIABLES.
VARIABLES currentNodes, graphState, executedNodes, iterationCount, status

INSTANCE StateGraph
    WITH Nodes <- {"researcher", "writer", "reviewer"},
         EntryPoint <- "researcher",
         RecursionLimit <- 10,
         SimpleEdges <- {[from |-> "writer", to |-> "__END__"]},
         ConditionalEdges <- {
             [from |-> "researcher", routes |-> {
                 [cond |-> "continue", to |-> "writer"],
                 [cond |-> "review", to |-> "reviewer"]
             }]
         },
         ParallelEdges <- {},
         currentNodes <- currentNodes,
         graphState <- graphState,
         executedNodes <- executedNodes,
         iterationCount <- iterationCount,
         status <- status

=============================================================================
