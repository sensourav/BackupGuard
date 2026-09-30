import type { AuditLog } from "../../models/types";

interface LogHistoryProps {
  logs: AuditLog[];
}

function formatTime(iso: string): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(iso));
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

function formatDuration(ms: number): string {
  if (ms < 1000) {
    return `${ms} ms`;
  }
  const seconds = ms / 1000;
  if (seconds < 60) {
    return `${seconds.toFixed(1)} s`;
  }
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `${minutes}m ${rest}s`;
}

export function LogHistory({ logs }: LogHistoryProps) {
  return (
    <div className="min-w-0 flex-1 p-4">
      <div className="mb-3">
        <h2 className="text-sm font-semibold text-zinc-100">Log history</h2>
        <p className="mt-1 text-xs text-zinc-500">
          Audit sessions saved in the local database.
        </p>
      </div>

      <div className="overflow-x-auto rounded-xl border border-zinc-800">
        {logs.length === 0 ? (
          <p className="px-4 py-10 text-center text-sm text-zinc-500">
            No audits recorded yet.
          </p>
        ) : (
          <table className="w-full min-w-[640px] border-collapse text-left text-sm">
            <thead className="bg-slate-900 text-xs uppercase tracking-wide text-zinc-500">
              <tr>
                <th className="px-4 py-3 font-medium">When</th>
                <th className="px-4 py-3 font-medium">Set</th>
                <th className="px-4 py-3 font-medium">Summary</th>
                <th className="px-4 py-3 font-medium">Counts</th>
                <th className="px-4 py-3 font-medium">Duration</th>
                <th className="px-4 py-3 font-medium">Read</th>
              </tr>
            </thead>
            <tbody>
              {logs.map((log) => (
                <tr key={log.id} className="border-t border-zinc-800 bg-zinc-900/40">
                  <td className="px-4 py-3 align-top text-xs text-zinc-300">
                    {formatTime(log.timestamp)}
                  </td>
                  <td className="px-4 py-3 align-top text-xs font-medium text-zinc-100">
                    {log.setName}
                  </td>
                  <td className="max-w-xs px-4 py-3 align-top text-xs text-zinc-400">
                    {log.summary}
                  </td>
                  <td className="px-4 py-3 align-top font-mono text-[11px] text-zinc-400">
                    <span className="text-emerald-300">{log.healthy} ok</span>
                    {" · "}
                    <span className="text-amber-300">{log.missing} miss</span>
                    {" · "}
                    <span className="text-rose-300">{log.corrupt} bad</span>
                    {" · "}
                    <span className="text-violet-300">{log.bitRot} rot</span>
                  </td>
                  <td className="px-4 py-3 align-top font-mono text-xs text-zinc-400">
                    {formatDuration(log.durationMs)}
                  </td>
                  <td className="px-4 py-3 align-top font-mono text-xs text-zinc-400">
                    {formatBytes(log.bytesProcessed)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
