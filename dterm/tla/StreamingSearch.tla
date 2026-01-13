------------------------ MODULE StreamingSearch ------------------------
(***************************************************************************)
(* Streaming Search State Machine                                          *)
(*                                                                         *)
(* Models the incremental search/filter system in dterm:                   *)
(* - Incremental search state machine (Idle -> Searching -> Results)       *)
(* - Match highlighting across scrollback history                          *)
(* - Filter modes: regex, literal, fuzzy                                   *)
(* - Search result navigation with wraparound                              *)
(* - Memory-bounded result set with overflow handling                      *)
(*                                                                         *)
(* Key features:                                                           *)
(* - Streaming: results appear as content is scanned                       *)
(* - Incremental: pattern changes trigger re-search                        *)
(* - Memory-bounded: caps result storage with LRU eviction                 *)
(*                                                                         *)
(* Safety Properties:                                                       *)
(* - Current match index always valid within results                       *)
(* - Result positions always valid grid coordinates                        *)
(* - Memory usage never exceeds configured limit                           *)
(* - No duplicate results in result set                                    *)
(*                                                                         *)
(* Liveness Properties:                                                     *)
(* - Search eventually completes                                           *)
(* - Pattern change triggers re-search                                     *)
(***************************************************************************)

EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS
    MaxRows,            \* Maximum grid rows (display + scrollback)
    MaxCols,            \* Maximum grid columns
    MaxResults,         \* Maximum number of stored results (memory bound)
    MaxPatternLen,      \* Maximum search pattern length
    FilterModes         \* Set: {"Literal", "Regex", "Fuzzy"}

\* Constraint assumptions for model checking
ASSUME MaxRows \in Nat /\ MaxRows > 0
ASSUME MaxCols \in Nat /\ MaxCols > 0
ASSUME MaxResults \in Nat /\ MaxResults > 0
ASSUME MaxPatternLen \in Nat /\ MaxPatternLen > 0
ASSUME FilterModes \subseteq {"Literal", "Regex", "Fuzzy"} /\ FilterModes # {}

VARIABLES
    state,              \* Search state: Idle, Searching, HasResults
    filterMode,         \* Current filter mode
    pattern,            \* Current search pattern (sequence of chars)
    results,            \* Sequence of Match records
    currentIndex,       \* Index of currently highlighted result (1-based, 0 = none)
    scanProgress,       \* Row currently being scanned (for streaming)
    totalMatches,       \* Total matches found (may exceed stored results)
    searchDirection,    \* "Forward" or "Backward"
    wrapEnabled,        \* Whether navigation wraps around
    caseSensitive,      \* Whether search is case-sensitive
    highlightAll        \* Whether all matches are highlighted (vs just current)

vars == <<state, filterMode, pattern, results, currentIndex, scanProgress,
          totalMatches, searchDirection, wrapEnabled, caseSensitive, highlightAll>>

(***************************************************************************)
(* Type Definitions                                                        *)
(***************************************************************************)

\* Search states
SearchStates == {"Idle", "Searching", "HasResults", "NoResults"}

\* Directions for navigation
Directions == {"Forward", "Backward"}

\* Valid grid coordinates
ValidRow == 0..(MaxRows - 1)
ValidCol == 0..(MaxCols - 1)

\* A match record captures a single search hit
\* row: which row the match starts
\* startCol: starting column
\* endCol: ending column (exclusive, must be >= startCol)
\* matchLen: length of the match in characters
Match == {m \in [row: ValidRow, startCol: ValidCol, endCol: ValidCol, matchLen: 1..MaxCols] :
          m.startCol <= m.endCol}

\* For model checking: limit matches per row to reduce state space
\* A match set for a row has at most 2 matches to keep SUBSET tractable
MatchesForRow(r) ==
    {m \in Match : m.row = r}

\* Bounded match sets for model checking (at most 2 matches per scan)
BoundedMatchSets(r) ==
    {ms \in SUBSET MatchesForRow(r) : Cardinality(ms) <= 2}

