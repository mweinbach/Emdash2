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
  if ((window as any).electronAPI) return;

  const shouldInitTauri = isTauriRuntime();
  const runtime = shouldInitTauri ? 'tauri' : 'web';

  let currentSettings: Settings = { ...DEFAULT_SETTINGS };

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

  (window as any).electronAPI = api;

  const runEventUnsubscribers: Array<() => void> = [];

  if (shouldInitTauri) {
    void Promise.all([
      import('@tauri-apps/api/core'),
      import('@tauri-apps/api/event'),
    ])
      .then(([{ invoke }, { listen }]) => {
        (window as any).electronAPI.__runtime = 'tauri';
        (window as any).electronAPI.__runtimeReady = true;
        (window as any).electronAPI.getAppVersion = () => invoke<string>('app_get_version');
        (window as any).electronAPI.getElectronVersion = () =>
          invoke<string>('app_get_electron_version');
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
        }) => invoke('pty_start', { args: opts });
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
        (window as any).electronAPI.getDbInitError = () => invoke('db_get_init_error');
        (window as any).electronAPI.dbRetryInit = () => invoke('db_retry_init');
        (window as any).electronAPI.dbBackupAndReset = () => invoke('db_backup_and_reset');
        (window as any).electronAPI.onDbInitError = (listener: (data: any) => void) => {
          const promise = listen('db:init-error', (event) => {
            listener(event.payload as any);
          });
          promise.catch(() => {});
          return () => {
            promise.then((unlisten) => unlisten()).catch(() => {});
          };
        };
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
          invoke('github_connect', { projectPath });
        (window as any).electronAPI.githubCloneRepository = (
          repoUrl: string,
          localPath: string
        ) => invoke('github_clone_repository', { repoUrl, localPath });
        (window as any).electronAPI.githubListPullRequests = (projectPath: string) =>
          invoke('github_list_pull_requests', { projectPath });
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
            isPrivate: params.isPrivate,
          });
        (window as any).electronAPI.githubIssuesList = (
          projectPath: string,
          limit?: number
        ) => invoke('github_issues_list', { projectPath, limit });
        (window as any).electronAPI.githubIssuesSearch = (
          projectPath: string,
          searchTerm: string,
          limit?: number
        ) =>
          invoke('github_issues_search', {
            projectPath,
            searchTerm,
            limit,
          });
        (window as any).electronAPI.githubIssueGet = (projectPath: string, number: number) =>
          invoke('github_issue_get', { projectPath, number });
        (window as any).electronAPI.getGitInfo = (projectPath: string) =>
          invoke('git_get_info', { projectPath });
        (window as any).electronAPI.getGitStatus = (taskPath: string) =>
          invoke('git_get_status', { taskPath });
        (window as any).electronAPI.getFileDiff = (args: { taskPath: string; filePath: string }) =>
          invoke('git_get_file_diff', { taskPath: args.taskPath, filePath: args.filePath });
        (window as any).electronAPI.stageFile = (args: { taskPath: string; filePath: string }) =>
          invoke('git_stage_file', { taskPath: args.taskPath, filePath: args.filePath });
        (window as any).electronAPI.revertFile = (args: { taskPath: string; filePath: string }) =>
          invoke('git_revert_file', { taskPath: args.taskPath, filePath: args.filePath });
        (window as any).electronAPI.gitCommitAndPush = (args: {
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
        (window as any).electronAPI.generatePrContent = (args: {
          taskPath: string;
          base?: string;
        }) =>
          invoke('git_generate_pr_content', {
            taskPath: args.taskPath,
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
            taskPath: args.taskPath,
            title: args.title,
            body: args.body,
            base: args.base,
            head: args.head,
            draft: args.draft,
            web: args.web,
            fill: args.fill,
          });
        (window as any).electronAPI.getPrStatus = (args: { taskPath: string }) =>
          invoke('git_get_pr_status', { taskPath: args.taskPath });
        (window as any).electronAPI.getBranchStatus = (args: { taskPath: string }) =>
          invoke('git_get_branch_status', { taskPath: args.taskPath });
        (window as any).electronAPI.listRemoteBranches = (args: {
          projectPath: string;
          remote?: string;
        }) =>
          invoke('git_list_remote_branches', {
            projectPath: args.projectPath,
            remote: args.remote,
          });
        (window as any).electronAPI.hostPreviewSetup = (args: { taskId: string; taskPath: string }) =>
          invoke('host_preview_setup', {
            taskId: args.taskId,
            taskPath: args.taskPath,
          });
        (window as any).electronAPI.hostPreviewStart = (args: {
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
        (window as any).electronAPI.hostPreviewStop = (taskId: string) =>
          invoke('host_preview_stop', { taskId });
        (window as any).electronAPI.hostPreviewStopAll = (exceptId?: string) =>
          invoke('host_preview_stop_all', { exceptId });
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
          invoke('db_get_tasks', { projectId });
        (window as any).electronAPI.saveTask = (task: any) => invoke('db_save_task', { task });
        (window as any).electronAPI.deleteProject = (projectId: string) =>
          invoke('db_delete_project', { projectId });
        (window as any).electronAPI.deleteTask = (taskId: string) =>
          invoke('db_delete_task', { taskId });
        (window as any).electronAPI.saveConversation = (conversation: any) =>
          invoke('db_save_conversation', { conversation });
        (window as any).electronAPI.getConversations = (taskId: string) =>
          invoke('db_get_conversations', { taskId });
        (window as any).electronAPI.getOrCreateDefaultConversation = (taskId: string) =>
          invoke('db_get_or_create_default_conversation', { taskId });
        (window as any).electronAPI.saveMessage = (message: any) =>
          invoke('db_save_message', { message });
        (window as any).electronAPI.getMessages = (conversationId: string) =>
          invoke('db_get_messages', { conversationId });
        (window as any).electronAPI.deleteConversation = (conversationId: string) =>
          invoke('db_delete_conversation', { conversationId });
        (window as any).electronAPI.getProjectSettings = (projectId: string) =>
          invoke('project_settings_get', { projectId });
        (window as any).electronAPI.updateProjectSettings = (args: {
          projectId: string;
          baseRef: string;
        }) =>
          invoke('project_settings_update', {
            projectId: args.projectId,
            baseRef: args.baseRef,
          });
        (window as any).electronAPI.fetchProjectBaseRef = (args: {
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

        (window as any).electronAPI.worktreeCreate = (args: {
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
        (window as any).electronAPI.worktreeList = (args: { projectPath: string }) =>
          invokeWithArgs('worktree_list', { projectPath: args.projectPath });
        (window as any).electronAPI.worktreeRemove = (args: {
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
        (window as any).electronAPI.worktreeStatus = (args: { worktreePath: string }) =>
          invokeWithArgs('worktree_status', { worktreePath: args.worktreePath });
        (window as any).electronAPI.worktreeMerge = (args: {
          projectPath: string;
          worktreeId: string;
        }) =>
          invokeWithArgs('worktree_merge', {
            projectPath: args.projectPath,
            worktreeId: args.worktreeId,
          });
        (window as any).electronAPI.worktreeGet = (args: { worktreeId: string }) =>
          invokeWithArgs('worktree_get', { worktreeId: args.worktreeId });
        (window as any).electronAPI.worktreeGetAll = () => invoke('worktree_get_all');
        (window as any).electronAPI.fsList = (
          root: string,
          opts?: { includeDirs?: boolean; maxEntries?: number }
        ) =>
          invoke('fs_list', {
            root,
            includeDirs: opts?.includeDirs,
            maxEntries: opts?.maxEntries,
          });
        (window as any).electronAPI.fsRead = (root: string, relPath: string, maxBytes?: number) =>
          invoke('fs_read', { root, relPath, maxBytes });
        (window as any).electronAPI.fsWriteFile = (
          root: string,
          relPath: string,
          content: string,
          mkdirs?: boolean
        ) => invoke('fs_write', { root, relPath, content, mkdirs });
        (window as any).electronAPI.fsRemove = (root: string, relPath: string) =>
          invoke('fs_remove', { root, relPath });
        (window as any).electronAPI.saveAttachment = (args: {
          taskPath: string;
          srcPath: string;
          subdir?: string;
        }) =>
          invoke('fs_save_attachment', {
            taskPath: args.taskPath,
            srcPath: args.srcPath,
            subdir: args.subdir,
          });
        (window as any).electronAPI.loadContainerConfig = (taskPath: string) =>
          invoke('container_load_config', { taskPath });
        (window as any).electronAPI.startContainerRun = (args: {
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
        (window as any).electronAPI.stopContainerRun = (taskId: string) =>
          invoke('container_stop_run', { taskId });
        (window as any).electronAPI.inspectContainerRun = (taskId: string) =>
          invoke('container_inspect_run', { taskId });
        (window as any).electronAPI.resolveServiceIcon = (args: {
          service: string;
          allowNetwork?: boolean;
          taskPath?: string;
        }) =>
          invoke('icons_resolve_service', {
            service: args.service,
            allowNetwork: args.allowNetwork,
            taskPath: args.taskPath,
          });
        (window as any).electronAPI.onRunEvent = (listener: (event: any) => void) => {
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
        (window as any).electronAPI.removeRunEventListeners = () => {
          const pending = runEventUnsubscribers.splice(0, runEventUnsubscribers.length);
          pending.forEach((fn) => {
            try {
              fn();
            } catch {}
          });
        };
        (window as any).electronAPI.netProbePorts = (
          host: string,
          ports: number[],
          timeoutMs?: number
        ) =>
          invoke('net_probe_ports', {
            host,
            ports,
            timeoutMs,
          });
        (window as any).electronAPI.planLock = (taskPath: string) =>
          invoke('plan_lock', { taskPath });
        (window as any).electronAPI.planUnlock = (taskPath: string) =>
          invoke('plan_unlock', { taskPath });
        (window as any).electronAPI.planApplyLock = (taskPath: string) =>
          invoke('plan_lock', { taskPath });
        (window as any).electronAPI.planReleaseLock = (taskPath: string) =>
          invoke('plan_unlock', { taskPath });
        (window as any).electronAPI.debugAppendLog = (
          filePath: string,
          content: string,
          options?: { reset?: boolean }
        ) => invoke('debug_append_log', { filePath, content, options });
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
        (window as any).electronAPI.browserShow = (
          bounds: { x: number; y: number; width: number; height: number },
          url?: string
        ) =>
          invoke('browser_view_show', {
            bounds,
            url,
          });
        (window as any).electronAPI.browserHide = () => invoke('browser_view_hide');
        (window as any).electronAPI.browserSetBounds = (bounds: {
          x: number;
          y: number;
          width: number;
          height: number;
        }) => invoke('browser_view_set_bounds', { bounds });
        (window as any).electronAPI.browserLoadURL = (url: string, forceReload?: boolean) =>
          invoke('browser_view_load_url', { url, forceReload });
        (window as any).electronAPI.browserGoBack = () => invoke('browser_view_go_back');
        (window as any).electronAPI.browserGoForward = () => invoke('browser_view_go_forward');
        (window as any).electronAPI.browserReload = () => invoke('browser_view_reload');
        (window as any).electronAPI.browserOpenDevTools = () =>
          invoke('browser_view_open_devtools');
        (window as any).electronAPI.browserClear = () => invoke('browser_view_clear');
        (window as any).electronAPI.linearSaveToken = (token: string) =>
          invoke('linear_save_token', { token });
        (window as any).electronAPI.linearCheckConnection = () =>
          invoke('linear_check_connection');
        (window as any).electronAPI.linearClearToken = () => invoke('linear_clear_token');
        (window as any).electronAPI.linearInitialFetch = (limit?: number) =>
          invoke('linear_initial_fetch', { limit });
        (window as any).electronAPI.linearSearchIssues = (searchTerm: string, limit?: number) =>
          invoke('linear_search_issues', { searchTerm, limit });
        (window as any).electronAPI.jiraSaveCredentials = (args: {
          siteUrl: string;
          email: string;
          token: string;
        }) =>
          invoke('jira_save_credentials', {
            siteUrl: args.siteUrl,
            email: args.email,
            token: args.token,
          });
        (window as any).electronAPI.jiraClearCredentials = () => invoke('jira_clear_credentials');
        (window as any).electronAPI.jiraCheckConnection = () => invoke('jira_check_connection');
        (window as any).electronAPI.jiraInitialFetch = (limit?: number) =>
          invoke('jira_initial_fetch', { limit });
        (window as any).electronAPI.jiraSearchIssues = (searchTerm: string, limit?: number) =>
          invoke('jira_search_issues', { searchTerm, limit });
        (window as any).electronAPI.githubCreatePullRequestWorktree = (args: {
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
