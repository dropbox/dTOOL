---- MODULE MC_DashStreamKafka ----
(***************************************************************************)
(* Model-checking configuration for DashStreamKafka                        *)
(* Uses reduced constants for tractable state space                        *)
(***************************************************************************)

EXTENDS DashStreamKafka

(***************************************************************************)
(* Concrete model values for model checking                               *)
(* Reduced from full spec to keep state space manageable                  *)
(***************************************************************************)

\* Small constants for verification - use same thread names as hardcoded in spec
MC_MaxMessages == 2
MC_MaxPartitions == 2
MC_MaxRetries == 1
MC_Threads == {"thread1", "thread2"}

====
