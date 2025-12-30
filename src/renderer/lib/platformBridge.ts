type Settings = {
  repository: { branchTemplate: string; pushOnCreate: boolean };
  projectPrep?: { autoInstallOnOpenInEditor: boolean };
  browserPreview?: { enabled: boolean; engine: 'chromium' };
  notifications?: { enabled: boolean; sound: boolean };
  mcp?: {
    context7?: {
      enabled: boolean;
      installHintsDismissed?: Record<string, boolean>;
    };
  };
  defaultProvider?: string;
  tasks?: {
    autoGenerateName: boolean;
    autoApproveByDefault: boolean;
  };
  projects?: {
    defaultDirectory: string;
  };
};

type TelemetryStatus = {
  enabled: boolean;
  envDisabled?: boolean;
  userOptOut?: boolean;
  hasKeyAndHost?: boolean;
  onboardingSeen?: boolean;
};

const DEFAULT_SETTINGS: Settings = {
  repository: {
    branchTemplate: 'agent/{slug}-{timestamp}',
    pushOnCreate: true,
  },
  projectPrep: {
    autoInstallOnOpenInEditor: true,
  },
  browserPreview: {
    enabled: true,
    engine: 'chromium',
  },
  notifications: {
    enabled: true,
    sound: true,
  },
  mcp: {
    context7: {
      enabled: false,
      installHintsDismissed: {},
    },
  },
  defaultProvider: 'claude',
  tasks: {
    autoGenerateName: true,
    autoApproveByDefault: false,
  },
  projects: {
    defaultDirectory: '~/emdash-projects',
  },
};

const DEFAULT_TELEMETRY: TelemetryStatus = {
  enabled: true,
  envDisabled: false,
  userOptOut: false,
  hasKeyAndHost: false,
  onboardingSeen: false,
};

const warned = new Set<string>();

const noopCleanup = () => {};

const shouldInitTauri = typeof window !== 'undefined' && !(window as any).electronAPI;
const runtime =
  typeof window !== 'undefined' && (window as any).__TAURI__ ? 'tauri' : 'web';

function warnOnce(name: string) {
  if (warned.has(name)) return;
  warned.add(name);
  try {
    console.warn(`[tauri] ${name} not implemented yet`);
  } catch {}
}

function mergeSettings(base: Settings, patch?: Partial<Settings>): Settings {
  if (!patch) return base;
  const next: any = { ...base };
  for (const [key, value] of Object.entries(patch)) {
    const prior = (base as any)[key];
    if (value && typeof value === 'object' && !Array.isArray(value)) {
      next[key] = mergeSettings(prior ?? {}, value as any);
    } else if (value !== undefined) {
      next[key] = value;
    }
  }
  return next as Settings;
}

