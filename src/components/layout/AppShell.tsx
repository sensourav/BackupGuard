import { isTauri } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import {
  addWatchedFolder,
  auditFolderSet,
  connectOneDrive,
  disconnectOneDrive,
  listAuditLogs,
  listDiscrepancies,
  listDriveMounts,
  listWatchedFolders,
  onAuditProgress,
  onDriveMountsChanged,
  onedriveStatus,
  removeWatchedFolder,
  type FolderRow,
} from "../../api/backup";
import { headerStatus, mountForPath } from "../../lib/mounts";
import type {
  AuditLog,
  AuditProgress,
  AuditReport,
  DiscrepancyItem,
  DriveMount,
  MainTab,
  NewWatchedFolder,
  OneDriveStatus,
  WatchedFolderSet,
} from "../../models/types";
import { WatchedFoldersPanel } from "../folders/WatchedFoldersPanel";
import { LogHistory } from "../logs/LogHistory";
import { DiscrepancyMatrix } from "../matrix/DiscrepancyMatrix";
import { DashboardHeader } from "./DashboardHeader";

const tabs: { id: MainTab; label: string }[] = [
  { id: "matrix", label: "Matrix" },
  { id: "history", label: "Log history" },
];

function withMounts(
  folder: FolderRow,
  drives: DriveMount[],
  cloudLink: OneDriveStatus["state"],
): WatchedFolderSet {
  return {
    id: folder.id,
    name: folder.name,
    localPath: folder.localPath,
    externalPath: folder.externalPath,
    cloudPath: folder.cloudPath,
    lastScanned: folder.lastScanned,
    localMount: mountForPath(folder.localPath, drives),
    externalMount: mountForPath(folder.externalPath, drives),
    cloudLink,
  };
}

function auditNotice(report: AuditReport): string {
  const counts = `${report.healthyCount} healthy, ${report.missingCount} missing, ${report.corruptCount} corrupt, ${report.bitrotCount} bit rot`;
  if (report.warnings.length === 0) {
    return counts;
  }
  return `${counts}. ${report.warnings.join(" ")}`;
}

