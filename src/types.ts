export type ViewName = "dashboard" | "versions" | "settings";

export type DshLifecycleStatus =
  | "notInstalled"
  | "readyToStart"
  | "installing"
  | "switchingVersion"
  | "starting"
  | "running"
  | "stopping"
  | "stopped"
  | "portConflict"
  | "startFailed"
  | "startupTimeout";

export type ThemeMode = "system" | "light" | "dark";

export interface LauncherConfig {
  port: number;
  theme: ThemeMode;
}

export interface ToolInfo {
  found: boolean;
  path?: string;
  version?: string;
  error?: string;
}

export interface EnvironmentInfo {
  node: ToolInfo;
  npm: ToolInfo;
  prefix?: string;
  dsh: ToolInfo;
  status: "ready" | "missingNode" | "brokenNode" | "missingNpm" | "brokenNpm" | "missingDsh" | "brokenDsh";
  checkedAt: string;
}

export interface DshVersion {
  version: string;
  tags: string[];
  prerelease: boolean;
  stable: boolean;
  current: boolean;
  installed: boolean;
  publishedAt?: string;
}

export interface LauncherError {
  kind: string;
  message: string;
  detail?: string;
  action?: string;
  port?: number;
}

export interface DshState {
  status: DshLifecycleStatus;
  port?: number;
  url?: string;
  currentVersion?: string;
  error?: LauncherError;
}

export interface OperationProgress {
  operation: "install" | "switch" | "start" | "stop";
  phase: string;
  message: string;
  percent?: number;
}
