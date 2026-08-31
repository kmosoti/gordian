# Orchestration Log

## Atom #1
- tier used / tier that should have been used: coordination for contract decisions, write workers for disjoint implementation, and deep verification at freeze; this matched the risk split
- reasoning spent that did not change the outcome: comparing several result-storage layouts before the post-admission closure rule made the bookkeeping artifact the only non-circular location
- any hallucinated API, file, flag, or command, and what would have caught it earlier: the first contract fixture captured a mutable working-copy commit, the first benchmark assumed an added workspace had a Git worktree, and cleanup initially masked manifest-write failure; executing success, injected-failure, and output-failure paths before expanding either implementation would have caught them
- one rule to apply next time: execute the smallest end-to-end fixture on success and evidence-write failure before multiplying shapes or assertions, compare exact output sets before timing commands, and implement the registered family adjustment before interpreting any benchmark cell

## Atom #2
- tier used / tier that should have been used: coordination for conflict and contract decisions, write workers for disjoint repairs, and one deep adversarial verifier; this was the right split after the stale attempt was rebased
- reasoning spent that did not change the outcome: whole-file conflict-side selection had to be undone once it proved broader than the marked overlap, and an early verifier run repeated setup before the Python package was installed in its fresh environment
- any hallucinated API, file, flag, or command, and what would have caught it earlier: the stale attempt used unsupported `jj diff --check`; the first formal evidence repair also assumed an empty-child `@-` verification topology and its negative probe triggered JJ auto-snapshotting; exercising the evidence writer in the real JJ workspace before freeze caught both identity errors
- one rule to apply next time: after any source-bound evidence change, run its success and failure paths in both Git-style and JJ-style state models before accepting fixture-only verification
