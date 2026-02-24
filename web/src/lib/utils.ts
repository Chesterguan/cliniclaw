import { clsx, type ClassValue } from "clsx";

export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

export function formatAge(birthDate: string): string {
  const birth = new Date(birthDate);
  const now = new Date();
  let age = now.getFullYear() - birth.getFullYear();
  if (
    now.getMonth() < birth.getMonth() ||
    (now.getMonth() === birth.getMonth() && now.getDate() < birth.getDate())
  ) {
    age--;
  }
  return `${age}`;
}

export function formatGender(gender: string): string {
  switch (gender?.toLowerCase()) {
    case "male":
      return "M";
    case "female":
      return "F";
    default:
      return gender?.[0]?.toUpperCase() || "U";
  }
}

export function formatDateTime(iso: string): string {
  try {
    return new Date(iso).toLocaleString("en-US", {
      month: "short",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

export function formatTime(iso: string): string {
  try {
    return new Date(iso).toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  } catch {
    return iso;
  }
}

export function elapsedSince(iso: string): string {
  const start = new Date(iso).getTime();
  const now = Date.now();
  const diff = now - start;
  const mins = Math.floor(diff / 60000);
  if (mins < 60) return `${mins}m`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ${mins % 60}m`;
  const days = Math.floor(hours / 24);
  return `${days}d ${hours % 24}h`;
}

export function getPatientName(
  name?: Array<{ family?: string; given?: string[] }>
): string {
  if (!name?.[0]) return "Unknown";
  const n = name[0];
  const given = n.given?.join(" ") || "";
  return `${given} ${n.family || ""}`.trim() || "Unknown";
}

export const PRACTITIONER_ID = "practitioner-001";
