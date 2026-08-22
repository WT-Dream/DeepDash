import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  DshState,
  EnvironmentInfo,
  LauncherConfig,
  OperationProgress,
  DshVersion,
} from "./types";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export const inTauri = typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);

const demoEnvironment: EnvironmentInfo = {
  node: { found: true, path: "node", version: "v24.18.0" },
  npm: { found: true, path: "npm", version: "11.4.2" },
  prefix: "本机 npm 默认 global prefix",
  dsh: { found: true, path: "dsh", version: "0.1.0" },
  status: "ready",
  checkedAt: new Date().toISOString(),
};

const demoVersions: DshVersion[] = [
  {
    version: "0.1.0",
    tags: ["latest", "当前", "稳定"],
    prerelease: false,
    stable: true,
    current: true,
    installed: true,
  },
  {
    version: "0.2.0-rc.1",
    tags: ["next", "预发布"],
    prerelease: true,
    stable: false,
    current: false,
    installed: false,
  },
];

export async function getConfig(): Promise<LauncherConfig> {
  if (!inTauri) return { port: 3080, theme: "system" };
  return invoke<LauncherConfig>("get_launcher_config");
}

export async function saveConfig(config: LauncherConfig): Promise<LauncherConfig> {
  if (!inTauri) return config;
  return invoke<LauncherConfig>("save_launcher_config", { config });
}

export async function openDataDirectory(): Promise<void> {
  if (!inTauri) return;
  await invoke("open_data_directory");
}

export async function detectEnvironment(): Promise<EnvironmentInfo> {
  if (!inTauri) return { ...demoEnvironment, checkedAt: new Date().toISOString() };
  return invoke<EnvironmentInfo>("detect_environment");
}

export async function getVersions(): Promise<DshVersion[]> {
  if (!inTauri) return demoVersions;
  return invoke<DshVersion[]>("get_dsh_versions");
}

export async function getCurrentVersion(): Promise<string | undefined> {
  if (!inTauri) return "0.1.0";
  return invoke<string | undefined>("get_dsh_current_version");
}

export async function getStatus(): Promise<DshState> {
  if (!inTauri) return { status: "readyToStart", currentVersion: "0.1.0" };
  return invoke<DshState>("get_dsh_status");
}

export async function installOrSwitch(version: string): Promise<DshState> {
  if (!inTauri) {
    await new Promise((resolve) => window.setTimeout(resolve, 500));
    return { status: "readyToStart", currentVersion: version };
  }
  return invoke<DshState>("install_or_switch_dsh_version", { version });
}

export async function cancelPackageOperation(): Promise<DshState> {
  if (!inTauri) return { status: "readyToStart", currentVersion: "0.1.0" };
  return invoke<DshState>("cancel_package_operation");
}

export async function startDsh(port: number): Promise<DshState> {
  if (!inTauri) {
    await new Promise((resolve) => window.setTimeout(resolve, 800));
    return { status: "running", port, url: `http://127.0.0.1:${port}`, currentVersion: "0.1.0" };
  }
  return invoke<DshState>("start_dsh", { port });
}

export async function stopDsh(): Promise<DshState> {
  if (!inTauri) return { status: "stopped" };
  return invoke<DshState>("stop_dsh");
}

export async function subscribeState(handler: (event: DshState) => void): Promise<UnlistenFn | undefined> {
  if (!inTauri) return undefined;
  return listen<DshState>("launcher://dsh-state", (event) => handler(event.payload));
}

export async function subscribeProgress(handler: (event: OperationProgress) => void): Promise<UnlistenFn | undefined> {
  if (!inTauri) return undefined;
  return listen<OperationProgress>("launcher://operation-progress", (event) => handler(event.payload));
}