export function installPlatformBridge() {
  if (typeof window === 'undefined') return;
  if ((window as any).electronAPI) return;

  let currentSettings: Settings = { ...DEFAULT_SETTINGS };
  let telemetry: TelemetryStatus = { ...DEFAULT_TELEMETRY };

  const base: Record<string, any> = {
    __runtime: runtime,
    getAppVersion: async () => 'tauri-dev',
    getElectronVersion: async () => 'tauri',
    getPlatform: async () => 'darwin',
    openExternal: async (url: string) => {
      try {
        if (typeof url === 'string' && url.length > 0) {
          window.open(url, '_blank', 'noopener');
        }
      } catch {}
      return { success: true };
    },
    openIn: async () => ({ success: false, error: 'not implemented' }),
    openProject: async () => ({ success: false, error: 'not implemented' }),
    getSettings: async () => ({ success: true, settings: currentSettings }),
    updateSettings: async (patch: Partial<Settings>) => {
      currentSettings = mergeSettings(currentSettings, patch);
      return { success: true, settings: currentSettings };
    },
    getTelemetryStatus: async () => ({ success: true, status: telemetry }),
    setTelemetryEnabled: async (enabled: boolean) => {
      telemetry = { ...telemetry, enabled: !!enabled, userOptOut: !enabled };
      return { success: true, status: telemetry };
    },
    setOnboardingSeen: async (flag: boolean) => {
      telemetry = { ...telemetry, onboardingSeen: !!flag };
      return { success: true, status: telemetry };
    },
    captureTelemetry: async () => ({ success: true }),
    checkForUpdates: async () => ({ success: false, error: 'not implemented' }),
    downloadUpdate: async () => ({ success: false, error: 'not implemented' }),
    quitAndInstallUpdate: async () => ({ success: false, error: 'not implemented' }),
    openLatestDownload: async () => ({ success: false, error: 'not implemented' }),
    onUpdateEvent: () => noopCleanup,
    ptyStart: async () => ({ ok: false, error: 'not implemented' }),
    ptyInput: () => {},
    ptyResize: () => {},
    ptyKill: () => {},
    onPtyData: () => noopCleanup,
    ptyGetSnapshot: async () => ({ ok: false, error: 'not implemented' }),
    ptySaveSnapshot: async () => ({ ok: false, error: 'not implemented' }),
    ptyClearSnapshot: async () => ({ ok: false }),
    onPtyExit: () => noopCleanup,
    onPtyStarted: () => noopCleanup,
    terminalGetTheme: async () => ({ ok: false, error: 'not implemented' }),
    getProjects: async () => [],
    getTasks: async () => [],
    saveProject: async () => ({ success: false, error: 'not implemented' }),
    saveTask: async () => ({ success: false, error: 'not implemented' }),
    deleteProject: async () => ({ success: false, error: 'not implemented' }),
    deleteTask: async () => ({ success: false, error: 'not implemented' }),
    saveConversation: async () => ({ success: false, error: 'not implemented' }),
    getConversations: async () => ({ success: false, error: 'not implemented' }),
    getOrCreateDefaultConversation: async () => ({ success: false, error: 'not implemented' }),
    saveMessage: async () => ({ success: false, error: 'not implemented' }),
    getMessages: async () => ({ success: false, error: 'not implemented' }),
    deleteConversation: async () => ({ success: false, error: 'not implemented' }),
    getProjectSettings: async () => ({ success: false, error: 'not implemented' }),
    updateProjectSettings: async () => ({ success: false, error: 'not implemented' }),
    fsList: async () => ({ success: false, error: 'not implemented' }),
    fsRead: async () => ({ success: false, error: 'not implemented' }),
    fsWriteFile: async () => ({ success: false, error: 'not implemented' }),
    fsRemove: async () => ({ success: false, error: 'not implemented' }),
    saveAttachment: async () => ({ success: false, error: 'not implemented' }),
    githubCheckCLIInstalled: async () => false,
    githubInstallCLI: async () => ({ success: false, error: 'not implemented' }),
    githubAuth: async () => ({ success: false, error: 'not implemented' }),
    githubCancelAuth: async () => ({ success: false, error: 'not implemented' }),
    githubGetStatus: async () => ({ installed: false, authenticated: false, user: null }),
    githubIsAuthenticated: async () => false,
    githubGetUser: async () => null,
    githubGetRepositories: async () => [],
    githubCloneRepository: async () => ({ success: false, error: 'not implemented' }),
    githubListPullRequests: async () => ({ success: false, error: 'not implemented' }),
    githubLogout: async () => ({ success: false, error: 'not implemented' }),
    githubGetOwners: async () => ({ success: false, owners: [] }),
    githubValidateRepoName: async () => ({
      success: false,
      valid: false,
      exists: false,
      error: 'not implemented',
    }),
    githubCreateNewProject: async () => ({ success: false, error: 'not implemented' }),
    githubIssuesList: async () => ({ success: false, error: 'not implemented' }),
    githubIssuesSearch: async () => ({ success: false, error: 'not implemented' }),
    githubIssueGet: async () => ({ success: false, error: 'not implemented' }),
    connectToGitHub: async () => ({ success: false, error: 'not implemented' }),
    onGithubAuthDeviceCode: () => noopCleanup,
    onGithubAuthPolling: () => noopCleanup,
    onGithubAuthSlowDown: () => noopCleanup,
    onGithubAuthSuccess: () => noopCleanup,
    onGithubAuthError: () => noopCleanup,
    onGithubAuthCancelled: () => noopCleanup,
    onGithubAuthUserUpdated: () => noopCleanup,
    hostPreviewStart: async () => ({ ok: false, error: 'not implemented' }),
    hostPreviewSetup: async () => ({ ok: false, error: 'not implemented' }),
    hostPreviewStop: async () => ({ ok: true }),
    hostPreviewStopAll: async () => ({ success: true }),
    onHostPreviewEvent: () => noopCleanup,
    browserShow: async () => ({ ok: false, error: 'not implemented' }),
    browserHide: async () => ({ ok: true }),
    browserSetBounds: async () => ({ ok: true }),
    browserLoadURL: async () => ({ ok: false, error: 'not implemented' }),
    browserGoBack: async () => ({ ok: false, error: 'not implemented' }),
    browserGoForward: async () => ({ ok: false, error: 'not implemented' }),
    browserReload: async () => ({ ok: false, error: 'not implemented' }),
    browserOpenDevTools: async () => ({ ok: false, error: 'not implemented' }),
    browserClear: async () => ({ ok: true }),
    getProviderStatuses: async () => ({ success: false, error: 'not implemented' }),
    onProviderStatusUpdated: () => noopCleanup,
    getGitInfo: async (projectPath: string) => ({
      isGitRepo: false,
      path: projectPath,
    }),
    getGitStatus: async () => ({ success: false, error: 'not implemented' }),
    getFileDiff: async () => ({ success: false, error: 'not implemented' }),
    stageFile: async () => ({ success: false, error: 'not implemented' }),
    revertFile: async () => ({ success: false, error: 'not implemented' }),
    gitCommitAndPush: async () => ({ success: false, error: 'not implemented' }),
    generatePrContent: async () => ({ success: false, error: 'not implemented' }),
    createPullRequest: async () => ({ success: false, error: 'not implemented' }),
    getPrStatus: async () => ({ success: false, error: 'not implemented' }),
    getBranchStatus: async () => ({ success: false, error: 'not implemented' }),
    listRemoteBranches: async () => ({ success: false, error: 'not implemented' }),
  };

  const api = new Proxy(base, {
    get(target, prop) {
      if (prop in target) return (target as any)[prop];
      if (typeof prop !== 'string') return undefined;
      if (prop.startsWith('on')) {
        return () => {
          warnOnce(prop);
          return noopCleanup;
        };
      }
      return (..._args: any[]) => {
        warnOnce(prop);
        return undefined;
      };
    },
  });

  (window as any).electronAPI = api;

  if (shouldInitTauri) {
    void Promise.all([
      import('@tauri-apps/api/core'),
      import('@tauri-apps/api/event'),
    ])
      .then(([{ invoke }, { listen }]) => {
        (window as any).electronAPI.__runtime = 'tauri';
        (window as any).electronAPI.getAppVersion = () => invoke<string>('app_get_version');
        (window as any).electronAPI.getPlatform = () => invoke<string>('app_get_platform');
        (window as any).electronAPI.openExternal = (url: string) =>
          invoke('app_open_external', { url });
        (window as any).electronAPI.openIn = (args: { app: string; path: string }) =>
          invoke('app_open_in', args);
        (window as any).electronAPI.openProject = () => invoke('project_open');
        (window as any).electronAPI.ptyStart = (opts: {
          id: string;
          cwd?: string;
          shell?: string;
          command?: string;
          env?: Record<string, string>;
          cols?: number;
          rows?: number;
          autoApprove?: boolean;
          initialPrompt?: string;
          skipResume?: boolean;
        }) => invoke('pty_start', opts);
        (window as any).electronAPI.ptyInput = (args: { id: string; data: string }) => {
          invoke('pty_input', args).catch(() => {});
        };
        (window as any).electronAPI.ptyResize = (args: { id: string; cols: number; rows: number }) => {
          invoke('pty_resize', args).catch(() => {});
        };
        (window as any).electronAPI.ptyKill = (id: string) => {
          invoke('pty_kill', { id }).catch(() => {});
        };
        (window as any).electronAPI.onPtyData = (
          id: string,
          listener: (data: string) => void
        ) => {
          const eventName = `pty:data:${id}`;
          const promise = listen(eventName, (event) => {
            listener(event.payload as string);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.ptyGetSnapshot = (args: { id: string }) =>
          invoke('pty_snapshot_get', args);
        (window as any).electronAPI.ptySaveSnapshot = (args: {
          id: string;
          payload: any;
        }) => invoke('pty_snapshot_save', args);
        (window as any).electronAPI.ptyClearSnapshot = (args: { id: string }) =>
          invoke('pty_snapshot_clear', args);
        (window as any).electronAPI.onPtyExit = (
          id: string,
          listener: (info: { exitCode: number; signal?: number }) => void
        ) => {
          const eventName = `pty:exit:${id}`;
          const promise = listen(eventName, (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onPtyStarted = (
          listener: (data: { id: string }) => void
        ) => {
          const promise = listen('pty:started', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.getSettings = () => invoke('settings_get');
        (window as any).electronAPI.updateSettings = (settings: Partial<Settings>) =>
          invoke('settings_update', { settings });
        (window as any).electronAPI.getProviderStatuses = (opts?: {
          refresh?: boolean;
          providers?: string[];
          providerId?: string;
        }) => invoke('providers_get_statuses', { opts });
        (window as any).electronAPI.onProviderStatusUpdated = (
          listener: (data: { providerId: string; status: any }) => void
        ) => {
          const promise = listen('provider:status-updated', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.getTelemetryStatus = () => invoke('telemetry_get_status');
        (window as any).electronAPI.setTelemetryEnabled = (enabled: boolean) =>
          invoke('telemetry_set_enabled', { enabled });
        (window as any).electronAPI.setOnboardingSeen = (flag: boolean) =>
          invoke('telemetry_set_onboarding_seen', { flag });
        (window as any).electronAPI.captureTelemetry = (
          event: string,
          properties?: Record<string, any>
        ) => invoke('telemetry_capture', { event, properties });
        (window as any).electronAPI.terminalGetTheme = () => invoke('terminal_get_theme');
        (window as any).electronAPI.githubCheckCLIInstalled = () =>
          invoke('github_check_cli_installed');
        (window as any).electronAPI.githubInstallCLI = () => invoke('github_install_cli');
        (window as any).electronAPI.githubAuth = () => invoke('github_auth');
        (window as any).electronAPI.githubCancelAuth = () => invoke('github_cancel_auth');
        (window as any).electronAPI.githubGetStatus = () => invoke('github_get_status');
        (window as any).electronAPI.githubIsAuthenticated = () => invoke('github_is_authenticated');
        (window as any).electronAPI.githubGetUser = () => invoke('github_get_user');
        (window as any).electronAPI.githubGetRepositories = () => invoke('github_get_repositories');
        (window as any).electronAPI.connectToGitHub = (projectPath: string) =>
          invoke('github_connect', { project_path: projectPath });
        (window as any).electronAPI.githubCloneRepository = (
          repoUrl: string,
          localPath: string
        ) => invoke('github_clone_repository', { repo_url: repoUrl, local_path: localPath });
        (window as any).electronAPI.githubListPullRequests = (projectPath: string) =>
          invoke('github_list_pull_requests', { project_path: projectPath });
        (window as any).electronAPI.githubLogout = () => invoke('github_logout');
        (window as any).electronAPI.githubGetOwners = () => invoke('github_get_owners');
        (window as any).electronAPI.githubValidateRepoName = (name: string, owner: string) =>
          invoke('github_validate_repo_name', { name, owner });
        (window as any).electronAPI.githubCreateNewProject = (params: {
          name: string;
          description?: string;
          owner: string;
          isPrivate: boolean;
          gitignoreTemplate?: string;
        }) =>
          invoke('github_create_new_project', {
            name: params.name,
            description: params.description,
            owner: params.owner,
            is_private: params.isPrivate,
          });
        (window as any).electronAPI.githubIssuesList = (
          projectPath: string,
          limit?: number
        ) => invoke('github_issues_list', { project_path: projectPath, limit });
        (window as any).electronAPI.githubIssuesSearch = (
          projectPath: string,
          searchTerm: string,
          limit?: number
        ) =>
          invoke('github_issues_search', {
            project_path: projectPath,
            search_term: searchTerm,
            limit,
          });
        (window as any).electronAPI.githubIssueGet = (projectPath: string, number: number) =>
          invoke('github_issue_get', { project_path: projectPath, number });
        (window as any).electronAPI.getGitInfo = (projectPath: string) =>
          invoke('git_get_info', { project_path: projectPath });
        (window as any).electronAPI.getGitStatus = (taskPath: string) =>
          invoke('git_get_status', { task_path: taskPath });
        (window as any).electronAPI.getFileDiff = (args: { taskPath: string; filePath: string }) =>
          invoke('git_get_file_diff', { task_path: args.taskPath, file_path: args.filePath });
        (window as any).electronAPI.stageFile = (args: { taskPath: string; filePath: string }) =>
          invoke('git_stage_file', { task_path: args.taskPath, file_path: args.filePath });
        (window as any).electronAPI.revertFile = (args: { taskPath: string; filePath: string }) =>
          invoke('git_revert_file', { task_path: args.taskPath, file_path: args.filePath });
        (window as any).electronAPI.gitCommitAndPush = (args: {
          taskPath: string;
          commitMessage?: string;
          createBranchIfOnDefault?: boolean;
          branchPrefix?: string;
        }) =>
          invoke('git_commit_and_push', {
            task_path: args.taskPath,
            commit_message: args.commitMessage,
            create_branch_if_on_default: args.createBranchIfOnDefault,
            branch_prefix: args.branchPrefix,
          });
        (window as any).electronAPI.generatePrContent = (args: {
          taskPath: string;
          base?: string;
        }) =>
          invoke('git_generate_pr_content', {
            task_path: args.taskPath,
            base: args.base,
          });
        (window as any).electronAPI.createPullRequest = (args: {
          taskPath: string;
          title?: string;
          body?: string;
          base?: string;
          head?: string;
          draft?: boolean;
          web?: boolean;
          fill?: boolean;
        }) =>
          invoke('git_create_pr', {
            task_path: args.taskPath,
            title: args.title,
            body: args.body,
            base: args.base,
            head: args.head,
            draft: args.draft,
            web: args.web,
            fill: args.fill,
          });
        (window as any).electronAPI.getPrStatus = (args: { taskPath: string }) =>
          invoke('git_get_pr_status', { task_path: args.taskPath });
        (window as any).electronAPI.getBranchStatus = (args: { taskPath: string }) =>
          invoke('git_get_branch_status', { task_path: args.taskPath });
        (window as any).electronAPI.listRemoteBranches = (args: {
          projectPath: string;
          remote?: string;
        }) =>
          invoke('git_list_remote_branches', {
            project_path: args.projectPath,
            remote: args.remote,
          });
        (window as any).electronAPI.hostPreviewSetup = (args: { taskId: string; taskPath: string }) =>
          invoke('host_preview_setup', {
            task_id: args.taskId,
            task_path: args.taskPath,
          });
        (window as any).electronAPI.hostPreviewStart = (args: {
          taskId: string;
          taskPath: string;
          script?: string;
          parentProjectPath?: string;
        }) =>
          invoke('host_preview_start', {
            task_id: args.taskId,
            task_path: args.taskPath,
            script: args.script,
          });
        (window as any).electronAPI.hostPreviewStop = (taskId: string) =>
          invoke('host_preview_stop', { task_id: taskId });
        (window as any).electronAPI.hostPreviewStopAll = (exceptId?: string) =>
          invoke('host_preview_stop_all', { except_id: exceptId });
        (window as any).electronAPI.onHostPreviewEvent = (listener: (data: any) => void) => {
          const promise = listen('preview:host:event', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.getProjects = () => invoke('db_get_projects');
        (window as any).electronAPI.saveProject = (project: any) =>
          invoke('db_save_project', { project });
        (window as any).electronAPI.getTasks = (projectId?: string) =>
          invoke('db_get_tasks', { project_id: projectId });
        (window as any).electronAPI.saveTask = (task: any) => invoke('db_save_task', { task });
        (window as any).electronAPI.deleteProject = (projectId: string) =>
          invoke('db_delete_project', { project_id: projectId });
        (window as any).electronAPI.deleteTask = (taskId: string) =>
          invoke('db_delete_task', { task_id: taskId });
        (window as any).electronAPI.saveConversation = (conversation: any) =>
          invoke('db_save_conversation', { conversation });
        (window as any).electronAPI.getConversations = (taskId: string) =>
          invoke('db_get_conversations', { task_id: taskId });
        (window as any).electronAPI.getOrCreateDefaultConversation = (taskId: string) =>
          invoke('db_get_or_create_default_conversation', { task_id: taskId });
        (window as any).electronAPI.saveMessage = (message: any) =>
          invoke('db_save_message', { message });
        (window as any).electronAPI.getMessages = (conversationId: string) =>
          invoke('db_get_messages', { conversation_id: conversationId });
        (window as any).electronAPI.deleteConversation = (conversationId: string) =>
          invoke('db_delete_conversation', { conversation_id: conversationId });
        (window as any).electronAPI.getProjectSettings = (projectId: string) =>
          invoke('project_settings_get', { project_id: projectId });
        (window as any).electronAPI.updateProjectSettings = (args: {
          projectId: string;
          baseRef: string;
        }) =>
          invoke('project_settings_update', {
            project_id: args.projectId,
            base_ref: args.baseRef,
          });
        (window as any).electronAPI.fsList = (
          root: string,
          opts?: { includeDirs?: boolean; maxEntries?: number }
        ) =>
          invoke('fs_list', {
            root,
            include_dirs: opts?.includeDirs,
            max_entries: opts?.maxEntries,
          });
        (window as any).electronAPI.fsRead = (root: string, relPath: string, maxBytes?: number) =>
          invoke('fs_read', { root, rel_path: relPath, max_bytes: maxBytes });
        (window as any).electronAPI.fsWriteFile = (
          root: string,
          relPath: string,
          content: string,
          mkdirs?: boolean
        ) => invoke('fs_write', { root, rel_path: relPath, content, mkdirs });
        (window as any).electronAPI.fsRemove = (root: string, relPath: string) =>
          invoke('fs_remove', { root, rel_path: relPath });
        (window as any).electronAPI.saveAttachment = (args: {
          taskPath: string;
          srcPath: string;
          subdir?: string;
        }) =>
          invoke('fs_save_attachment', {
            task_path: args.taskPath,
            src_path: args.srcPath,
            subdir: args.subdir,
          });
        (window as any).electronAPI.onGithubAuthDeviceCode = (
          listener: (data: {
            userCode: string;
            verificationUri: string;
            expiresIn: number;
            interval: number;
          }) => void
        ) => {
          const promise = listen('github:auth:device-code', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onGithubAuthPolling = (
          listener: (data: { status: string }) => void
        ) => {
          const promise = listen('github:auth:polling', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onGithubAuthSlowDown = (
          listener: (data: { newInterval: number }) => void
        ) => {
          const promise = listen('github:auth:slow-down', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onGithubAuthSuccess = (
          listener: (data: { token: string; user: any }) => void
        ) => {
          const promise = listen('github:auth:success', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onGithubAuthError = (
          listener: (data: { error: string; message: string }) => void
        ) => {
          const promise = listen('github:auth:error', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onGithubAuthCancelled = (listener: () => void) => {
          const promise = listen('github:auth:cancelled', () => {
            listener();
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onGithubAuthUserUpdated = (
          listener: (data: { user: any }) => void
        ) => {
          const promise = listen('github:auth:user-updated', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).electronAPI.onPlanEvent = (
          listener: (data: {
            type: 'write_blocked' | 'remove_blocked';
            root: string;
            relPath: string;
            code?: string;
            message?: string;
          }) => void
        ) => {
          const promise = listen('plan:event', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
      })
      .catch(() => {});
  }
}
