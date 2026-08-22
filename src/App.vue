<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  Activity,
  AlertCircle,
  ArrowUpCircle,
  Check,
  ChevronRight,
  CircleHelp,
  Copy,
  ExternalLink,
  FolderOpen,
  Gauge,
  LoaderCircle,
  MonitorCog,
  PackageCheck,
  Play,
  RefreshCw,
  Settings2,
  Terminal,
  Wifi,
  X,
} from "lucide-vue-next";
import QRCode from "qrcode";
import {
  cancelPackageOperation,
  detectEnvironment,
  getConfig,
  getCurrentVersion,
  getLanHosts,
  getStatus,
  getVersions,
  installOrSwitch,
  openDataDirectory,
  saveConfig,
  startDsh,
  stopDsh,
  subscribeProgress,
  subscribeState,
  inTauri,
} from "./bridge";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  DshLifecycleStatus,
  DshState,
  DshVersion,
  EnvironmentInfo,
  LauncherConfig,
  LauncherError,
  LanHost,
  OperationProgress,
  ThemeMode,
  ViewName,
} from "./types";
import mascotImage from "./assets/dsh-mascot.png";

const activeView = ref<ViewName>("dashboard");
const config = ref<LauncherConfig>({ port: 3080, theme: "system", lanEnabled: false });
const environment = ref<EnvironmentInfo>();
const versions = ref<DshVersion[]>([]);
const state = ref<DshState>({ status: "stopped" });
const currentVersion = ref<string>();
const progress = ref<OperationProgress>();
const error = ref<LauncherError>();
const loading = ref(true);
const refreshing = ref(false);
const busy = ref(false);
const portDraft = ref(3080);
const lanEnabledDraft = ref(false);
const lanHostDraft = ref("");
const lanHosts = ref<LanHost[]>([]);
const configSaved = ref(false);
const dshUrl = ref<string>();
const lanQrCode = ref<string>();
const lanPanelOpen = ref(false);
const unlisten: Array<(() => void) | undefined> = [];
let systemThemeMedia: MediaQueryList | undefined;

const statusMeta: Record<DshLifecycleStatus, { label: string; tone: string }> = {
  notInstalled: { label: "未安装", tone: "muted" },
  readyToStart: { label: "已就绪", tone: "ready" },
  installing: { label: "安装中", tone: "working" },
  switchingVersion: { label: "切换中", tone: "working" },
  starting: { label: "启动中", tone: "working" },
  running: { label: "运行中", tone: "running" },
  stopping: { label: "停止中", tone: "working" },
  stopped: { label: "已停止", tone: "muted" },
  portConflict: { label: "端口冲突", tone: "danger" },
  startFailed: { label: "启动失败", tone: "danger" },
  startupTimeout: { label: "启动超时", tone: "danger" },
};

