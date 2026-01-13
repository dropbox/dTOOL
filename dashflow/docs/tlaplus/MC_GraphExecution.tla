---- MODULE MC_GraphExecution ----
(***************************************************************************)
(* Model-checking configuration for GraphExecution                        *)
(* This module defines concrete values for model checking                 *)
(***************************************************************************)

EXTENDS GraphExecution

(***************************************************************************)
(* Concrete model values for model checking                               *)
(***************************************************************************)

MC_Nodes == {1, 2, 3}
MC_Edges == {<<1, 2>>, <<1, 3>>, <<2, 3>>}
MC_EntryNode == 1
MC_ExitNodes == {3}

====
