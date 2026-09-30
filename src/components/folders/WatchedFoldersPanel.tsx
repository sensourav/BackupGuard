import { Cloud, FolderOpen, HardDrive, ScanSearch, Trash2, Usb } from "lucide-react";
import { useState, type FormEvent, type ReactNode } from "react";
import type {
  CloudLinkState,
  MountState,
  NewWatchedFolder,
  OneDriveStatus,
  WatchedFolderSet,
} from "../../models/types";

interface WatchedFoldersPanelProps {
  sets: WatchedFolderSet[];
  busyId: string | null;
  notice: string | null;
  error: string | null;
  cloud: OneDriveStatus;
  cloudBusy: boolean;
  onConnect: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onAdd: (input: NewWatchedFolder) => Promise<boolean>;
  onRemove: (id: string) => Promise<void>;
  onAudit: (
    id: string,
    scope: "full" | "folder" | "file",
    relativePath?: string,
  ) => Promise<void>;
}

const emptyForm: NewWatchedFolder = {
  name: "",
  localPath: "",
  externalPath: "",
  cloudPath: "",
};

export function WatchedFoldersPanel({
  sets,
  busyId,
  notice,
  error,
  cloud,
  cloudBusy,
  onConnect,
  onDisconnect,
  onAdd,
  onRemove,
  onAudit,
}: WatchedFoldersPanelProps) {
  const [form, setForm] = useState<NewWatchedFolder>(emptyForm);
  const [adding, setAdding] = useState(false);

  async function handleAdd(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAdding(true);
    try {
      const saved = await onAdd(form);
      if (saved) {
        setForm(emptyForm);
      }
    } finally {
      setAdding(false);
    }
  }

  return (
    <aside className="flex w-full shrink-0 flex-col border-zinc-800 bg-slate-900 lg:max-h-screen lg:w-80 lg:overflow-y-auto lg:border-r">
      <div className="border-b border-zinc-800 px-4 py-3">
        <h2 className="text-sm font-semibold text-zinc-100">Watched folders</h2>
        <p className="mt-1 text-xs text-zinc-500">
          Saved in the local database. Removing a set leaves the backup files on disk.
        </p>
      </div>

      <div className="border-b border-zinc-800 px-4 py-3">
        <p className="text-xs text-zinc-300">{cloudStatusLabel(cloud)}</p>
        <p className="mt-1 text-[11px] text-zinc-500">
          Read-only Files.Read. Register http://127.0.0.1:43821/callback on the Azure app, then put
          the client id in onedrive.client.local.json.
        </p>
        <div className="mt-2 flex gap-2">
          {cloud.state === "CONNECTED" ? null : (
            <button
              type="button"
              disabled={cloudBusy}
              onClick={() => void onConnect()}
              className="rounded-lg bg-sky-500/15 px-3 py-1.5 text-xs font-medium text-sky-100 ring-1 ring-sky-500/30 hover:bg-sky-500/25 disabled:opacity-60"
            >
              {cloudBusy ? "Waiting for sign-in…" : "Connect OneDrive"}
            </button>
          )}
          {cloud.state === "OFFLINE" ? null : (
            <button
              type="button"
              disabled={cloudBusy}
              onClick={() => void onDisconnect()}
              className="rounded-lg px-3 py-1.5 text-xs text-zinc-300 ring-1 ring-zinc-700 hover:bg-zinc-800 disabled:opacity-60"
            >
              Disconnect
            </button>
          )}
        </div>
      </div>

      <form onSubmit={handleAdd} className="flex flex-col gap-2 border-b border-zinc-800 p-4">
        <Field
          label="Name"
          value={form.name}
          onChange={(name) => setForm((current) => ({ ...current, name }))}
        />
        <Field
          label="Local path"
          value={form.localPath}
          onChange={(localPath) => setForm((current) => ({ ...current, localPath }))}
          placeholder="C:/Backups/DJI/"
        />
        <Field
          label="External path"
          value={form.externalPath}
          onChange={(externalPath) => setForm((current) => ({ ...current, externalPath }))}
          placeholder="E:/Backups/DJI/"
        />
        <Field
          label="OneDrive path"
          value={form.cloudPath}
          onChange={(cloudPath) => setForm((current) => ({ ...current, cloudPath }))}
          placeholder="/OneDrive/Backups/DJI/"
        />
        <button
          type="submit"
          disabled={adding}
          className="mt-1 rounded-lg bg-emerald-500/15 px-3 py-2 text-sm font-medium text-emerald-200 ring-1 ring-emerald-500/30 hover:bg-emerald-500/25 disabled:opacity-60"
        >
          {adding ? "Saving…" : "Add set"}
        </button>
      </form>

      {error ? <p className="px-4 pt-3 text-xs text-rose-300">{error}</p> : null}
      {notice ? <p className="px-4 pt-3 text-xs text-zinc-400">{notice}</p> : null}

      <div className="flex flex-col gap-3 p-4">
        {sets.length === 0 ? (
          <div className="rounded-xl border border-dashed border-zinc-700 bg-zinc-900/60 px-4 py-8 text-center">
            <FolderOpen className="mx-auto size-5 text-zinc-500" aria-hidden="true" />
            <p className="mt-3 text-sm text-zinc-300">No watched sets yet</p>
            <p className="mt-1 text-xs text-zinc-500">
              Add a local, external, and OneDrive path to start tracking a backup set.
            </p>
          </div>
        ) : (
          sets.map((set) => (
            <FolderSetCard
              key={set.id}
              set={set}
              busy={busyId === set.id}
              onRemove={onRemove}
              onAudit={onAudit}
            />
          ))
        )}
      </div>
    </aside>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}) {
  const id = `watched-${label.toLowerCase().replace(/\s+/g, "-")}`;
  return (
    <label htmlFor={id} className="flex flex-col gap-1 text-xs text-zinc-400">
      {label}
      <input
        id={id}
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        className="rounded-lg border border-zinc-800 bg-slate-950 px-2.5 py-1.5 font-mono text-xs text-zinc-100 outline-none ring-emerald-500/40 placeholder:text-zinc-600 focus:ring-1"
      />
    </label>
  );
}

