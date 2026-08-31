# Orchestration Log

## Atom #1
- tier used / tier that should have been used: coordination for contract decisions, write workers for disjoint implementation, and deep verification at freeze; this matched the risk split
- reasoning spent that did not change the outcome: comparing several result-storage layouts before the post-admission closure rule made the bookkeeping artifact the only non-circular location
- any hallucinated API, file, flag, or command, and what would have caught it earlier: the first contract fixture captured a mutable working-copy commit, the first benchmark assumed an added workspace had a Git worktree, and cleanup initially masked manifest-write failure; executing success, injected-failure, and output-failure paths before expanding either implementation would have caught them
- one rule to apply next time: execute the smallest end-to-end fixture on success and evidence-write failure before multiplying shapes or assertions, compare exact output sets before timing commands, and implement the registered family adjustment before interpreting any benchmark cell
