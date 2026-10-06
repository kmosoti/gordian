"""Write the B4 manifests.

Usage:
  b4_manifests.py xcheck        R6's held-out manifest (b = 5, rho = 0.7) with only `source_revision`
                                replaced, run id kept, under the prefix `xcheck5-` (the byte-identity gate)
  b4_manifests.py xcheck-final  the same under the prefix `xcheck6-` (the gate again, with the final tree's
                                binary)

Exploration (nothing here tests a hypothesis). A manifest records `git rev-parse HEAD` and the driver
refuses a run whose manifest does not match a clean tree, so run this after committing and
immediately before the runs. The manifests are written by B2's writer (`b2_manifests.write`): the
environment is captured by `gordian-run init-stream`, then the arms, the reasoner's (b, rho) and the
per-arm noticers are set.
"""

import sys

import b2_manifests as M2


def main():
    stage = sys.argv[1] if len(sys.argv) > 1 else ""
    if stage == "xcheck":
        M2.xcheck("5")
    elif stage == "xcheck-final":
        M2.xcheck("6")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
