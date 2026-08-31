import Gordian
import Lean.Elab.Command
import Lean.Util.CollectAxioms

open Lean

namespace Gordian

/-- Axioms accepted by Gordian's formal kernel. The audit fails closed on every other axiom. -/
def allowedAxioms : Array Lean.Name :=
  #[``propext, ``Classical.choice, ``Quot.sound]

def isRepositoryModule (env : Lean.Environment) (declName : Lean.Name) : Bool :=
  let moduleName := match env.getModuleIdxFor? declName with
    | some moduleIdx => env.header.moduleNames[moduleIdx]!
    | none => env.mainModule
  (`Gordian : Lean.Name).isPrefixOf moduleName

/--
Audit every theorem in a repository-owned Gordian formal module's transitive axiom closure and
every axiom declared by those modules. Ownership is determined from the declaration's source
module, not its namespace, so a foreign namespace in a repository module is still audited while
axioms belonging to imported Lean or Std modules are not audited as declarations. This command
runs after importing the compiled formal kernel, so it checks the exact environment that
`leanchecker` replays rather than grepping source text.
-/
def auditAxioms : Elab.Command.CommandElabM Nat := do
  let env ← getEnv
  let declarations := env.constants.toList.filter fun (name, info) =>
    isRepositoryModule env name && (info.isTheorem || info matches .axiomInfo ..)
  let theorems := declarations.filter fun (_, info) => info.isTheorem
  if theorems.isEmpty then
    throwError "Gordian axiom audit found no theorems; refusing a vacuous pass"

  for (name, info) in declarations do
    if info matches .axiomInfo .. then
      unless allowedAxioms.contains name do
        throwError m!"Gordian declaration {name} is a non-allowlisted axiom"
    if info.isTheorem then
      for ax in (← collectAxioms name) do
        unless allowedAxioms.contains ax do
          throwError m!"Gordian theorem {name} depends on non-allowlisted axiom {ax}"

  return theorems.length

run_cmd do
  let checked ← auditAxioms
  logInfo m!"Gordian axiom audit checked {checked} theorems"

end Gordian
