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

const warned = new Set<string>();

const noopCleanup = () => {};

const isTauriRuntime = () => {
  if (typeof window === 'undefined') return false;
  const win = window as any;
  return !!(win.__TAURI_INTERNALS__ || win.__TAURI__);
};

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
  if ((window as any).desktopAPI) return;

  const shouldInitTauri = isTauriRuntime();
  const runtime = shouldInitTauri ? 'tauri' : 'web';

  let currentSettings: Settings = { ...DEFAULT_SETTINGS };

  const base: Record<string, any> = {
    __runtime: runtime,
    getAppVersion: async () => 'tauri-dev',
    getRuntimeVersion: async () => 'tauri',
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
    getDbInitError: async () => ({ success: true }),
    dbRetryInit: async () => ({ success: false, error: 'not implemented' }),
    dbBackupAndReset: async () => ({ success: false, error: 'not implemented' }),
    onDbInitError: () => noopCleanup,
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
    fetchProjectBaseRef: async () => ({ success: false, error: 'not implemented' }),
    worktreeCreate: async () => ({ success: false, error: 'not implemented' }),
    worktreeList: async () => ({ success: false, error: 'not implemented' }),
    worktreeRemove: async () => ({ success: false, error: 'not implemented' }),
    worktreeStatus: async () => ({ success: false, error: 'not implemented' }),
    worktreeMerge: async () => ({ success: false, error: 'not implemented' }),
    worktreeGet: async () => ({ success: false, error: 'not implemented' }),
    worktreeGetAll: async () => ({ success: false, error: 'not implemented' }),
    fsList: async () => ({ success: false, error: 'not implemented' }),
    fsRead: async () => ({ success: false, error: 'not implemented' }),
    fsWriteFile: async () => ({ success: false, error: 'not implemented' }),
    fsRemove: async () => ({ success: false, error: 'not implemented' }),
    saveAttachment: async () => ({ success: false, error: 'not implemented' }),
    loadContainerConfig: async () => ({ ok: false, error: 'not implemented' }),
    startContainerRun: async () => ({ ok: false, error: 'not implemented' }),
    stopContainerRun: async () => ({ ok: false, error: 'not implemented' }),
    inspectContainerRun: async () => ({ ok: false, error: 'not implemented' }),
    resolveServiceIcon: async () => ({ ok: false, error: 'not implemented' }),
    onRunEvent: () => noopCleanup,
    removeRunEventListeners: () => {},
    netProbePorts: async () => ({ reachable: [] }),
    planLock: async () => ({ success: false, error: 'not implemented' }),
    planUnlock: async () => ({ success: false, error: 'not implemented' }),
    planApplyLock: async () => ({ success: false, error: 'not implemented' }),
    planReleaseLock: async () => ({ success: false, error: 'not implemented' }),
    debugAppendLog: async () => ({ success: false, error: 'not implemented' }),
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
    githubCreatePullRequestWorktree: async () => ({ success: false, error: 'not implemented' }),
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
    linearCheckConnection: async () => ({ connected: false }),
    linearSaveToken: async () => ({ success: false, error: 'not implemented' }),
    linearClearToken: async () => ({ success: false, error: 'not implemented' }),
    linearInitialFetch: async () => ({ success: false, error: 'not implemented' }),
    linearSearchIssues: async () => ({ success: false, error: 'not implemented' }),
    jiraSaveCredentials: async () => ({ success: false, error: 'not implemented' }),
    jiraClearCredentials: async () => ({ success: false, error: 'not implemented' }),
    jiraCheckConnection: async () => ({ connected: false }),
    jiraInitialFetch: async () => ({ success: false, error: 'not implemented' }),
    jiraSearchIssues: async () => ({ success: false, error: 'not implemented' }),
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

  (window as any).desktopAPI = api;

  const runEventUnsubscribers: Array<() => void> = [];

  if (shouldInitTauri) {
    void Promise.all([
      import('@tauri-apps/api/core'),
      import('@tauri-apps/api/event'),
    ])
      .then(([{ invoke }, { listen }]) => {
        (window as any).desktopAPI.__runtime = 'tauri';
        (window as any).desktopAPI.__runtimeReady = true;
        (window as any).desktopAPI.getAppVersion = () => invoke<string>('app_get_version');
        (window as any).desktopAPI.getRuntimeVersion = () =>
          invoke<string>('app_get_runtime_version');
        (window as any).desktopAPI.getPlatform = () => invoke<string>('app_get_platform');
        (window as any).desktopAPI.openExternal = (url: string) =>
          invoke('app_open_external', { url });
        (window as any).desktopAPI.openIn = (args: { app: string; path: string }) =>
          invoke('app_open_in', args);
        (window as any).desktopAPI.openProject = () => invoke('project_open');
        (window as any).desktopAPI.ptyStart = (opts: {
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
        }) => invoke('pty_start', { args: opts });
        (window as any).desktopAPI.ptyInput = (args: { id: string; data: string }) => {
          invoke('pty_input', args).catch(() => {});
        };
        (window as any).desktopAPI.ptyResize = (args: { id: string; cols: number; rows: number }) => {
          invoke('pty_resize', args).catch(() => {});
        };
        (window as any).desktopAPI.ptyKill = (id: string) => {
          invoke('pty_kill', { id }).catch(() => {});
        };
        (window as any).desktopAPI.onPtyData = (
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
        (window as any).desktopAPI.ptyGetSnapshot = (args: { id: string }) =>
          invoke('pty_snapshot_get', args);
        (window as any).desktopAPI.ptySaveSnapshot = (args: {
          id: string;
          payload: any;
        }) => invoke('pty_snapshot_save', args);
        (window as any).desktopAPI.ptyClearSnapshot = (args: { id: string }) =>
          invoke('pty_snapshot_clear', args);
        (window as any).desktopAPI.onPtyExit = (
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
        (window as any).desktopAPI.onPtyStarted = (
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
        (window as any).desktopAPI.getSettings = () => invoke('settings_get');
        (window as any).desktopAPI.updateSettings = (settings: Partial<Settings>) =>
          invoke('settings_update', { settings });
        (window as any).desktopAPI.getDbInitError = () => invoke('db_get_init_error');
        (window as any).desktopAPI.dbRetryInit = () => invoke('db_retry_init');
        (window as any).desktopAPI.dbBackupAndReset = () => invoke('db_backup_and_reset');
        (window as any).desktopAPI.onDbInitError = (listener: (data: any) => void) => {
          const promise = listen('db:init-error', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).desktopAPI.getProviderStatuses = (opts?: {
          refresh?: boolean;
          providers?: string[];
          providerId?: string;
        }) => invoke('providers_get_statuses', { opts });
        (window as any).desktopAPI.onProviderStatusUpdated = (
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
        (window as any).desktopAPI.terminalGetTheme = () => invoke('terminal_get_theme');
        (window as any).desktopAPI.githubCheckCLIInstalled = () =>
          invoke('github_check_cli_installed');
        (window as any).desktopAPI.githubInstallCLI = () => invoke('github_install_cli');
        (window as any).desktopAPI.githubAuth = () => invoke('github_auth');
        (window as any).desktopAPI.githubCancelAuth = () => invoke('github_cancel_auth');
        (window as any).desktopAPI.githubGetStatus = () => invoke('github_get_status');
        (window as any).desktopAPI.githubIsAuthenticated = () => invoke('github_is_authenticated');
        (window as any).desktopAPI.githubGetUser = () => invoke('github_get_user');
        (window as any).desktopAPI.githubGetRepositories = () => invoke('github_get_repositories');
        (window as any).desktopAPI.connectToGitHub = (projectPath: string) =>
          invoke('github_connect', { projectPath });
        (window as any).desktopAPI.githubCloneRepository = (
          repoUrl: string,
          localPath: string
        ) => invoke('github_clone_repository', { repoUrl, localPath });
        (window as any).desktopAPI.githubListPullRequests = (projectPath: string) =>
          invoke('github_list_pull_requests', { projectPath });
        (window as any).desktopAPI.githubLogout = () => invoke('github_logout');
        (window as any).desktopAPI.githubGetOwners = () => invoke('github_get_owners');
        (window as any).desktopAPI.githubValidateRepoName = (name: string, owner: string) =>
          invoke('github_validate_repo_name', { name, owner });
        (window as any).desktopAPI.githubCreateNewProject = (params: {
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
            isPrivate: params.isPrivate,
          });
        (window as any).desktopAPI.githubIssuesList = (
          projectPath: string,
          limit?: number
        ) => invoke('github_issues_list', { projectPath, limit });
        (window as any).desktopAPI.githubIssuesSearch = (
          projectPath: string,
          searchTerm: string,
          limit?: number
        ) =>
          invoke('github_issues_search', {
            projectPath,
            searchTerm,
            limit,
          });
        (window as any).desktopAPI.githubIssueGet = (projectPath: string, number: number) =>
          invoke('github_issue_get', { projectPath, number });
        (window as any).desktopAPI.getGitInfo = (projectPath: string) =>
          invoke('git_get_info', { projectPath });
        (window as any).desktopAPI.getGitStatus = (taskPath: string) =>
          invoke('git_get_status', { taskPath });
        (window as any).desktopAPI.getFileDiff = (args: { taskPath: string; filePath: string }) =>
          invoke('git_get_file_diff', { taskPath: args.taskPath, filePath: args.filePath });
        (window as any).desktopAPI.stageFile = (args: { taskPath: string; filePath: string }) =>
          invoke('git_stage_file', { taskPath: args.taskPath, filePath: args.filePath });
        (window as any).desktopAPI.revertFile = (args: { taskPath: string; filePath: string }) =>
          invoke('git_revert_file', { taskPath: args.taskPath, filePath: args.filePath });
        (window as any).desktopAPI.gitCommitAndPush = (args: {
          taskPath: string;
          commitMessage?: string;
          createBranchIfOnDefault?: boolean;
          branchPrefix?: string;
        }) =>
          invoke('git_commit_and_push', {
            taskPath: args.taskPath,
            commitMessage: args.commitMessage,
            createBranchIfOnDefault: args.createBranchIfOnDefault,
            branchPrefix: args.branchPrefix,
          });
        (window as any).desktopAPI.generatePrContent = (args: {
          taskPath: string;
          base?: string;
        }) =>
          invoke('git_generate_pr_content', {
            taskPath: args.taskPath,
            base: args.base,
          });
        (window as any).desktopAPI.createPullRequest = (args: {
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
            taskPath: args.taskPath,
            title: args.title,
            body: args.body,
            base: args.base,
            head: args.head,
            draft: args.draft,
            web: args.web,
            fill: args.fill,
          });
        (window as any).desktopAPI.getPrStatus = (args: { taskPath: string }) =>
          invoke('git_get_pr_status', { taskPath: args.taskPath });
        (window as any).desktopAPI.getBranchStatus = (args: { taskPath: string }) =>
          invoke('git_get_branch_status', { taskPath: args.taskPath });
        (window as any).desktopAPI.listRemoteBranches = (args: {
          projectPath: string;
          remote?: string;
        }) =>
          invoke('git_list_remote_branches', {
            projectPath: args.projectPath,
            remote: args.remote,
          });
        (window as any).desktopAPI.hostPreviewSetup = (args: { taskId: string; taskPath: string }) =>
          invoke('host_preview_setup', {
            taskId: args.taskId,
            taskPath: args.taskPath,
          });
        (window as any).desktopAPI.hostPreviewStart = (args: {
          taskId: string;
          taskPath: string;
          script?: string;
          parentProjectPath?: string;
        }) =>
          invoke('host_preview_start', {
            taskId: args.taskId,
            taskPath: args.taskPath,
            script: args.script,
            parentProjectPath: args.parentProjectPath,
          });
        (window as any).desktopAPI.hostPreviewStop = (taskId: string) =>
          invoke('host_preview_stop', { taskId });
        (window as any).desktopAPI.hostPreviewStopAll = (exceptId?: string) =>
          invoke('host_preview_stop_all', { exceptId });
        (window as any).desktopAPI.onHostPreviewEvent = (listener: (data: any) => void) => {
          const promise = listen('preview:host:event', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };

        (window as any).desktopAPI.onPlanEvent = (listener: (data: any) => void) => {
          const promise = listen('plan:event', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).desktopAPI.getProjects = () => invoke('db_get_projects');
        (window as any).desktopAPI.saveProject = (project: any) =>
          invoke('db_save_project', { project });
        (window as any).desktopAPI.getTasks = (projectId?: string) =>
          invoke('db_get_tasks', { projectId });
        (window as any).desktopAPI.saveTask = (task: any) => invoke('db_save_task', { task });
        (window as any).desktopAPI.deleteProject = (projectId: string) =>
          invoke('db_delete_project', { projectId });
        (window as any).desktopAPI.deleteTask = (taskId: string) =>
          invoke('db_delete_task', { taskId });
        (window as any).desktopAPI.saveConversation = (conversation: any) =>
          invoke('db_save_conversation', { conversation });
        (window as any).desktopAPI.getConversations = (taskId: string) =>
          invoke('db_get_conversations', { taskId });
        (window as any).desktopAPI.getOrCreateDefaultConversation = (taskId: string) =>
          invoke('db_get_or_create_default_conversation', { taskId });
        (window as any).desktopAPI.saveMessage = (message: any) =>
          invoke('db_save_message', { message });
        (window as any).desktopAPI.getMessages = (conversationId: string) =>
          invoke('db_get_messages', { conversationId });
        (window as any).desktopAPI.deleteConversation = (conversationId: string) =>
          invoke('db_delete_conversation', { conversationId });
        (window as any).desktopAPI.getProjectSettings = (projectId: string) =>
          invoke('project_settings_get', { projectId });
        (window as any).desktopAPI.updateProjectSettings = (args: {
          projectId: string;
          baseRef: string;
        }) =>
          invoke('project_settings_update', {
            projectId: args.projectId,
            baseRef: args.baseRef,
          });
        (window as any).desktopAPI.fetchProjectBaseRef = (args: {
          projectId: string;
          projectPath: string;
        }) =>
          invoke('project_settings_fetch_base_ref', {
            projectId: args.projectId,
            projectPath: args.projectPath,
          });
        const invokeWithArgs = async <T>(command: string, payload: Record<string, any>) => {
          try {
            return await invoke<T>(command, payload);
          } catch (error: any) {
            const message = error?.message ? String(error.message) : String(error || '');
            if (message.includes('missing required key args')) {
              return await invoke<T>(command, { args: payload });
            }
            throw error;
          }
        };

        (window as any).desktopAPI.worktreeCreate = (args: {
          projectPath: string;
          taskName: string;
          projectId: string;
          autoApprove?: boolean;
        }) =>
          invokeWithArgs('worktree_create', {
            projectPath: args.projectPath,
            taskName: args.taskName,
            projectId: args.projectId,
            autoApprove: args.autoApprove,
          });
        (window as any).desktopAPI.worktreeList = (args: { projectPath: string }) =>
          invokeWithArgs('worktree_list', { projectPath: args.projectPath });
        (window as any).desktopAPI.worktreeRemove = (args: {
          projectPath: string;
          worktreeId: string;
          worktreePath?: string;
          branch?: string;
        }) =>
          invokeWithArgs('worktree_remove', {
            projectPath: args.projectPath,
            worktreeId: args.worktreeId,
            worktreePath: args.worktreePath,
            branch: args.branch,
          });
        (window as any).desktopAPI.worktreeStatus = (args: { worktreePath: string }) =>
          invokeWithArgs('worktree_status', { worktreePath: args.worktreePath });
        (window as any).desktopAPI.worktreeMerge = (args: {
          projectPath: string;
          worktreeId: string;
        }) =>
          invokeWithArgs('worktree_merge', {
            projectPath: args.projectPath,
            worktreeId: args.worktreeId,
          });
        (window as any).desktopAPI.worktreeGet = (args: { worktreeId: string }) =>
          invokeWithArgs('worktree_get', { worktreeId: args.worktreeId });
        (window as any).desktopAPI.worktreeGetAll = () => invoke('worktree_get_all');
        (window as any).desktopAPI.fsList = (
          root: string,
          opts?: { includeDirs?: boolean; maxEntries?: number }
        ) =>
          invoke('fs_list', {
            root,
            includeDirs: opts?.includeDirs,
            maxEntries: opts?.maxEntries,
          });
        (window as any).desktopAPI.fsRead = (root: string, relPath: string, maxBytes?: number) =>
          invoke('fs_read', { root, relPath, maxBytes });
        (window as any).desktopAPI.fsWriteFile = (
          root: string,
          relPath: string,
          content: string,
          mkdirs?: boolean
        ) => invoke('fs_write', { root, relPath, content, mkdirs });
        (window as any).desktopAPI.fsRemove = (root: string, relPath: string) =>
          invoke('fs_remove', { root, relPath });
        (window as any).desktopAPI.saveAttachment = (args: {
          taskPath: string;
          srcPath: string;
          subdir?: string;
        }) =>
          invoke('fs_save_attachment', {
            taskPath: args.taskPath,
            srcPath: args.srcPath,
            subdir: args.subdir,
          });
        (window as any).desktopAPI.loadContainerConfig = (taskPath: string) =>
          invoke('container_load_config', { taskPath });
        (window as any).desktopAPI.startContainerRun = (args: {
          taskId: string;
          taskPath: string;
          runId?: string;
          mode?: 'container' | 'host';
        }) =>
          invoke('container_start_run', {
            taskId: args.taskId,
            taskPath: args.taskPath,
            runId: args.runId,
            mode: args.mode,
          });
        (window as any).desktopAPI.stopContainerRun = (taskId: string) =>
          invoke('container_stop_run', { taskId });
        (window as any).desktopAPI.inspectContainerRun = (taskId: string) =>
          invoke('container_inspect_run', { taskId });
        (window as any).desktopAPI.resolveServiceIcon = (args: {
          service: string;
          allowNetwork?: boolean;
          taskPath?: string;
        }) =>
          invoke('icons_resolve_service', {
            service: args.service,
            allowNetwork: args.allowNetwork,
            taskPath: args.taskPath,
          });
        (window as any).desktopAPI.onRunEvent = (listener: (event: any) => void) => {
          const promise = listen('run:event', (event) => {
            listener(event.payload as any);
          });
          promise
            .then((unlisten) => {
              runEventUnsubscribers.push(unlisten);
            })
            .catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).desktopAPI.removeRunEventListeners = () => {
          const pending = runEventUnsubscribers.splice(0, runEventUnsubscribers.length);
          pending.forEach((fn) => {
            try {
              fn();
            } catch {}
          });
        };
        (window as any).desktopAPI.netProbePorts = (
          host: string,
          ports: number[],
          timeoutMs?: number
        ) =>
          invoke('net_probe_ports', {
            host,
            ports,
            timeoutMs,
          });
        (window as any).desktopAPI.planLock = (taskPath: string) =>
          invoke('plan_lock', { taskPath });
        (window as any).desktopAPI.planUnlock = (taskPath: string) =>
          invoke('plan_unlock', { taskPath });
        (window as any).desktopAPI.planApplyLock = (taskPath: string) =>
          invoke('plan_lock', { taskPath });
        (window as any).desktopAPI.planReleaseLock = (taskPath: string) =>
          invoke('plan_unlock', { taskPath });
        (window as any).desktopAPI.debugAppendLog = (
          filePath: string,
          content: string,
          options?: { reset?: boolean }
        ) => invoke('debug_append_log', { filePath, content, options });
        (window as any).desktopAPI.onGithubAuthDeviceCode = (
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
        (window as any).desktopAPI.onGithubAuthPolling = (
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
        (window as any).desktopAPI.onGithubAuthSlowDown = (
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
        (window as any).desktopAPI.onGithubAuthSuccess = (
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
        (window as any).desktopAPI.onGithubAuthError = (
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
        (window as any).desktopAPI.onGithubAuthCancelled = (listener: () => void) => {
          const promise = listen('github:auth:cancelled', () => {
            listener();
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
        (window as any).desktopAPI.onGithubAuthUserUpdated = (
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
        (window as any).desktopAPI.onPlanEvent = (
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
        (window as any).desktopAPI.browserShow = (
          bounds: { x: number; y: number; width: number; height: number },
          url?: string
        ) =>
          invoke('browser_view_show', {
            bounds,
            url,
          });
        (window as any).desktopAPI.browserHide = () => invoke('browser_view_hide');
        (window as any).desktopAPI.browserSetBounds = (bounds: {
          x: number;
          y: number;
          width: number;
          height: number;
        }) => invoke('browser_view_set_bounds', { bounds });
        (window as any).desktopAPI.browserLoadURL = (url: string, forceReload?: boolean) =>
          invoke('browser_view_load_url', { url, forceReload });
        (window as any).desktopAPI.browserGoBack = () => invoke('browser_view_go_back');
        (window as any).desktopAPI.browserGoForward = () => invoke('browser_view_go_forward');
        (window as any).desktopAPI.browserReload = () => invoke('browser_view_reload');
        (window as any).desktopAPI.browserOpenDevTools = () =>
          invoke('browser_view_open_devtools');
        (window as any).desktopAPI.browserClear = () => invoke('browser_view_clear');
        (window as any).desktopAPI.linearSaveToken = (token: string) =>
          invoke('linear_save_token', { token });
        (window as any).desktopAPI.linearCheckConnection = () =>
          invoke('linear_check_connection');
        (window as any).desktopAPI.linearClearToken = () => invoke('linear_clear_token');
        (window as any).desktopAPI.linearInitialFetch = (limit?: number) =>
          invoke('linear_initial_fetch', { limit });
        (window as any).desktopAPI.linearSearchIssues = (searchTerm: string, limit?: number) =>
          invoke('linear_search_issues', { searchTerm, limit });
        (window as any).desktopAPI.jiraSaveCredentials = (args: {
          siteUrl: string;
          email: string;
          token: string;
        }) =>
          invoke('jira_save_credentials', {
            siteUrl: args.siteUrl,
            email: args.email,
            token: args.token,
          });
        (window as any).desktopAPI.jiraClearCredentials = () => invoke('jira_clear_credentials');
        (window as any).desktopAPI.jiraCheckConnection = () => invoke('jira_check_connection');
        (window as any).desktopAPI.jiraInitialFetch = (limit?: number) =>
          invoke('jira_initial_fetch', { limit });
        (window as any).desktopAPI.jiraSearchIssues = (searchTerm: string, limit?: number) =>
          invoke('jira_search_issues', { searchTerm, limit });
        (window as any).desktopAPI.githubCreatePullRequestWorktree = (args: {
          projectPath: string;
          projectId: string;
          prNumber: number;
          prTitle?: string;
          taskName?: string;
          branchName?: string;
        }) =>
          invoke('github_create_pull_request_worktree', {
            projectPath: args.projectPath,
            projectId: args.projectId,
            prNumber: args.prNumber,
            prTitle: args.prTitle,
            taskName: args.taskName,
            branchName: args.branchName,
          });
      })
      .catch(() => {});
  }
}
