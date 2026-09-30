import type {
  CloudLinkState,
  DriveMount,
  MountState,
  SystemStatus,
  WatchedFolderSet,
} from "../models/types";

export function driveLetter(path: string): string | null {
  const match = /^([A-Za-z]):/.exec(path.trim());
  return match ? `${match[1].toUpperCase()}:` : null;
}

export function mountForPath(path: string, drives: DriveMount[]): MountState {
  const letter = driveLetter(path);
  if (!letter) {
    return "UNMOUNTED";
  }
  const connected = drives.some(
    (drive) =>
      drive.driveLetter.toUpperCase() === letter && drive.status === "CONNECTED",
  );
  return connected ? "MOUNTED" : "UNMOUNTED";
}

export function headerStatus(
  sets: WatchedFolderSet[],
  drives: DriveMount[],
  cloudLink: CloudLinkState,
): SystemStatus {
  const localLetters = sets
    .map((set) => driveLetter(set.localPath))
    .filter((letter): letter is string => letter !== null);
  const externalLetters = sets
    .map((set) => driveLetter(set.externalPath))
    .filter((letter): letter is string => letter !== null);

  const connected = (letter: string) =>
    drives.some(
      (drive) => drive.driveLetter.toUpperCase() === letter && drive.status === "CONNECTED",
    );

  const localMount: MountState =
    localLetters.length === 0
      ? connected("C:")
        ? "MOUNTED"
        : "UNMOUNTED"
      : localLetters.every(connected)
        ? "MOUNTED"
        : "UNMOUNTED";

  const externalMount: MountState =
    externalLetters.length > 0 && externalLetters.every(connected) ? "MOUNTED" : "UNMOUNTED";

  return {
    localMount,
    externalMount,
    cloudLink,
    lastAuditAt: null,
  };
}
