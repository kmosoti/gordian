import Std

namespace Gordian

inductive Role where
  | worker
  | coordinator
  | deploymentAuthority
  deriving Repr, DecidableEq

def CanPromoteAccepted : Role → Prop
  | .coordinator => True
  | _ => False

def CanDeploy : Role → Prop
  | .deploymentAuthority => True
  | _ => False

theorem worker_cannot_promote : ¬ CanPromoteAccepted .worker := by
  simp [CanPromoteAccepted]

theorem coordinator_can_promote : CanPromoteAccepted .coordinator := by
  simp [CanPromoteAccepted]

theorem worker_cannot_deploy : ¬ CanDeploy .worker := by
  simp [CanDeploy]

theorem coordinator_cannot_deploy_by_default : ¬ CanDeploy .coordinator := by
  simp [CanDeploy]

end Gordian
