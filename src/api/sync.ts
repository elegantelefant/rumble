// ABOUTME: Typed calls to the Rust host's team-sync commands: matters, versions, invites, forks.
// ABOUTME: Shapes mirror src-tauri/src/sync_matters.rs; ids are hex (matters, devices, versions) or uuids (files).
import { invoke } from "@tauri-apps/api/core";

export type Standing = "admin" | "member" | "waiting";

export type MatterSummary = {
  id: string;
  name: string;
  folder: string;
  standing: Standing;
  waitingFor: string | null;
};

export type VersionView = {
  id: string;
  label: string;
  author: string;
  mine: boolean;
  savedAtMs: number;
  head: boolean;
};

export type FileView = {
  id: string;
  name: string;
  openable: boolean;
  versions: VersionView[];
  forkedFrom: string | null;
};

export type RequestView = { device: string; name: string; safetyNumber: string | null; contested: boolean };
export type MemberView = { device: string; name: string; admin: boolean; mine: boolean };

export type MatterDetail = {
  matter: MatterSummary;
  files: FileView[];
  members: MemberView[];
  requests: RequestView[];
  safetyNumber: string | null;
  confirmed: boolean;
  needsSecondAdmin: boolean;
};

export const listMatters = () => invoke<MatterSummary[]>("sync_matters");
export const getMatter = (id: string) => invoke<MatterDetail>("sync_matter", { id });
/** `null` when the folder picker is dismissed. */
export const createMatter = (yourName: string) => invoke<MatterSummary | null>("sync_create_matter", { yourName });
export const joinMatter = (link: string, yourName: string) =>
  invoke<MatterSummary | null>("sync_join_matter", { link, yourName });
/** `false` when the file picker is dismissed. */
export const addFile = (id: string) => invoke<boolean>("sync_add_file", { id });
/** `true` when opened in its app, `false` when shown in its folder instead. */
export const openFile = (id: string, fileId: string) => invoke<boolean>("sync_open_file", { id, fileId });
export const combineFile = (id: string, fileId: string) => invoke<boolean>("sync_combine", { id, fileId });
export const resolveFork = (id: string, fileId: string, version: string) =>
  invoke<null>("sync_resolve", { id, fileId, version });
export const createInvite = (id: string, inviteeName: string) => invoke<string>("sync_invite", { id, inviteeName });
export const confirmNumber = (id: string, number: string) => invoke<null>("sync_confirm", { id, number });
export const approveRequest = (id: string, device: string, number: string) =>
  invoke<null>("sync_approve", { id, device, number });
export const makeAdmin = (id: string, device: string) => invoke<null>("sync_promote", { id, device });

export const errorText = (error: unknown) => (typeof error === "string" ? error : String(error));
