"""B3's byte-identity gate: R6's held-out manifest at b = 5, rho = 0.7, replayed with this branch's binary
under its own run id (only `source_revision` changed, written to its own directory), writes
`results.csv` and `incidents.csv` whose SHA-256 equals the ones recorded for it, for every arm
(`experiments/exploration/r6-results-sha256.csv`).

Usage: b3_gate.py [RUN_DIR [OUT_CSV]]
  default: artifacts/runs/xcheck3-r6-heldout-b5-rho0.7, b3-regression.csv. The repeat with the final
  tree's binary is `b3_gate.py xcheck4-r6-heldout-b5-rho0.7 b3-regression-final.csv`. Exit 1 if any arm
  differs or the arms of the replay are not the arms of the record. (B1's gate script does the work;
  this fixes its arguments and the output names.)
"""

import sys

import b1_gate
import b3_common as C

if __name__ == "__main__":
    run_dir = sys.argv[1] if len(sys.argv) > 1 else f"xcheck3-r6-heldout-{C.setting_id(*C.PRIMARY)}"
    out = sys.argv[2] if len(sys.argv) > 2 else "b3-regression.csv"
    sys.argv = [sys.argv[0], run_dir, f"r6-heldout-{C.setting_id(*C.PRIMARY)}", "r6-results-sha256.csv", out]
    b1_gate.main()
