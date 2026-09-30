import type {
  AuditLog,
  DiscrepancyItem,
  SystemStatus,
  TargetPresence,
  WatchedFolderSet,
} from "../models/types";

const unmounted = (): TargetPresence => ({
  present: false,
  sizeBytes: null,
  mtime: null,
  hash: null,
  unmounted: true,
});

export const watchedSets: WatchedFolderSet[] = [
  {
    id: "set-dji",
    name: "DJI",
    localPath: "C:/Backups/DJI/",
    externalPath: "E:/Backups/DJI/",
    cloudPath: "/OneDrive/Backups/DJI/",
    localMount: "MOUNTED",
    externalMount: "UNMOUNTED",
    cloudLink: "CONNECTED",
    lastScanned: null,
  },
];

export const systemStatus: SystemStatus = {
  localMount: "MOUNTED",
  externalMount: "UNMOUNTED",
  cloudLink: "CONNECTED",
  lastAuditAt: "2026-09-26T08:14:00Z",
};

export const discrepancies: DiscrepancyItem[] = [
  {
    id: "item-healthy",
    setId: "set-dji",
    relativePath: "2026-09-16/DJI_0001.MP4",
    status: "HEALTHY",
    local: {
      present: true,
      sizeBytes: 1_284_332_544,
      mtime: "2026-09-16T11:22:04Z",
      hash: "qx-a1c4e9",
    },
    external: unmounted(),
    cloud: {
      present: true,
      sizeBytes: 1_284_332_544,
      mtime: "2026-09-16T11:22:04Z",
      hash: "qx-a1c4e9",
    },
    note: null,
  },
  {
    id: "item-missing",
    setId: "set-dji",
    relativePath: "2026-09-16/DJI_0002.SRT",
    status: "MISSING",
    local: {
      present: true,
      sizeBytes: 18_432,
      mtime: "2026-09-16T11:22:08Z",
      hash: "qx-b77d02",
    },
    external: unmounted(),
    cloud: {
      present: false,
      sizeBytes: null,
      mtime: null,
      hash: null,
    },
    note: "Present on local storage and absent from OneDrive.",
  },
  {
    id: "item-corrupt",
    setId: "set-dji",
    relativePath: "2026-09-18/DJI_0044.MOV",
    status: "CORRUPT",
    local: {
      present: true,
      sizeBytes: 842_112_000,
      mtime: "2026-09-18T16:41:33Z",
      hash: "qx-c90aa1",
    },
    external: unmounted(),
    cloud: {
      present: true,
      sizeBytes: 801_332_000,
      mtime: "2026-09-18T16:41:33Z",
      hash: "qx-c11ff0",
    },
    note: "File exists on every connected target, but byte size and hash differ.",
  },
  {
    id: "item-bitrot",
    setId: "set-dji",
    relativePath: "2026-09-12/DJI_0019.DNG",
    status: "BIT_ROT",
    local: {
      present: true,
      sizeBytes: 48_220_160,
      mtime: "2026-09-12T18:04:11Z",
      hash: "qx-rot-9f",
    },
    external: unmounted(),
    cloud: {
      present: true,
      sizeBytes: 48_220_160,
      mtime: "2026-09-12T18:04:11Z",
      hash: "qx-base-2a",
    },
    note: "Local hash drifted while size and modified time stayed unchanged.",
  },
  {
    id: "item-healthy-still",
    setId: "set-dji",
    relativePath: "2026-09-18/DJI_0045.JPG",
    status: "HEALTHY",
    local: {
      present: true,
      sizeBytes: 6_291_456,
      mtime: "2026-09-18T16:44:02Z",
      hash: "qx-d44018",
    },
    external: unmounted(),
    cloud: {
      present: true,
      sizeBytes: 6_291_456,
      mtime: "2026-09-18T16:44:02Z",
      hash: "qx-d44018",
    },
    note: null,
  },
];

export const auditLogs: AuditLog[] = [
  {
    id: "audit-2026-09-26",
    timestamp: "2026-09-26T08:14:00Z",
    setId: "set-dji",
    setName: "DJI",
    summary: "Compared local storage with OneDrive. External volume was unmounted.",
    healthy: 2,
    missing: 1,
    corrupt: 1,
    bitRot: 1,
    durationMs: 1280,
    bytesProcessed: 2_133_000_000,
  },
  {
    id: "audit-2026-09-20",
    timestamp: "2026-09-20T19:02:41Z",
    setId: "set-dji",
    setName: "DJI",
    summary: "All three targets were connected. One size mismatch on 2026-09-18.",
    healthy: 3,
    missing: 0,
    corrupt: 1,
    bitRot: 0,
    durationMs: 980,
    bytesProcessed: 1_200_000_000,
  },
  {
    id: "audit-2026-09-16",
    timestamp: "2026-09-16T21:10:05Z",
    setId: "set-dji",
    setName: "DJI",
    summary: "Baseline checksums stored for the 2026-09-16 folder.",
    healthy: 4,
    missing: 0,
    corrupt: 0,
    bitRot: 0,
    durationMs: 640,
    bytesProcessed: 800_000_000,
  },
];
