import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AuditLog,
  AuditProgress,
  AuditReport,
  DiscrepancyItem,
  DriveMount,
  NewWatchedFolder,
  OneDriveStatus,
} from "../models/types";

interface FolderRow {
  id: string;
  name: string;
  localPath: string;
  externalPath: string;
  cloudPath: string;
  lastScanned: string | null;
  createdAt: string;
}

export async function listWatchedFolders(): Promise<FolderRow[]> {
  return invoke<FolderRow[]>("list_watched_folders");
}

export async function addWatchedFolder(input: NewWatchedFolder): Promise<FolderRow> {
  return invoke<FolderRow>("add_watched_folder", {
    name: input.name,
    localPath: input.localPath,
    externalPath: input.externalPath,
    cloudPath: input.cloudPath,
  });
}

export async function removeWatchedFolder(id: string): Promise<void> {
  await invoke("remove_watched_folder", { id });
}

export async function listDriveMounts(): Promise<DriveMount[]> {
  return invoke<DriveMount[]>("list_drive_mounts");
}

export async function auditFolderSet(
  id: string,
  scope: "full" | "folder" | "file",
  relativePath?: string,
): Promise<AuditReport> {
  return invoke<AuditReport>("audit_folder_set", {
    id,
    scope,
    relativePath: relativePath ?? null,
  });
}

export async function listDiscrepancies(): Promise<DiscrepancyItem[]> {
  return invoke<DiscrepancyItem[]>("list_discrepancies");
}

export async function listAuditLogs(): Promise<AuditLog[]> {
  return invoke<AuditLog[]>("list_audit_logs");
}

export async function onedriveStatus(): Promise<OneDriveStatus> {
  return invoke<OneDriveStatus>("onedrive_status");
}

export async function connectOneDrive(): Promise<OneDriveStatus> {
  return invoke<OneDriveStatus>("onedrive_connect");
}

export async function disconnectOneDrive(): Promise<OneDriveStatus> {
  return invoke<OneDriveStatus>("onedrive_disconnect");
}

export function onAuditProgress(
  handler: (progress: AuditProgress) => void,
): Promise<UnlistenFn> {
  return listen<AuditProgress>("audit-progress", (event) => {
    handler(event.payload);
  });
}

export function onDriveMountsChanged(
  handler: (drives: DriveMount[]) => void,
): Promise<UnlistenFn> {
  return listen<DriveMount[]>("drive-mounts-changed", (event) => {
    handler(event.payload);
  });
}

export type { FolderRow };