function FolderSetCard({
  set,
  busy,
  onRemove,
  onAudit,
}: {
  set: WatchedFolderSet;
  busy: boolean;
  onRemove: (id: string) => Promise<void>;
  onAudit: (
    id: string,
    scope: "full" | "folder" | "file",
    relativePath?: string,
  ) => Promise<void>;
}) {
  const [targetPath, setTargetPath] = useState("");
  return (
    <article className="rounded-xl border border-zinc-800 bg-zinc-900 p-3">
      <div className="mb-3 flex items-center justify-between gap-2">
        <h3 className="text-sm font-medium text-zinc-50">{set.name}</h3>
        <span className="text-[11px] uppercase tracking-wide text-zinc-500">3 targets</span>
      </div>
      <ul className="flex flex-col gap-2">
        <TargetRow
          icon={<HardDrive className="size-3.5" aria-hidden="true" />}
          label="Local"
          path={set.localPath}
          pill={mountPill(set.localMount)}
          ok={set.localMount === "MOUNTED"}
        />
        <TargetRow
          icon={<Usb className="size-3.5" aria-hidden="true" />}
          label="External"
          path={set.externalPath}
          pill={mountPill(set.externalMount)}
          ok={set.externalMount === "MOUNTED"}
        />
        <TargetRow
          icon={<Cloud className="size-3.5" aria-hidden="true" />}
          label="OneDrive"
          path={set.cloudPath}
          pill={cloudPill(set.cloudLink)}
          ok={set.cloudLink === "CONNECTED"}
        />
      </ul>
      <p className="mt-3 text-[11px] text-zinc-500">
        {set.lastScanned ? `Last scan ${formatTime(set.lastScanned)}` : "Not scanned yet"}
      </p>
      <div className="mt-3 flex flex-col gap-2">
        <button
          type="button"
          disabled={busy}
          onClick={() => void onAudit(set.id, "full")}
          className="inline-flex items-center justify-center gap-1.5 rounded-lg bg-slate-950 px-2 py-1.5 text-xs text-zinc-200 ring-1 ring-zinc-700 hover:bg-zinc-800 disabled:opacity-60"
        >
          <ScanSearch className="size-3.5" aria-hidden="true" />
          {busy ? "Auditing…" : "Audit set"}
        </button>
        <label className="flex flex-col gap-1 text-[11px] text-zinc-500">
          Daily folder or file
          <input
            value={targetPath}
            placeholder="2026-09-16 or 2026-09-16/clip.mp4"
            onChange={(event) => setTargetPath(event.target.value)}
            className="rounded-lg border border-zinc-800 bg-slate-950 px-2 py-1.5 font-mono text-[11px] text-zinc-100 outline-none ring-emerald-500/40 placeholder:text-zinc-600 focus:ring-1"
          />
        </label>
        <div className="flex gap-2">
          <button
            type="button"
            disabled={busy || targetPath.trim() === ""}
            onClick={() => void onAudit(set.id, "folder", targetPath.trim())}
            className="flex-1 rounded-lg px-2 py-1.5 text-xs text-zinc-200 ring-1 ring-zinc-700 hover:bg-zinc-800 disabled:opacity-60"
          >
            Audit folder
          </button>
          <button
            type="button"
            disabled={busy || targetPath.trim() === ""}
            onClick={() => void onAudit(set.id, "file", targetPath.trim())}
            className="flex-1 rounded-lg px-2 py-1.5 text-xs text-zinc-200 ring-1 ring-zinc-700 hover:bg-zinc-800 disabled:opacity-60"
          >
            Check file
          </button>
        </div>
        <button
          type="button"
          disabled={busy}
          onClick={() => void onRemove(set.id)}
          className="inline-flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-xs text-rose-300 ring-1 ring-rose-500/30 hover:bg-rose-500/10 disabled:opacity-60"
        >
          <Trash2 className="size-3.5" aria-hidden="true" />
          Remove
        </button>
      </div>
    </article>
  );
}