export function AppShell() {
  const [tab, setTab] = useState<MainTab>("matrix");
  const [folders, setFolders] = useState<FolderRow[]>([]);
  const [drives, setDrives] = useState<DriveMount[]>([]);
  const [items, setItems] = useState<DiscrepancyItem[]>([]);
  const [logs, setLogs] = useState<AuditLog[]>([]);
  const [progress, setProgress] = useState<AuditProgress | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cloud, setCloud] = useState<OneDriveStatus>({ state: "OFFLINE", account: null });
  const [cloudBusy, setCloudBusy] = useState(false);

  useEffect(() => {
    if (!isTauri()) {
      setError("Open the Guard Senpai desktop window to save folders and detect drives.");
      return;
    }

    let unlisten: (() => void) | undefined;
    let disposed = false;

    void (async () => {
      try {
        const [nextFolders, nextDrives, nextItems, nextLogs, nextCloud] = await Promise.all([
          listWatchedFolders(),
          listDriveMounts(),
          listDiscrepancies(),
          listAuditLogs(),
          onedriveStatus(),
        ]);
        if (disposed) {
          return;
        }
        setFolders(nextFolders);
        setDrives(nextDrives);
        setItems(nextItems);
        setLogs(nextLogs);
        setCloud(nextCloud);
        const stopDrives = await onDriveMountsChanged((next) => {
          setDrives(next);
        });
        const stopProgress = await onAuditProgress((next) => {
          setProgress(next);
        });
        unlisten = () => {
          stopDrives();
          stopProgress();
        };
      } catch (err) {
        if (!disposed) {
          setError(err instanceof Error ? err.message : String(err));
        }
      }
    })();

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function handleAdd(input: NewWatchedFolder): Promise<boolean> {
    setError(null);
    setNotice(null);
    try {
      const created = await addWatchedFolder(input);
      setFolders((current) =>
        [...current, created].sort((left, right) => left.name.localeCompare(right.name)),
      );
      return true;
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      return false;
    }
  }

  async function handleRemove(id: string) {
    const folder = folders.find((item) => item.id === id);
    const confirmed = window.confirm(
      `Remove ${folder?.name ?? "this set"} from Guard Senpai? Files on disk stay where they are.`,
    );
    if (!confirmed) {
      return;
    }
    setError(null);
    setBusyId(id);
    try {
      await removeWatchedFolder(id);
      setFolders((current) => current.filter((item) => item.id !== id));
      setNotice("Removed the saved set. Backup files were not deleted.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyId(null);
    }
  }

  async function handleAudit(
    id: string,
    scope: "full" | "folder" | "file",
    relativePath?: string,
  ) {
    setError(null);
    setNotice(null);
    setBusyId(id);
    setProgress(null);
    try {
      const report = await auditFolderSet(id, scope, relativePath);
      setFolders((current) =>
        current.map((folder) =>
          folder.id === id ? { ...folder, lastScanned: report.lastScanned } : folder,
        ),
      );
      const [nextItems, nextLogs] = await Promise.all([listDiscrepancies(), listAuditLogs()]);
      setItems(nextItems);
      setLogs(nextLogs);
      setTab("matrix");
      setNotice(auditNotice(report));
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyId(null);
      setProgress(null);
    }
  }

  async function handleConnect() {
    setError(null);
    setCloudBusy(true);
    try {
      setCloud(await connectOneDrive());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCloudBusy(false);
    }
  }

  async function handleDisconnect() {
    setError(null);
    setCloudBusy(true);
    try {
      setCloud(await disconnectOneDrive());
      const nextItems = await listDiscrepancies();
      setItems(nextItems);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setCloudBusy(false);
    }
  }

  const sets = folders.map((folder) => withMounts(folder, drives, cloud.state));

  return (
    <div className="flex min-h-screen flex-col bg-slate-950 text-zinc-100">
      <DashboardHeader status={headerStatus(sets, drives, cloud.state)} />
      {progress ? (
        <div
          className={`border-b px-5 py-2 ${
            progress.phase === "cloud"
              ? "border-sky-500/30 bg-sky-500/10"
              : "border-emerald-500/30 bg-emerald-500/10"
          }`}
        >
          <p className={`text-sm ${progress.phase === "cloud" ? "text-sky-100" : "text-emerald-100"}`}>
            {progress.phase === "cloud"
              ? `OneDrive metadata ${progress.relativePath} — listing`
              : `Scanning ${progress.relativePath} — ${progress.filePercent}%`}
          </p>
          <div className="mt-2 h-1.5 overflow-hidden rounded-full bg-slate-800">
            <div
              className={`h-full rounded-full ${
                progress.phase === "cloud" ? "w-full bg-sky-400" : "bg-emerald-400"
              }`}
              style={progress.phase === "cloud" ? undefined : { width: `${progress.filePercent}%` }}
            />
          </div>
        </div>
      ) : null}
      <div className="flex min-h-0 flex-1 flex-col lg:flex-row">
        <WatchedFoldersPanel
          sets={sets}
          busyId={busyId}
          notice={notice}
          error={error}
          cloud={cloud}
          cloudBusy={cloudBusy}
          onConnect={handleConnect}
          onDisconnect={handleDisconnect}
          onAdd={handleAdd}
          onRemove={handleRemove}
          onAudit={handleAudit}
        />
        <main className="flex min-w-0 flex-1 flex-col">
          <div
            role="tablist"
            aria-label="Audit views"
            className="flex gap-1 border-b border-zinc-800 bg-slate-900 px-4"
          >
            {tabs.map((item) => {
              const selected = tab === item.id;
              return (
                <button
                  key={item.id}
                  type="button"
                  role="tab"
                  id={`tab-${item.id}`}
                  aria-selected={selected}
                  aria-controls={`panel-${item.id}`}
                  onClick={() => setTab(item.id)}
                  className={`border-b-2 px-3 py-3 text-sm font-medium transition-colors ${
                    selected
                      ? "border-emerald-400 text-zinc-50"
                      : "border-transparent text-zinc-500 hover:text-zinc-300"
                  }`}
                >
                  {item.label}
                </button>
              );
            })}
          </div>
          <div
            role="tabpanel"
            id={`panel-${tab}`}
            aria-labelledby={`tab-${tab}`}
            className="min-w-0 flex-1 bg-slate-950"
          >
            {tab === "matrix" ? (
              <DiscrepancyMatrix items={items} />
            ) : (
              <LogHistory logs={logs} />
            )}
          </div>
        </main>
      </div>
    </div>
  );
}
