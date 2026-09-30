import type { HealthStatus } from "../../models/types";

const styles: Record<HealthStatus, string> = {
  HEALTHY: "bg-emerald-500/15 text-emerald-300 ring-emerald-500/30",
  MISSING: "bg-amber-500/15 text-amber-300 ring-amber-500/30",
  CORRUPT: "bg-rose-500/15 text-rose-300 ring-rose-500/30",
  BIT_ROT: "bg-violet-500/15 text-violet-300 ring-violet-500/30",
};

const labels: Record<HealthStatus, string> = {
  HEALTHY: "Healthy",
  MISSING: "Missing",
  CORRUPT: "Corrupt",
  BIT_ROT: "Bit Rot",
};

interface HealthBadgeProps {
  status: HealthStatus;
}

export function HealthBadge({ status }: HealthBadgeProps) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium ring-1 ring-inset ${styles[status]}`}
    >
      <span aria-hidden="true" className="size-1.5 rounded-full bg-current" />
      {labels[status]}
    </span>
  );
}
