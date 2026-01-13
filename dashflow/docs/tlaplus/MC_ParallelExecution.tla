---- MODULE MC_ParallelExecution ----
(***************************************************************************)
(* Model-checking configuration for ParallelExecution                     *)
(***************************************************************************)

EXTENDS ParallelExecution

MC_Branches == {"a", "b", "c"}
MC_MaxStateValue == 3

====
