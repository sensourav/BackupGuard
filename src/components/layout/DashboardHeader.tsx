import { Cloud, HardDrive, Shield, Usb } from "lucide-react";
import type { ReactNode } from "react";
import type { CloudLinkState, MountState, SystemStatus } from "../../models/types";

interface DashboardHeaderProps {
  status: SystemStatus;
}

function formatAuditTime(iso: string | null): string {
  if (!iso) {
    return "No audit yet";
  }
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(iso));
}

function mountLabel(state: MountState): string {
  return state === "MOUNTED" ? "Mounted" : "Unmounted";
}

function cloudLabel(state: CloudLinkState): string {
  if (state === "CONNECTED") {
    return "Connected";
  }
  if (state === "EXPIRED") {
    return "Token Expired";
  }
  return "Offline";
}

function cloudTone(state: CloudLinkState): string {
  if (state === "CONNECTED") {
    return tone(true);
  }
  if (state === "EXPIRED") {
    return "bg-amber-500/15 text-amber-200 ring-amber-500/30";
  }
  return tone(false);
}

function tone(ok: boolean): string {
  return ok
    ? "bg-emerald-500/15 text-emerald-300 ring-emerald-500/30"
    : "bg-zinc-800 text-zinc-400 ring-zinc-700";
}

export function DashboardHeader({ status }: DashboardHeaderProps) {
  return (
    <header className="border-b border-zinc-800 bg-slate-900">
      <div className="flex flex-col gap-4 px-5 py-4 lg:flex-row lg:items-center lg:justify-between">
        <div className="flex items-center gap-3">
          <div className="flex size-10 items-center justify-center rounded-lg bg-emerald-500/15 text-emerald-300 ring-1 ring-emerald-500/30">
            <Shield className="size-5" aria-hidden="true" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h1 className="text-lg font-semibold tracking-tight text-zinc-50">
                Guard Senpai
              </h1>
              <span className="rounded-full bg-zinc-800 px-2 py-0.5 text-[11px] font-medium uppercase tracking-wide text-zinc-300 ring-1 ring-zinc-700">
                Read-only
              </span>
            </div>
            <p className="text-sm text-zinc-400">
              Backup tracker and consistency health
            </p>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <StatusPill
            icon={<HardDrive className="size-3.5" aria-hidden="true" />}
            label={`Local ${mountLabel(status.localMount)}`}
            className={tone(status.localMount === "MOUNTED")}
          />
          <StatusPill
            icon={<Usb className="size-3.5" aria-hidden="true" />}
            label={`External ${mountLabel(status.externalMount)}`}
            className={tone(status.externalMount === "MOUNTED")}
          />
          <StatusPill
            icon={<Cloud className="size-3.5" aria-hidden="true" />}
            label={`OneDrive ${cloudLabel(status.cloudLink)}`}
            className={cloudTone(status.cloudLink)}
          />
          <span className="px-1 text-xs text-zinc-500">
            Last audit {formatAuditTime(status.lastAuditAt)}
          </span>
        </div>
      </div>
    </header>
  );
}

interface StatusPillProps {
  icon: ReactNode;
  label: string;
  className: string;
}

function StatusPill({ icon, label, className }: StatusPillProps) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-1 text-xs font-medium ring-1 ring-inset ${className}`}
    >
      {icon}
      {label}
    </span>
  );
}