const status = computed(() => statusMeta[state.value.status]);
const isRunning = computed(() => state.value.status === "running");
const isStarting = computed(() => ["starting", "stopping"].includes(state.value.status));
const hasEnvironmentError = computed(() => ["missingNode", "brokenNode", "missingNpm", "brokenNpm"].includes(environment.value?.status ?? ""));
const latestVersion = computed(() => versions.value.find((item) => item.tags.includes("latest"))?.version);
const canStart = computed(() => Boolean(environment.value?.dsh.found) && !busy.value && !isStarting.value && !isRunning.value);
const canManageVersions = computed(() => Boolean(environment.value?.npm.found) && !busy.value);
const themeOptions: Array<[ThemeMode, string]> = [["system", "跟随系统"], ["light", "浅色"], ["dark", "深色"]];
const lanAccessUrl = computed(() => state.value.lanUrl);
const dshBindAddress = computed(() => lanAccessUrl.value?.replace(/^https?:\/\//, "") ?? `127.0.0.1:${config.value.port}`);

function resolvedTheme(mode: ThemeMode) {
  return mode === "system"
    ? (window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light")
    : mode;
}

function applyTheme(mode: ThemeMode) {
  const resolved = resolvedTheme(mode);
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
  if (inTauri) {
    void getCurrentWindow().setTheme(resolved).catch(() => undefined);
  }
}

function handleSystemThemeChange() {
  if (config.value.theme === "system") applyTheme("system");
}

function errorTitle(value?: LauncherError) {
  if (!value) return "操作未完成";
  const titles: Record<string, string> = {
    nodeMissing: "未检测到 Node.js",
    npmMissing: "npm 不可用",
    dshMissing: "未安装 DSH",
    dshBroken: "DSH 命令不可执行",
    portConflict: "端口已被占用",
    startupTimeout: "DSH 启动超时",
    npmInstallFailed: "npm 安装失败",
    invalidPort: "端口无效",
    processExited: "DSH 进程已退出",
    invalidTheme: "主题设置无效",
    lanHostRequired: "请选择局域网地址",
    invalidLanHost: "局域网地址无效",
    lanHostUnavailable: "局域网地址不可用",
    lanDiscoveryFailed: "无法读取局域网地址",
    qrCodeFailed: "二维码生成失败",
    dataDirectoryUnavailable: "数据目录不可用",
    dataDirectoryOpenFailed: "无法打开数据目录",
  };
  return titles[value.kind] ?? "操作未完成";
}

async function refreshAll() {
  refreshing.value = true;
  error.value = undefined;
  try {
    const [detected, current, nextState] = await Promise.all([
      detectEnvironment(),
      getCurrentVersion(),
      getStatus(),
    ]);
    environment.value = detected;
    currentVersion.value = current;
    state.value = { ...nextState, currentVersion: current ?? nextState.currentVersion };
    dshUrl.value = nextState.url;
    portDraft.value = config.value.port;
    try {
      versions.value = await getVersions();
    } catch (cause) {
      error.value = normalizeError(cause);
    }
  } catch (cause) {
    error.value = normalizeError(cause);
  } finally {
    refreshing.value = false;
    loading.value = false;
  }
}

function normalizeError(cause: unknown): LauncherError {
  if (typeof cause === "object" && cause !== null && "kind" in cause && "message" in cause) {
    return cause as LauncherError;
  }
  return { kind: "unknown", message: String(cause) };
}

async function updateState(next: DshState) {
  const previousLanUrl = state.value.lanUrl;
  state.value = next;
  currentVersion.value = next.currentVersion ?? currentVersion.value;
  dshUrl.value = next.status === "running" ? next.url : undefined;
  if (next.error) error.value = next.error;
  if (!next.lanUrl) {
    lanQrCode.value = undefined;
    lanPanelOpen.value = false;
  } else if (next.lanUrl !== previousLanUrl) {
    lanQrCode.value = undefined;
  }
}

async function toggleLanPanel() {
  if (lanPanelOpen.value) {
    lanPanelOpen.value = false;
    return;
  }
  if (!lanAccessUrl.value) return;
  if (!lanQrCode.value) {
    try {
      lanQrCode.value = await QRCode.toDataURL(lanAccessUrl.value, {
        margin: 1,
        width: 224,
        errorCorrectionLevel: "M",
      });
    } catch {
      error.value = { kind: "qrCodeFailed", message: "无法生成二维码，请使用下方地址访问。" };
    }
  }
  lanPanelOpen.value = true;
}

async function refreshLanHosts() {
  lanHosts.value = await getLanHosts();
  if (!lanHosts.value.some((host) => host.address === lanHostDraft.value) && lanHosts.value.length) {
    lanHostDraft.value = lanHosts.value[0].address;
  }
}

async function refreshLanHostsFromSettings() {
  try {
    await refreshLanHosts();
  } catch (cause) {
    error.value = normalizeError(cause);
  }
}

async function copyLanAddress() {
  if (!lanAccessUrl.value) return;
  try {
    await navigator.clipboard.writeText(lanAccessUrl.value);
  } catch {
    error.value = { kind: "clipboardWriteFailed", message: "无法复制地址，请手动输入手机访问地址。" };
  }
}

async function start() {
  if (!canStart.value) return;
  error.value = undefined;
  busy.value = true;
  try {
    const saved = await saveConfig({ ...config.value, port: config.value.port });
    config.value = saved;
    await updateState(await startDsh(config.value.port));
  } catch (cause) {
    error.value = normalizeError(cause);
  } finally {
    progress.value = undefined;
    busy.value = false;
  }
}

async function stop() {
  if (busy.value || !["running", "starting", "portConflict", "startFailed", "startupTimeout"].includes(state.value.status)) return;
  busy.value = true;
  try {
    await updateState(await stopDsh());
  } catch (cause) {
    error.value = normalizeError(cause);
  } finally {
    progress.value = undefined;
    busy.value = false;
  }
}

async function restart() {
  if (isRunning.value) await stop();
  await start();
}

async function chooseVersion(item: DshVersion) {
  if (!canManageVersions.value || item.current) return;
  const confirmed = window.confirm(`切换到 ${item.version} 将替换当前全局激活版本，继续吗？`);
  if (!confirmed) return;
  busy.value = true;
  error.value = undefined;
  progress.value = { operation: item.version === latestVersion.value ? "install" : "switch", phase: "prepare", message: "正在准备 npm 操作" };
  try {
    await updateState(await installOrSwitch(item.version));
    currentVersion.value = item.version;
    versions.value = versions.value.map((version) => ({ ...version, current: version.version === item.version, installed: version.version === item.version }));
    await refreshAll();
  } catch (cause) {
    const mapped = normalizeError(cause);
    if (mapped.kind !== "operationCanceled") error.value = mapped;
  } finally {
    progress.value = undefined;
    busy.value = false;
  }
}

async function cancelOperation() {
  if (!busy.value) return;
  try {
    await updateState(await cancelPackageOperation());
  } catch (cause) {
    error.value = normalizeError(cause);
  }
  progress.value = undefined;
  busy.value = false;
}

async function saveSettings() {
  if (!Number.isInteger(portDraft.value) || portDraft.value < 1 || portDraft.value > 65535) {
    error.value = { kind: "invalidPort", message: "请输入 1 到 65535 之间的整数端口。" };
    return;
  }
  try {
    if (lanEnabledDraft.value && !lanHostDraft.value) {
      error.value = { kind: "lanHostRequired", message: "请先选择手机访问使用的局域网地址。" };
      return;
    }
    config.value = await saveConfig({
      port: portDraft.value,
      theme: config.value.theme,
      lanEnabled: lanEnabledDraft.value,
      lanHost: lanEnabledDraft.value ? lanHostDraft.value : undefined,
    });
    configSaved.value = true;
    window.setTimeout(() => (configSaved.value = false), 2200);
  } catch (cause) {
    error.value = normalizeError(cause);
  }
}

async function showDataDirectory() {
  try {
    await openDataDirectory();
  } catch (cause) {
    error.value = normalizeError(cause);
  }
}

async function changeTheme(mode: ThemeMode) {
  config.value.theme = mode;
  applyTheme(mode);
  try {
    config.value = await saveConfig({ ...config.value, port: config.value.port });
    configSaved.value = true;
    window.setTimeout(() => (configSaved.value = false), 2200);
  } catch (cause) {
    error.value = normalizeError(cause);
  }
}

function environmentMessage() {
  switch (environment.value?.status) {
    case "missingNode": return "Node.js 未安装。安装后重新检测环境。";
    case "brokenNode": return "Node.js 不可用：版本命令执行失败。";
    case "missingNpm": return "npm 未安装。安装 Node.js 后重新检测环境。";
    case "brokenNpm": return "npm 不可用：无法读取版本或全局 prefix。";
    case "missingDsh": return "DSH 未安装，可前往版本管理页安装。";
    case "brokenDsh": return "DSH 不可用：命令已找到但无法读取版本。";
    default: return "环境已准备好。";
  }
}

function go(view: ViewName) {
  activeView.value = view;
}

onMounted(async () => {
  config.value = await getConfig();
  applyTheme(config.value.theme);
  systemThemeMedia = window.matchMedia?.("(prefers-color-scheme: dark)");
  systemThemeMedia?.addEventListener("change", handleSystemThemeChange);
  portDraft.value = config.value.port;
  lanEnabledDraft.value = config.value.lanEnabled;
  lanHostDraft.value = config.value.lanHost ?? "";
  try {
    await refreshLanHosts();
  } catch (cause) {
    error.value = normalizeError(cause);
  }
  unlisten.push(await subscribeState(updateState));
  unlisten.push(await subscribeProgress((event) => (progress.value = event)));
  await refreshAll();
});

onUnmounted(() => {
  unlisten.forEach((remove) => remove?.());
  systemThemeMedia?.removeEventListener("change", handleSystemThemeChange);
});
</script>

<template>
  <div :class="['app-shell', { 'dsh-focused-shell': isRunning && activeView === 'dashboard' }]">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark"><img :src="mascotImage" alt="DSH" /></div>
        <div>
          <strong>DeepDash</strong>
          <span>DeepSeek Harness</span>
        </div>
      </div>

      <nav class="nav-list" aria-label="主导航">
        <button :class="['nav-item', { active: activeView === 'dashboard' }]" @click="go('dashboard')">
          <Gauge :size="18" /> <span>启动面板</span>
        </button>
        <button :class="['nav-item', { active: activeView === 'versions' }]" @click="go('versions')">
          <PackageCheck :size="18" /> <span>版本管理</span>
          <span v-if="latestVersion && currentVersion !== latestVersion" class="nav-dot" />
        </button>
        <button :class="['nav-item', { active: activeView === 'mobile' }]" @click="go('mobile')">
          <Wifi :size="18" /> <span>手机连接</span>
          <span v-if="isRunning && lanAccessUrl" class="nav-dot ready-dot" />
        </button>
        <button :class="['nav-item', { active: activeView === 'settings' }]" @click="go('settings')">
          <Settings2 :size="18" /> <span>设置</span>
        </button>
      </nav>

      <div class="sidebar-foot">
        <div class="side-status">
          <span :class="['status-dot', status.tone]" />
          <span>{{ status.label }}</span>
        </div>
        <span class="app-version">DeepDash 1.0.4</span>
      </div>
    </aside>

    <main :class="['main-content', { 'dsh-focused': isRunning && activeView === 'dashboard' }]">
      <header v-if="!(isRunning && activeView === 'dashboard')" class="topbar">
        <div>
          <p class="eyebrow">全局版本管理器</p>
          <h1>{{ activeView === 'dashboard' ? '启动面板' : activeView === 'versions' ? '版本管理' : activeView === 'mobile' ? '手机连接' : '偏好设置' }}</h1>
        </div>
      </header>

      <div v-if="error" class="alert error-alert" role="alert">
        <AlertCircle :size="19" />
        <div class="alert-content">
          <strong>{{ errorTitle(error) }}</strong>
          <span>{{ error.message }}</span>
          <small v-if="error.detail">{{ error.detail }}</small>
          <small v-if="error.port">目标端口：{{ error.port }}</small>
        </div>
        <button class="alert-close" title="关闭提示" @click="error = undefined"><X :size="16" /></button>
      </div>

      <div v-if="loading" class="loading-state">
        <LoaderCircle class="spinning" :size="24" />
        <span>正在检测本机环境...</span>
      </div>

      <template v-else-if="activeView === 'dashboard'">
        <section v-if="!isRunning" class="hero-panel">
          <div class="hero-copy">
            <span class="section-kicker"><Activity :size="15" /> 当前实例</span>
            <h2>准备启动 DSH Web</h2>
            <p>启动器会使用当前全局激活版本，并等待本地 Web 服务健康后再加载。</p>
            <div class="hero-actions">
              <button class="button primary large" :disabled="!canStart" @click="start"><Play :size="17" />启动服务</button>
              <button class="button secondary large" :disabled="!canStart" @click="restart"><RefreshCw :size="17" />重启服务</button>
              <button class="text-button" @click="go('settings')">调整端口 <ChevronRight :size="15" /></button>
            </div>
          </div>
          <div class="runtime-orbit" :class="status.tone">
            <div class="orbit-ring ring-one" />
            <div class="orbit-ring ring-two" />
            <div class="runtime-icon"><Wifi :size="31" /></div>
            <span>{{ status.label }}</span>
          </div>
        </section>

        <section v-if="!isRunning" class="content-section">
          <div class="section-heading">
            <div><h2>运行概览</h2><p>启动器只管理自己的服务进程和配置。</p></div>
            <span class="privacy-note"><MonitorCog :size="15" />{{ lanAccessUrl ? `局域网监听：${dshBindAddress}` : '仅监听本机回环地址' }}</span>
          </div>
          <div class="metrics-grid">
            <div class="metric-item"><span class="metric-label">当前版本</span><strong>{{ currentVersion ?? '未检测到' }}</strong><span class="metric-caption">全局激活版本</span></div>
            <div class="metric-item"><span class="metric-label">服务地址</span><strong>{{ dshBindAddress }}</strong><span class="metric-caption">DSH Web 地址</span></div>
            <div class="metric-item"><span class="metric-label">npm 通道</span><strong>{{ latestVersion ?? '读取中' }}</strong><span class="metric-caption">官方 latest 标签</span></div>
            <div class="metric-item"><span class="metric-label">可用版本</span><strong>{{ versions.length || '未读取到' }}</strong><span class="metric-caption">官方 registry 版本数</span></div>
          </div>
        </section>

        <section v-if="!isRunning" class="content-section environment-section">
          <div class="section-heading"><div><h2>环境检测</h2><p>{{ environmentMessage() }}</p></div><button class="text-button" @click="go('settings')">查看详情 <ChevronRight :size="15" /></button></div>
          <div class="environment-row">
            <div :class="['environment-check', { failed: !environment?.node.found || !environment?.node.version }]" ><span class="check-icon"><Check v-if="environment?.node.found && environment?.node.version" :size="14" /><X v-else :size="14" /></span><div><strong>Node.js</strong><small>{{ environment?.node.found ? (environment.node.version ?? '不可用：版本检测失败') : '未安装' }}</small></div></div>
            <div :class="['environment-check', { failed: !environment?.npm.found || !environment?.npm.version }]" ><span class="check-icon"><Check v-if="environment?.npm.found && environment?.npm.version" :size="14" /><X v-else :size="14" /></span><div><strong>npm</strong><small>{{ environment?.npm.found ? (environment.npm.version ?? '不可用：版本检测失败') : '未安装' }}</small></div></div>
            <div :class="['environment-check', { failed: !environment?.dsh.found || !environment?.dsh.version }]" ><span class="check-icon"><Check v-if="environment?.dsh.found && environment?.dsh.version" :size="14" /><X v-else :size="14" /></span><div><strong>DSH</strong><small>{{ environment?.dsh.found ? (environment.dsh.version ?? '不可用：版本检测失败') : '未安装' }}</small></div></div>
          </div>
        </section>

        <section v-if="dshUrl" class="embedded-view">
          <iframe :src="dshUrl" title="DSH Web 界面" class="dsh-frame" allow="autoplay; clipboard-read; clipboard-write; display-capture; fullscreen; microphone; notifications" allowfullscreen />
        </section>
      </template>

      <template v-else-if="activeView === 'versions'">
        <section class="page-intro"><div><p class="eyebrow">版本选择</p><h2>选择全局激活版本</h2><p>每次安装或切换都会替换本机 npm 的当前全局版本。</p></div><button class="button secondary" :disabled="refreshing" @click="refreshAll"><RefreshCw :size="16" />刷新列表</button></section>
        <div v-if="hasEnvironmentError" class="inline-warning"><CircleHelp :size="18" /><span>{{ environmentMessage() }} 版本安装操作暂不可执行。</span><button class="text-button" @click="go('settings')">去设置</button></div>
        <section class="version-panel">
          <div class="version-panel-head"><div><h2>DSH 版本</h2><span>{{ versions.length }} 个版本来自官方 registry</span></div><span v-if="progress" class="operation-state"><LoaderCircle :size="15" class="spinning" />{{ progress.message }} <button class="text-button cancel-button" @click="cancelOperation">取消</button></span></div>
          <div v-if="versions.length" class="version-list">
            <div v-for="item in versions" :key="item.version" class="version-row">
              <div class="version-number"><span class="version-emphasis">{{ item.version }}</span><span v-if="item.stable" class="type-label stable">稳定</span><span v-else class="type-label prerelease">预发布</span></div>
              <div class="version-tags"><span v-for="tag in item.tags" :key="tag" :class="['version-tag', { highlight: ['latest', 'next', '当前'].includes(tag) }]">{{ tag }}</span></div>
              <div class="version-action"><span v-if="item.current" class="active-version"><Check :size="15" />当前版本</span><button v-else class="button secondary small" :disabled="!canManageVersions" @click="chooseVersion(item)"><ArrowUpCircle :size="15" />切换到此版本</button></div>
            </div>
          </div>
          <div v-else class="empty-state"><PackageCheck :size="28" /><strong>暂无版本数据</strong><span>检查网络连接后重试。</span><button class="button secondary" @click="refreshAll">重试</button></div>
        </section>
      </template>

      <template v-else-if="activeView === 'mobile'">
        <section class="page-intro"><div><p class="eyebrow">局域网访问</p><h2>用手机继续当前 DSH 会话</h2><p>手机和电脑连接同一个可信 Wi-Fi 后，扫码即可打开 DSH Web。</p></div></section>
        <div class="mobile-connect-layout">
          <section class="settings-panel mobile-connect-panel">
            <div class="panel-title"><Wifi :size="18" /><div><h2>手机连接</h2><p>二维码只在你点击显示时生成。</p></div></div>
            <template v-if="!isRunning">
              <div class="inline-warning"><CircleHelp :size="18" /><span>DSH 服务尚未运行，请先启动服务。</span><button class="text-button" @click="go('dashboard')">去启动</button></div>
            </template>
            <template v-else-if="lanAccessUrl">
              <div class="mobile-connect-ready"><span class="status-dot running" />局域网访问已开启</div>
              <button class="button primary mobile-qr-toggle" @click="toggleLanPanel"><Wifi :size="16" />{{ lanPanelOpen ? '隐藏二维码' : '显示二维码' }}</button>
              <div v-if="lanPanelOpen" class="mobile-qr-area">
                <img v-if="lanQrCode" :src="lanQrCode" alt="手机访问 DSH 的二维码" class="lan-qr-code" />
                <span v-else class="qr-loading"><LoaderCircle :size="17" class="spinning" />正在生成二维码...</span>
                <code>{{ lanAccessUrl }}</code>
                <button class="button secondary" @click="copyLanAddress"><Copy :size="15" />复制访问地址</button>
                <button class="text-button" @click="lanPanelOpen = false"><X :size="15" />关闭二维码</button>
              </div>
            </template>
            <template v-else>
              <div class="inline-warning"><CircleHelp :size="18" /><span>尚未启用手机局域网访问，请在下方配置并保存。</span></div>
            </template>
            <div class="mobile-settings-divider" />
            <div class="mobile-setting-group">
              <div class="toggle-row mobile-toggle-row"><div><strong>手机局域网访问</strong><small>只在可信私有 Wi-Fi 下启用。</small></div><input v-model="lanEnabledDraft" class="toggle-input" type="checkbox" aria-label="启用手机局域网访问" /></div>
              <div v-if="lanEnabledDraft" class="lan-host-setting"><label class="field-label" for="mobile-lan-host">本机局域网地址</label><div class="lan-host-control"><select id="mobile-lan-host" v-model="lanHostDraft" class="lan-host-select"><option v-for="host in lanHosts" :key="host.address" :value="host.address">{{ host.name }} - {{ host.address }}</option></select><button class="icon-button" title="刷新局域网地址" aria-label="刷新局域网地址" @click="refreshLanHostsFromSettings"><RefreshCw :size="15" /></button></div><small v-if="lanHosts.length" class="field-help warning-help">同一网络内的设备可操作当前项目，请勿在公共 Wi‑Fi 或公网暴露此端口。</small><small v-else class="field-help warning-help">未发现可用私有 IPv4 地址。请刷新或连接可信 Wi‑Fi。</small></div>
              <div class="mobile-settings-actions"><button class="button primary" @click="saveSettings"><Check v-if="configSaved" :size="16" /><span>{{ configSaved ? '已保存' : '保存连接设置' }}</span></button><small>保存后重启 DSH 服务才会应用监听地址。</small></div>
            </div>
          </section>
          <section class="settings-panel mobile-help-panel">
            <div class="panel-title"><MonitorCog :size="18" /><div><h2>使用提示</h2><p>连接后手机可以操作当前项目。</p></div></div>
            <div class="mobile-help-list"><div><strong>1</strong><span>确认手机和电脑连接同一个可信 Wi-Fi。</span></div><div><strong>2</strong><span>点击“显示二维码”，用手机浏览器扫码。</span></div><div><strong>3</strong><span>扫码完成后，点击“关闭二维码”收起面板。</span></div></div>
          </section>
        </div>
      </template>

      <template v-else>
        <section class="page-intro"><div><p class="eyebrow">应用偏好</p><h2>偏好设置</h2><p>这些设置只属于 DeepDash，不会修改 DSH 官方数据。</p></div></section>
        <div class="settings-layout">
          <section class="settings-panel"><div class="panel-title"><Settings2 :size="18" /><div><h2>服务设置</h2><p>下次启动或重启服务时生效。</p></div></div><label class="field-label" for="port">DSH Web 端口</label><div class="port-input"><span>127.0.0.1:</span><input id="port" v-model.number="portDraft" type="number" min="1" max="65535" step="1" /></div><small class="field-help">默认端口为 3080，可使用 1 到 65535 的 TCP 端口。</small><div class="theme-setting"><div><strong>界面主题</strong><small>只影响 DeepDash 外壳，不改变 DSH Web。</small></div><div class="theme-options" role="group" aria-label="界面主题"><button v-for="item in themeOptions" :key="item[0]" :class="['theme-option', { active: config.theme === item[0] }]" @click="changeTheme(item[0])">{{ item[1] }}</button></div></div><div class="settings-actions"><button class="button primary" @click="saveSettings"><Check v-if="configSaved" :size="16" /><span>{{ configSaved ? '已保存' : '保存设置' }}</span></button><button class="button secondary" @click="showDataDirectory"><FolderOpen :size="16" /><span>数据目录</span></button></div></section>
          <section class="settings-panel"><div class="panel-title"><Terminal :size="18" /><div><h2>运行环境</h2><p>启动器使用系统 PATH 和 npm 默认 prefix。</p></div></div><div class="detail-list"><div><span>Node.js</span><code>{{ environment?.node.path ?? '未检测到' }}</code><small>{{ environment?.node.found ? (environment.node.version ?? '不可用：版本检测失败') : '未安装' }}</small></div><div><span>npm</span><code>{{ environment?.npm.path ?? '未检测到' }}</code><small>{{ environment?.npm.found ? (environment.npm.version ?? '不可用：版本检测失败') : '未安装' }}</small></div><div><span>全局 prefix</span><code>{{ environment?.prefix ?? '未检测到' }}</code><small>{{ environment?.prefix ? '由本机 npm 决定' : '不可用：未读取到 prefix' }}</small></div><div><span>dsh</span><code>{{ environment?.dsh.path ?? '未检测到' }}</code><small>{{ environment?.dsh.found ? (environment.dsh.version ?? '不可用：版本检测失败') : '未安装' }}</small></div></div><button class="text-button external-link" @click="go('versions')">前往版本管理 <ChevronRight :size="15" /></button></section>
          <section class="settings-panel links-panel"><div class="panel-title"><ExternalLink :size="18" /><div><h2>相关链接</h2><p>仅在需要时打开外部官方页面。</p></div></div><a href="https://nodejs.org/" target="_blank" rel="noreferrer" class="link-row"><span>Node.js 官方下载</span><ExternalLink :size="15" /></a><a href="https://github.com/deepseek-ai/deepseek-harness" target="_blank" rel="noreferrer" class="link-row"><span>DSH 官方仓库</span><ExternalLink :size="15" /></a><div class="about-row"><span>应用版本</span><strong>1.0.4</strong></div></section>
        </div>
      </template>

      <div v-if="progress && activeView !== 'versions'" class="bottom-progress"><LoaderCircle :size="16" class="spinning" /><span>{{ progress.message }}</span><span v-if="progress.percent !== undefined">{{ progress.percent }}%</span></div>
    </main>
  </div>
</template>