function formatTime(iso: string): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(new Date(iso));
}

function mountPill(state: MountState): string {
  return state === "MOUNTED" ? "Mounted" : "Unmounted";
}

function cloudStatusLabel(cloud: OneDriveStatus): string {
  if (cloud.state === "CONNECTED") {
    return cloud.account ? `Connected as ${cloud.account}` : "Connected";
  }
  if (cloud.state === "EXPIRED") {
    return "Token Expired";
  }
  return "Signed out";
}

function cloudPill(state: CloudLinkState): string {
  if (state === "CONNECTED") {
    return "Connected";
  }
  if (state === "EXPIRED") {
    return "Token Expired";
  }
  return "Offline";
}

interface TargetRowProps {
  icon: ReactNode;
  label: string;
  path: string;
  pill: string;
  ok: boolean;
}

function TargetRow({ icon, label, path, pill, ok }: TargetRowProps) {
  return (
    <li className="rounded-lg bg-slate-950/70 px-2.5 py-2">
      <div className="flex items-center justify-between gap-2">
        <span className="inline-flex items-center gap-1.5 text-xs font-medium text-zinc-300">
          {icon}
          {label}
        </span>
        <span
          className={`rounded-full px-2 py-0.5 text-[10px] font-medium ring-1 ring-inset ${
            ok
              ? "bg-emerald-500/15 text-emerald-300 ring-emerald-500/30"
              : "bg-zinc-800 text-zinc-400 ring-zinc-700"
          }`}
        >
          {pill}
        </span>
      </div>
      <p className="mt-1 truncate font-mono text-[11px] text-zinc-400" title={path}>
        {path}
      </p>
    </li>
  );
}
