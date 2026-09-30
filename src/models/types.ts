export type HealthStatus = "HEALTHY" | "MISSING" | "CORRUPT" | "BIT_ROT";

export type MountState = "MOUNTED" | "UNMOUNTED";

export type CloudLinkState = "CONNECTED" | "OFFLINE" | "EXPIRED";

export type MainTab = "matrix" | "history";

export interface WatchedFolderSet {
  id: string;
  name: string;
  localPath: string;
  externalPath: string;
  cloudPath: string;
  localMount: MountState;
  externalMount: MountState;
  cloudLink: CloudLinkState;
  lastScanned: string | null;
}

export interface DriveMount {
  driveLetter: string;
  label: string;
  status: "CONNECTED" | "UNMOUNTED";
  updatedAt: string;
}

export interface NewWatchedFolder {
  name: string;
  localPath: string;
  externalPath: string;
  cloudPath: string;
}

export interface AuditProgress {
  relativePath: string;
  filePercent: number;
  bytesProcessed: number;
  totalBytes: number;
  phase: "hash" | "cloud";
}

export interface OneDriveStatus {
  state: CloudLinkState;
  account: string | null;
}

export interface AuditReport {
  folderId: string;
  lastScanned: string;
  durationMs: number;
  bytesProcessed: number;
  totalFiles: number;
  healthyCount: number;
  missingCount: number;
  corruptCount: number;
  bitrotCount: number;
  warnings: string[];
}

export interface TargetPresence {
  present: boolean;
  sizeBytes: number | null;
  mtime: string | null;
  hash: string | null;
  unmounted?: boolean;
  offline?: boolean;
  unscanned?: boolean;
}

export interface DiscrepancyItem {
  id: string;
  setId: string;
  relativePath: string;
  status: HealthStatus;
  local: TargetPresence;
  external: TargetPresence;
  cloud: TargetPresence;
  note: string | null;
}

export interface AuditLog {
  id: string;
  timestamp: string;
  setId: string;
  setName: string;
  summary: string;
  healthy: number;
  missing: number;
  corrupt: number;
  bitRot: number;
  durationMs: number;
  bytesProcessed: number;
}

export interface SystemStatus {
  localMount: MountState;
  externalMount: MountState;
  cloudLink: CloudLinkState;
  lastAuditAt: string | null;
}
