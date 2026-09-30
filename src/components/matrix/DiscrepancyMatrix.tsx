import type { DiscrepancyItem, TargetPresence } from "../../models/types";
import { HealthBadge } from "./HealthBadge";

interface DiscrepancyMatrixProps {
  items: DiscrepancyItem[];
}

function formatBytes(size: number): string {
  if (size < 1024) {
    return `${size} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = size / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

function cellLabel(target: TargetPresence): string {
  if (target.unmounted) {
    return "Unmounted";
  }
  if (target.unscanned) {
    return "Not scanned";
  }
  if (target.offline) {
    return "Offline";
  }
  if (!target.present || target.sizeBytes === null) {
    return "Absent";
  }
  return formatBytes(target.sizeBytes);
}

function cellClass(target: TargetPresence): string {
  if (target.unmounted || target.offline || target.unscanned) {
    return "text-zinc-500";
  }
  if (!target.present) {
    return "text-amber-300/90";
  }
  return "text-zinc-200";
}

export function DiscrepancyMatrix({ items }: DiscrepancyMatrixProps) {
  return (
    <div className="min-w-0 flex-1 p-4">
      <div className="mb-3">
        <h2 className="text-sm font-semibold text-zinc-100">Discrepancy matrix</h2>
        <p className="mt-1 text-xs text-zinc-500">
          Local, external, and cloud compared by path. An unmounted drive stays
          Unmounted and is not treated as a missing file.
        </p>
      </div>

      <div className="overflow-x-auto rounded-xl border border-zinc-800">
        <table className="w-full min-w-[720px] border-collapse text-left text-sm">
          <thead className="bg-slate-900 text-xs uppercase tracking-wide text-zinc-500">
            <tr>
              <th className="px-4 py-3 font-medium">File</th>
              <th className="px-4 py-3 font-medium">Local</th>
              <th className="px-4 py-3 font-medium">External</th>
              <th className="px-4 py-3 font-medium">Cloud</th>
              <th className="px-4 py-3 font-medium">Status</th>
            </tr>
          </thead>
          <tbody>
            {items.length === 0 ? (
              <tr>
                <td colSpan={5} className="px-4 py-10 text-center text-sm text-zinc-500">
                  No audit yet. Run an audit on a watched set.
                </td>
              </tr>
            ) : (
              items.map((item) => (
                <tr key={item.id} className="border-t border-zinc-800 bg-zinc-900/40">
                  <td className="px-4 py-3 align-top">
                    <p className="font-mono text-xs text-zinc-100">{item.relativePath}</p>
                    {item.note ? (
                      <p className="mt-1 max-w-xs text-xs text-zinc-500">{item.note}</p>
                    ) : null}
                  </td>
                  <td className={`px-4 py-3 align-top font-mono text-xs ${cellClass(item.local)}`}>
                    {cellLabel(item.local)}
                  </td>
                  <td className={`px-4 py-3 align-top font-mono text-xs ${cellClass(item.external)}`}>
                    {cellLabel(item.external)}
                  </td>
                  <td className={`px-4 py-3 align-top font-mono text-xs ${cellClass(item.cloud)}`}>
                    {cellLabel(item.cloud)}
                  </td>
                  <td className="px-4 py-3 align-top">
                    <HealthBadge status={item.status} />
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