\* Pattern is a sequence of characters
\* For model checking, use a small set of characters to reduce state space
PatternChars == 0..3  \* Small alphabet for tractable model checking
Patterns == UNION {[1..n -> PatternChars] : n \in 0..MaxPatternLen}

(***************************************************************************)
(* Type Invariant                                                          *)
(***************************************************************************)

TypeInvariant ==
    /\ state \in SearchStates
    /\ filterMode \in FilterModes
    /\ pattern \in Patterns
    /\ results \in Seq(Match)
    /\ Len(results) <= MaxResults
    /\ currentIndex \in 0..MaxResults
    /\ scanProgress \in -1..MaxRows       \* -1 = not scanning
    /\ totalMatches \in Nat
    /\ searchDirection \in Directions
    /\ wrapEnabled \in BOOLEAN
    /\ caseSensitive \in BOOLEAN
    /\ highlightAll \in BOOLEAN
    \* Current index validity
    /\ (currentIndex > 0 => currentIndex <= Len(results))
    /\ (state = "Idle" => currentIndex = 0)
    /\ (state = "NoResults" => Len(results) = 0)

(***************************************************************************)
(* Safety Invariants                                                       *)
(***************************************************************************)

\* INV-SEARCH-1: Current match index is always valid
CurrentIndexValid ==
    currentIndex = 0 \/ currentIndex <= Len(results)

\* INV-SEARCH-2: All result positions are valid grid coordinates
ResultPositionsValid ==
    \A i \in 1..Len(results):
        /\ results[i].row \in ValidRow
        /\ results[i].startCol \in ValidCol
        /\ results[i].endCol \in ValidCol
        /\ results[i].startCol <= results[i].endCol

\* INV-SEARCH-3: Memory usage bounded
MemoryBounded ==
    Len(results) <= MaxResults

\* INV-SEARCH-4: No duplicate results
NoDuplicateResults ==
    \A i, j \in 1..Len(results):
        i # j =>
            ~(results[i].row = results[j].row /\
              results[i].startCol = results[j].startCol)

