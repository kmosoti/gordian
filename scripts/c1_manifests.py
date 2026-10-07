"""Write C1's manifests.

Usage: c1_manifests.py STAGE     (tune | heldout; see `c1_common.arms`)

Every arm is the selection oracle at R5's delay (16 s) with the rung's own context, under the noticer
it names, at b = 5, rho = 0.7, as B1's, B2's, B3's, M2's and L1's tables. A manifest records
`git rev-parse HEAD` and the driver refuses a run whose manifest does not match a clean tree, so run
this after committing and immediately before the run. Written by L1's writer (`l1_manifests.write`,
which starts from `gordian-run init-stream` and refuses to overwrite).
"""

import sys

import c1_common as C
import l1_manifests as W


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    C.check_row()
    policy = {"policy": C.M.SEL_POLICY, "delay_ns": C.M.sel_delay_s() * C.NS}
    noticers, rows = {}, []
    for name, noticer in C.arms(stage):
        arm = C.arm_name(name)
        rows.append((arm, policy))
        noticers[arm] = noticer
    W.write(C.run_id(stage), C.seeds(stage), rows, noticers, C.RUN_SEED, C.experiment(stage))


if __name__ == "__main__":
    main()
