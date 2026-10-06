# Charter revision proposal: the aim and its proxies

Status: **proposal, not adopted.** Nothing here is normative until the user approves it and the
charter is edited. It would add one section to `docs/charter.md` and change nothing else.

## The aim

The program's aim, stated by the user on 2026-10-06, is a different kind of intelligence rather
than the status quo: capability at or above the best conventional systems, at resource needs near
a human's. "Superintelligence" is not measurable. The aim is made testable through three resource
gaps and two proxies.

## The three gaps, as numbers

| Resource | Human (order of magnitude) | Conventional system | Gap |
|---|---|---|---|
| Energy | 20 W | kilowatts per inference node | about 10³ |
| Data | 10⁹ words in a lifetime | 10¹³ or more tokens in training | about 10⁴ |
| Compute per decision | 10¹⁵ synaptic events per second available, nearly all idle | dense: every parameter every token | sparsity, not rate |

These figures are from memory and must be checked against primary sources before the charter
cites them. The data gap is the largest and the one the status quo has no answer for.

## The two proxies

A result counts toward the aim only if it holds under hard resource limits and reports both:

1. **Decisions per unit cost:** verified correct decisions per unit of total modelled cost
   (charter section 4), with an energy proxy beside it (joules per correct decision, under a
   declared conversion that is fixed in the charter once, not per experiment).
2. **Improvement per unit experience:** the slope of correct decisions against incidents seen,
   compared with a matched conventional learner on the same stream. A mechanism that only matches
   the status quo at equal experience, or that needs more experience to match it, does not count
   toward the aim, whatever its cost.

## What this changes in practice

- Every preregistered experiment reports sample efficiency and the energy proxy for every arm.
- The learned substrate is tested on improvement per experience as soon as hand-designed gating
  earns its cost (charter section 12, build order), not after EXP-102 and EXP-103.
- The conventional comparators stay in every experiment. The aim is shown by beating them, not
  by removing them.

## What would falsify the aim's pursuit here

If a learned substrate, given public observations and its own history only, cannot beat the
tuned threshold rung at noticing within a few hundred streams, the cell model has no claim to
sample efficiency on this world, and the aim is pursued through another mechanism.