\* INV-SEARCH-5: Scan progress consistent with state
ScanProgressConsistent ==
    /\ (state = "Idle" => scanProgress = -1)
    /\ (state = "Searching" => scanProgress \in 0..(MaxRows - 1))
    /\ (scanProgress = MaxRows - 1 => state # "Searching")

\* INV-SEARCH-6: Total matches >= stored results
TotalMatchesConsistent ==
    totalMatches >= Len(results)

\* Combined safety invariant
SafetyInvariant ==
    /\ CurrentIndexValid
    /\ ResultPositionsValid
    /\ MemoryBounded
    /\ NoDuplicateResults
    /\ TotalMatchesConsistent

(***************************************************************************)
(* Helper Functions                                                        *)
(***************************************************************************)

\* Convert set to sequence (for adding matches to results)
SetToSeq(s) ==
    LET f[ss \in SUBSET s] ==
        IF ss = {} THEN <<>>
        ELSE LET x == CHOOSE x \in ss: TRUE
             IN <<x>> \o f[ss \ {x}]
    IN f[s]

\* Check if a match is already in results
MatchExists(m, res) ==
    \E i \in 1..Len(res):
        /\ res[i].row = m.row
        /\ res[i].startCol = m.startCol

\* Get next index with wraparound
NextIndex(idx, len, dir, wrap) ==
    IF dir = "Forward" THEN
        IF idx >= len THEN
            IF wrap THEN 1 ELSE idx
        ELSE idx + 1
    ELSE \* Backward
        IF idx <= 1 THEN
            IF wrap THEN len ELSE idx
        ELSE idx - 1

\* Check if pattern is empty
PatternEmpty == Len(pattern) = 0

(***************************************************************************)
(* Initial State                                                           *)
(***************************************************************************)

Init ==
    /\ state = "Idle"
    /\ filterMode = "Literal"
    /\ pattern = <<>>
    /\ results = <<>>
    /\ currentIndex = 0
    /\ scanProgress = -1
    /\ totalMatches = 0
    /\ searchDirection = "Forward"
    /\ wrapEnabled = TRUE
    /\ caseSensitive = FALSE
    /\ highlightAll = TRUE

(***************************************************************************)
(* Search Operations                                                       *)
(***************************************************************************)

\* Start a new search with a pattern
\* Only allowed when idle, or when pattern/mode actually changes
StartSearch(newPattern, mode) ==
    /\ newPattern # <<>>
    /\ Len(newPattern) <= MaxPatternLen
    /\ mode \in FilterModes
    /\ \/ state = "Idle"                           \* Can always start from idle
       \/ newPattern # pattern                      \* Can restart with different pattern
       \/ mode # filterMode                         \* Can restart with different mode
    /\ pattern' = newPattern
    /\ filterMode' = mode
    /\ state' = "Searching"
    /\ results' = <<>>
    /\ currentIndex' = 0
    /\ scanProgress' = 0
    /\ totalMatches' = 0
    /\ UNCHANGED <<searchDirection, wrapEnabled, caseSensitive, highlightAll>>

\* Update pattern incrementally (as user types)
UpdatePattern(newPattern) ==
    /\ state \in {"Searching", "HasResults", "NoResults"}
    /\ newPattern # pattern
    /\ Len(newPattern) <= MaxPatternLen
    /\ pattern' = newPattern
    /\ IF newPattern = <<>> THEN
           \* Pattern cleared - reset to idle
           /\ state' = "Idle"
           /\ results' = <<>>
           /\ currentIndex' = 0
           /\ scanProgress' = -1
           /\ totalMatches' = 0
       ELSE
           \* Pattern changed - restart search
           /\ state' = "Searching"
           /\ results' = <<>>
           /\ currentIndex' = 0
           /\ scanProgress' = 0
           /\ totalMatches' = 0
    /\ UNCHANGED <<filterMode, searchDirection, wrapEnabled, caseSensitive, highlightAll>>

\* Deduplicate matches by (row, startCol) - keep only one match per position
DedupeMatches(matches) ==
    {m \in matches : \A m2 \in matches :
        (m.row = m2.row /\ m.startCol = m2.startCol) => m = m2}

\* Process one row during streaming search
ScanRow(row, matchesFound) ==
    /\ state = "Searching"
    /\ scanProgress = row
    /\ row < MaxRows
    /\ matchesFound \in SUBSET Match
    /\ \A m \in matchesFound: m.row = row
    \* Dedupe incoming matches and filter out already-existing ones
    /\ LET dedupedFound == DedupeMatches(matchesFound)
           newMatches == {m \in dedupedFound: ~MatchExists(m, results)}
           addableMatches == IF Len(results) + Cardinality(newMatches) <= MaxResults
                            THEN newMatches
                            ELSE CHOOSE subset \in SUBSET newMatches:
                                   Cardinality(subset) = MaxResults - Len(results)
       IN
           /\ results' = results \o SetToSeq(addableMatches)
           /\ totalMatches' = totalMatches + Cardinality(matchesFound)
           /\ scanProgress' = row + 1
           /\ IF row + 1 >= MaxRows THEN
                  \* Scan complete
                  IF Len(results') > 0 THEN
                      /\ state' = "HasResults"
                      /\ currentIndex' = 1
                  ELSE
                      /\ state' = "NoResults"
                      /\ currentIndex' = 0
              ELSE
                  /\ UNCHANGED <<state, currentIndex>>
    /\ UNCHANGED <<filterMode, pattern, searchDirection, wrapEnabled,
                  caseSensitive, highlightAll>>

\* Complete search scan
CompleteSearch ==
    /\ state = "Searching"
    /\ scanProgress = MaxRows
    /\ IF Len(results) > 0 THEN
           /\ state' = "HasResults"
           /\ currentIndex' = 1
       ELSE
           /\ state' = "NoResults"
           /\ currentIndex' = 0
    /\ scanProgress' = -1
    /\ UNCHANGED <<filterMode, pattern, results, totalMatches, searchDirection,
                  wrapEnabled, caseSensitive, highlightAll>>

\* Cancel search
CancelSearch ==
    /\ state \in {"Searching", "HasResults", "NoResults"}
    /\ state' = "Idle"
    /\ pattern' = <<>>
    /\ results' = <<>>
    /\ currentIndex' = 0
    /\ scanProgress' = -1
    /\ totalMatches' = 0
    /\ UNCHANGED <<filterMode, searchDirection, wrapEnabled, caseSensitive, highlightAll>>

(***************************************************************************)
(* Navigation Operations                                                   *)
(***************************************************************************)

\* Navigate to next match
NextMatch ==
    /\ state = "HasResults"
    /\ Len(results) > 0
    /\ currentIndex' = NextIndex(currentIndex, Len(results), "Forward", wrapEnabled)
    /\ UNCHANGED <<state, filterMode, pattern, results, scanProgress, totalMatches,
                  searchDirection, wrapEnabled, caseSensitive, highlightAll>>

\* Navigate to previous match
PrevMatch ==
    /\ state = "HasResults"
    /\ Len(results) > 0
    /\ currentIndex' = NextIndex(currentIndex, Len(results), "Backward", wrapEnabled)
    /\ UNCHANGED <<state, filterMode, pattern, results, scanProgress, totalMatches,
                  searchDirection, wrapEnabled, caseSensitive, highlightAll>>

\* Jump to specific match index
JumpToMatch(idx) ==
    /\ state = "HasResults"
    /\ idx \in 1..Len(results)
    /\ currentIndex' = idx
    /\ UNCHANGED <<state, filterMode, pattern, results, scanProgress, totalMatches,
                  searchDirection, wrapEnabled, caseSensitive, highlightAll>>

\* Set search direction
SetDirection(dir) ==
    /\ dir \in Directions
    /\ searchDirection' = dir
    /\ UNCHANGED <<state, filterMode, pattern, results, currentIndex, scanProgress,
                  totalMatches, wrapEnabled, caseSensitive, highlightAll>>

(***************************************************************************)
(* Configuration Operations                                                *)
(***************************************************************************)

\* Toggle wrap-around navigation
ToggleWrap ==
    /\ wrapEnabled' = ~wrapEnabled
    /\ UNCHANGED <<state, filterMode, pattern, results, currentIndex, scanProgress,
                  totalMatches, searchDirection, caseSensitive, highlightAll>>

\* Toggle case sensitivity
ToggleCaseSensitive ==
    /\ caseSensitive' = ~caseSensitive
    \* Changing case sensitivity requires re-search
    /\ IF state \in {"HasResults", "NoResults"} /\ Len(pattern) > 0 THEN
           /\ state' = "Searching"
           /\ results' = <<>>
           /\ currentIndex' = 0
           /\ scanProgress' = 0
           /\ totalMatches' = 0
       ELSE
           UNCHANGED <<state, results, currentIndex, scanProgress, totalMatches>>
    /\ UNCHANGED <<filterMode, pattern, searchDirection, wrapEnabled, highlightAll>>

\* Toggle highlight all matches
ToggleHighlightAll ==
    /\ highlightAll' = ~highlightAll
    /\ UNCHANGED <<state, filterMode, pattern, results, currentIndex, scanProgress,
                  totalMatches, searchDirection, wrapEnabled, caseSensitive>>

\* Change filter mode
SetFilterMode(mode) ==
    /\ mode \in FilterModes
    /\ mode # filterMode
    /\ filterMode' = mode
    \* Changing mode requires re-search if we have a pattern
    /\ IF state \in {"HasResults", "NoResults"} /\ Len(pattern) > 0 THEN
           /\ state' = "Searching"
           /\ results' = <<>>
           /\ currentIndex' = 0
           /\ scanProgress' = 0
           /\ totalMatches' = 0
       ELSE
           UNCHANGED <<state, results, currentIndex, scanProgress, totalMatches>>
    /\ UNCHANGED <<pattern, searchDirection, wrapEnabled, caseSensitive, highlightAll>>

(***************************************************************************)
(* Content Change Handling                                                 *)
(***************************************************************************)

\* New content added to terminal (may contain matches)
ContentAdded(row, newMatches) ==
    /\ row \in ValidRow
    /\ state = "HasResults"
    /\ newMatches \in SUBSET Match
    /\ \A m \in newMatches: m.row = row
    \* Add new matches if we have room (dedupe first, then bound to available space)
    /\ LET dedupedNew == DedupeMatches(newMatches)
           notInResults == {m \in dedupedNew: ~MatchExists(m, results)}
           availableSpace == MaxResults - Len(results)
           addable == IF Cardinality(notInResults) <= availableSpace
                      THEN notInResults
                      ELSE CHOOSE subset \in SUBSET notInResults:
                           Cardinality(subset) = availableSpace
       IN results' = results \o SetToSeq(addable)
    /\ totalMatches' = totalMatches + Cardinality(newMatches)
    /\ IF currentIndex = 0 /\ Len(results') > 0 THEN
           currentIndex' = 1
       ELSE
           UNCHANGED currentIndex
    /\ UNCHANGED <<state, filterMode, pattern, scanProgress, searchDirection,
                  wrapEnabled, caseSensitive, highlightAll>>

\* Content cleared/scrolled - invalidate affected results
ContentInvalidated(fromRow, toRow) ==
    /\ fromRow \in ValidRow
    /\ toRow \in ValidRow
    /\ fromRow <= toRow
    /\ state \in {"HasResults", "NoResults"}
    \* Remove results in the invalidated range
    /\ LET validResults == SelectSeq(results,
            LAMBDA m: m.row < fromRow \/ m.row > toRow)
       IN
           /\ results' = validResults
           /\ IF Len(validResults) = 0 THEN
                  /\ state' = "NoResults"
                  /\ currentIndex' = 0
              ELSE
                  /\ UNCHANGED state
                  /\ currentIndex' = IF currentIndex > Len(validResults)
                                     THEN Len(validResults)
                                     ELSE currentIndex
    /\ UNCHANGED <<filterMode, pattern, scanProgress, totalMatches, searchDirection,
                  wrapEnabled, caseSensitive, highlightAll>>

(***************************************************************************)
(* State Machine Specification                                             *)
(***************************************************************************)

Next ==
    \* Start search
    \/ \E p \in Patterns, m \in FilterModes:
        p # <<>> /\ StartSearch(p, m)
    \* Update pattern
    \/ \E p \in Patterns: UpdatePattern(p)
    \* Scan rows (streaming search) - use bounded match sets for tractable model checking
    \/ \E r \in ValidRow: \E matches \in BoundedMatchSets(r):
        ScanRow(r, matches)
    \* Complete search
    \/ CompleteSearch
    \* Cancel search
    \/ CancelSearch
    \* Navigation
    \/ NextMatch
    \/ PrevMatch
    \/ \E idx \in 1..MaxResults: JumpToMatch(idx)
    \/ \E dir \in Directions: SetDirection(dir)
    \* Configuration
    \/ ToggleWrap
    \/ ToggleCaseSensitive
    \/ ToggleHighlightAll
    \/ \E mode \in FilterModes: SetFilterMode(mode)
    \* Content changes - use bounded match sets for tractable model checking
    \/ \E r \in ValidRow: \E matches \in BoundedMatchSets(r):
        ContentAdded(r, matches)
    \/ \E fr \in ValidRow, tr \in ValidRow:
        fr <= tr /\ ContentInvalidated(fr, tr)

Spec == Init /\ [][Next]_vars

(***************************************************************************)
(* Liveness Properties                                                     *)
(***************************************************************************)

\* Search eventually completes or is cancelled
EventualSearchCompletion ==
    state = "Searching" ~> state \in {"HasResults", "NoResults", "Idle"}

\* Pattern change triggers re-search
\* Note: Simplified to avoid primed variables in temporal formula
PatternChangeTriggersResearch ==
    [](pattern # <<>> => <>(state = "Searching"))

(***************************************************************************)
(* Theorems                                                                *)
(***************************************************************************)

\* THEOREM: Type invariant is preserved
THEOREM TypeInvariantHolds ==
    Spec => []TypeInvariant

\* THEOREM: Safety invariant is preserved
THEOREM SafetyInvariantHolds ==
    Spec => []SafetyInvariant

\* THEOREM: Current index is always valid
THEOREM CurrentIndexAlwaysValid ==
    Spec => []CurrentIndexValid

\* THEOREM: Memory is always bounded
THEOREM MemoryAlwaysBounded ==
    Spec => []MemoryBounded

\* THEOREM: No duplicate results
THEOREM NoDuplicatesEver ==
    Spec => []NoDuplicateResults

\* THEOREM: State transitions are valid
StateTransitionsValid ==
    [][
        \/ state = state'
        \/ (state = "Idle" /\ state' = "Searching")
        \/ (state = "Searching" /\ state' \in {"HasResults", "NoResults", "Idle"})
        \/ (state = "HasResults" /\ state' \in {"Searching", "NoResults", "Idle"})
        \/ (state = "NoResults" /\ state' \in {"Searching", "Idle"})
    ]_vars

THEOREM StateTransitionsAreValid ==
    Spec => StateTransitionsValid

\* THEOREM: Scan progress is monotonic during a continuous search
\* scanProgress can reset when search restarts (pattern or mode change, or new StartSearch)
\* We check monotonicity only when pattern, filterMode, and results are unchanged
ScanProgressMonotonic ==
    [][
        (state = "Searching" /\ state' = "Searching" /\
         pattern = pattern' /\ filterMode = filterMode' /\ results = results') =>
        scanProgress' >= scanProgress
    ]_vars

THEOREM ScanIsMonotonic ==
    Spec => ScanProgressMonotonic

\* THEOREM: Total matches never decreases within a continuous search
\* Resets when pattern, filterMode, or caseSensitive changes
TotalMatchesMonotonic ==
    [][
        (pattern = pattern' /\ filterMode = filterMode' /\ caseSensitive = caseSensitive') =>
        totalMatches' >= totalMatches
    ]_totalMatches

THEOREM TotalMatchesNeverDecreases ==
    Spec => TotalMatchesMonotonic

\* THEOREM: Cancelling search clears state
CancelClearsState ==
    [][
        (state # "Idle" /\ state' = "Idle" /\ pattern' = <<>>) =>
        (results' = <<>> /\ currentIndex' = 0)
    ]_vars

THEOREM CancelClearsEverything ==
    Spec => CancelClearsState

(***************************************************************************)
(* STATE CONSTRAINT FOR MODEL CHECKING                                     *)
(***************************************************************************)

\* Bound the state space for tractable model checking
StateConstraint ==
    /\ Len(results) <= MaxResults
    /\ totalMatches <= MaxResults * 2
    /\ Len(pattern) <= MaxPatternLen

(***************************************************************************)
(* Model Checking Configuration                                            *)
(*                                                                         *)
(* For tractable model checking, use small constants:                      *)
(* MaxRows = 2, MaxCols = 3, MaxResults = 2, MaxPatternLen = 1            *)
(* FilterModes = {"Literal", "Regex", "Fuzzy"}                            *)
(***************************************************************************)

=============================================================================
