import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { Spinner } from './ui/spinner';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from './ui/alert-dialog';
import { Popover, PopoverContent, PopoverTrigger } from './ui/popover';
import { ScrollArea } from './ui/scroll-area';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Checkbox } from './ui/checkbox';
import { useToast } from '../hooks/use-toast';
import { useCreatePR } from '../hooks/useCreatePR';
import ChangesDiffModal from './ChangesDiffModal';
import AllChangesDiffModal from './AllChangesDiffModal';
import { useFileChanges } from '../hooks/useFileChanges';
import { usePrChanges } from '../hooks/usePrChanges';
import { usePrStatus } from '../hooks/usePrStatus';
import { usePtyBusy } from '../hooks/usePtyBusy';
import { isActivePr } from '../lib/prStatus';
import type { PrComment } from '../lib/prStatus';
import FileTypeIcon from './ui/file-type-icon';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from './ui/tooltip';
import { Streamdown } from 'streamdown';
import {
  Plus,
  Undo2,
  ArrowUpRight,
  FileDiff,
  ChevronDown,
  MessageSquare,
  GitMerge,
  Archive,
  RefreshCw,
} from 'lucide-react';

interface FileChangesPanelProps {
  taskId: string;
  projectPath?: string | null;
  worktreeId?: string;
  worktreeBranch?: string;
  className?: string;
  title?: string;
  subtitle?: string;
  logoSrc?: string;
  logoAlt?: string;
  logoInvert?: boolean;
  activityId?: string;
  activityProvider?: string;
  collapsible?: boolean;
  defaultCollapsed?: boolean;
}

const FileChangesPanelComponent: React.FC<FileChangesPanelProps> = ({
  taskId,
  projectPath,
  worktreeId,
  worktreeBranch,
  className,
  title,
  subtitle,
  logoSrc,
  logoAlt,
  logoInvert = false,
  activityId,
  activityProvider,
  collapsible = false,
  defaultCollapsed = false,
}) => {
  const [isCollapsed, setIsCollapsed] = useState(defaultCollapsed);
  const [showDiffModal, setShowDiffModal] = useState(false);
  const [showAllChangesModal, setShowAllChangesModal] = useState(false);
  const [selectedPath, setSelectedPath] = useState<string | undefined>(undefined);
  const [stagingFiles, setStagingFiles] = useState<Set<string>>(new Set());
  const [revertingFiles, setRevertingFiles] = useState<Set<string>>(new Set());
  const [commitMessage, setCommitMessage] = useState('');
  const [isCommitting, setIsCommitting] = useState(false);
  const [prCommentsOpen, setPrCommentsOpen] = useState(false);
  const [prComments, setPrComments] = useState<PrComment[]>([]);
  const [prCommentsLoading, setPrCommentsLoading] = useState(false);
  const [prCommentsError, setPrCommentsError] = useState<string | null>(null);
  const [mergeOpen, setMergeOpen] = useState(false);
  const [mergeMethod, setMergeMethod] = useState<
    'default' | 'merge' | 'squash' | 'rebase'
  >('default');
  const [isMerging, setIsMerging] = useState(false);
  const [archiveOpen, setArchiveOpen] = useState(false);
  const [deleteBranchOnArchive, setDeleteBranchOnArchive] = useState(false);
  const [isArchiving, setIsArchiving] = useState(false);
  const { isCreating: isCreatingPR, createPR } = useCreatePR();
  const { fileChanges, refreshChanges } = useFileChanges(taskId);
  const { toast } = useToast();
  const hasChanges = fileChanges.length > 0;
  const hasStagedChanges = fileChanges.some((change) => change.isStaged);
  const isBusy = usePtyBusy(activityId, activityProvider);
  const { pr, refresh: refreshPr } = usePrStatus(taskId);
  const [viewMode, setViewMode] = useState<'working' | 'pr'>('working');
  const {
    commits: prCommits,
    files: prFiles,
    loading: prChangesLoading,
    error: prChangesError,
    refresh: refreshPrChanges,
  } = usePrChanges(taskId, Boolean(pr && viewMode === 'pr'));
  const [branchAhead, setBranchAhead] = useState<number | null>(null);
  const [branchStatusLoading, setBranchStatusLoading] = useState<boolean>(false);
  const stagedCount = fileChanges.filter((f) => f.isStaged).length;
  const hasCommitsToPr = hasChanges || (branchAhead !== null && branchAhead > 0);

  const toNumber = (value: unknown) => {
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (typeof value === 'string') {
      const parsed = Number.parseInt(value, 10);
      if (Number.isFinite(parsed)) return parsed;
    }
    return 0;
  };

  const prState = typeof pr?.state === 'string' ? pr.state.toLowerCase() : '';
  const isPrOpen = prState === 'open';
  const isPrMerged = prState === 'merged';
  const isPrClosed = prState === 'closed';
  const prStateLabel = pr?.isDraft
    ? 'Draft'
    : isPrMerged
      ? 'Merged'
      : isPrClosed
        ? 'Closed'
        : isPrOpen
          ? 'Open'
          : prState
            ? prState.charAt(0).toUpperCase() + prState.slice(1)
            : 'PR';
  const prStateTone = pr?.isDraft
    ? 'amber'
    : isPrMerged
      ? 'emerald'
      : isPrClosed
        ? 'slate'
        : 'sky';
  const prStateClass =
    prStateTone === 'emerald'
      ? 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/40 dark:bg-emerald-500/10 dark:text-emerald-200'
      : prStateTone === 'amber'
        ? 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/40 dark:bg-amber-500/10 dark:text-amber-200'
        : prStateTone === 'slate'
          ? 'border-slate-200 bg-slate-100 text-slate-700 dark:border-slate-500/40 dark:bg-slate-500/10 dark:text-slate-200'
          : 'border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-500/40 dark:bg-sky-500/10 dark:text-sky-200';

  const commentsCount = toNumber(pr?.commentsCount);
  const reviewsCount = toNumber(pr?.reviewCount);
  const totalCommentCount = commentsCount + reviewsCount;
  const checksSummary = pr?.checksSummary;
  const checksTotal = toNumber(checksSummary?.total);
  const checksPassed = toNumber(checksSummary?.passed);
  const checksFailed = toNumber(checksSummary?.failed);
  const checksPending = toNumber(checksSummary?.pending);
  const showChecks = checksTotal > 0;
  const checksLabel =
    checksFailed > 0
      ? `${checksFailed} failing`
      : checksPending > 0
        ? `${checksPassed}/${checksTotal} pending`
        : `${checksPassed}/${checksTotal} passing`;
  const checksClass =
    checksFailed > 0
      ? 'border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-500/40 dark:bg-rose-500/10 dark:text-rose-200'
      : checksPending > 0
        ? 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/40 dark:bg-amber-500/10 dark:text-amber-200'
        : 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/40 dark:bg-emerald-500/10 dark:text-emerald-200';

  const mergeState = typeof pr?.mergeStateStatus === 'string' ? pr.mergeStateStatus.toLowerCase() : '';
  const checksOk = !checksSummary || (checksFailed === 0 && checksPending === 0);
  const mergeStateBlocked = new Set(['dirty', 'blocked', 'behind', 'draft']);
  const mergeStateAllows = !mergeState || !mergeStateBlocked.has(mergeState);
  const canMerge = Boolean(pr && isPrOpen && !pr.isDraft && mergeStateAllows && checksOk);
  const mergeDisabledReason = !pr
    ? 'No pull request found'
    : pr.isDraft
      ? 'Draft pull requests cannot be merged'
      : !isPrOpen
        ? 'Pull request is not open'
        : !checksOk
          ? 'Checks must be passing before merge'
          : !mergeStateAllows
            ? `Merge blocked (${pr?.mergeStateStatus || 'unknown'})`
            : undefined;

  const supportsPrComments = typeof window.desktopAPI?.getPrComments === 'function';
  const canArchive = Boolean(projectPath && worktreeId && isPrMerged);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      if (!taskId || hasChanges) {
        setBranchAhead(null);
        setBranchStatusLoading(false);
        return;
      }
      setBranchStatusLoading(true);
      try {
        const res = await window.desktopAPI.getBranchStatus({ taskPath: taskId });
        if (!cancelled) {
          setBranchAhead(res?.success ? (res?.ahead ?? 0) : 0);
        }
      } catch {
        if (!cancelled) setBranchAhead(0);
      } finally {
        if (!cancelled) setBranchStatusLoading(false);
      }
    };
    load();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [taskId, hasChanges]);

  useEffect(() => {
    if (!taskId) return;
    if (!pr) {
      setViewMode('working');
      return;
    }
    if (fileChanges.length === 0) {
      setViewMode('pr');
    } else {
      setViewMode('working');
    }
  }, [fileChanges.length, pr?.number, taskId]);

  useEffect(() => {
    setIsCollapsed(defaultCollapsed);
  }, [taskId, defaultCollapsed]);

  const loadPrComments = useCallback(async () => {
    if (!taskId || !pr) {
      setPrComments([]);
      setPrCommentsError(null);
      return;
    }
    if (!supportsPrComments) {
      setPrComments([]);
      setPrCommentsError('PR comments are unavailable in this build.');
      return;
    }

    setPrCommentsLoading(true);
    setPrCommentsError(null);
    try {
      const res = await window.desktopAPI.getPrComments({ taskPath: taskId });
      if (res?.success) {
        const items = Array.isArray(res.comments) ? (res.comments as PrComment[]) : [];
        setPrComments(items);
      } else {
        setPrComments([]);
        setPrCommentsError(res?.error || 'Failed to load comments.');
      }
    } catch (error: any) {
      setPrComments([]);
      setPrCommentsError(error?.message || 'Failed to load comments.');
    } finally {
      setPrCommentsLoading(false);
    }
  }, [pr, supportsPrComments, taskId]);

  useEffect(() => {
    if (!prCommentsOpen) return;
    void loadPrComments();
  }, [loadPrComments, prCommentsOpen]);

  useEffect(() => {
    setPrCommentsOpen(false);
    setPrComments([]);
    setPrCommentsError(null);
  }, [pr?.number, taskId]);

  useEffect(() => {
    if (!taskId || !pr || !isActivePr(pr)) return;
    const interval = window.setInterval(() => {
      refreshPr().catch(() => {});
    }, 20000);
    return () => window.clearInterval(interval);
  }, [pr, refreshPr, taskId]);

  const handleStageFile = async (filePath: string, event: React.MouseEvent) => {
    event.stopPropagation(); // Prevent opening diff modal
    setStagingFiles((prev) => new Set(prev).add(filePath));

    try {
      const result = await window.desktopAPI.stageFile({
        taskPath: taskId,
        filePath,
      });

      if (result.success) {
        await refreshChanges();
      } else {
        toast({
          title: 'Stage Failed',
          description: result.error || 'Failed to stage file.',
          variant: 'destructive',
        });
      }
    } catch (_error) {
      toast({
        title: 'Stage Failed',
        description: 'An unexpected error occurred.',
        variant: 'destructive',
      });
    } finally {
      setStagingFiles((prev) => {
        const newSet = new Set(prev);
        newSet.delete(filePath);
        return newSet;
      });
    }
  };

  const handleRevertFile = async (filePath: string, event: React.MouseEvent) => {
    event.stopPropagation(); // Prevent opening diff modal
    setRevertingFiles((prev) => new Set(prev).add(filePath));

    try {
      const result = await window.desktopAPI.revertFile({
        taskPath: taskId,
        filePath,
      });

      if (result.success) {
        const action = result.action;
        if (action !== 'unstaged') {
          toast({
            title: 'File Reverted',
            description: `${filePath} changes have been reverted.`,
          });
        }
        await refreshChanges();
      } else {
        toast({
          title: 'Revert Failed',
          description: result.error || 'Failed to revert file.',
          variant: 'destructive',
        });
      }
    } catch (_error) {
      toast({
        title: 'Revert Failed',
        description: 'An unexpected error occurred.',
        variant: 'destructive',
      });
    } finally {
      setRevertingFiles((prev) => {
        const newSet = new Set(prev);
        newSet.delete(filePath);
        return newSet;
      });
    }
  };

  const handleCommitAndPush = async () => {
    if (!commitMessage.trim()) {
      toast({
        title: 'Commit Message Required',
        description: 'Please enter a commit message.',
        variant: 'destructive',
      });
      return;
    }

    if (!hasStagedChanges) {
      toast({
        title: 'No Staged Changes',
        description: 'Please stage some files before committing.',
        variant: 'destructive',
      });
      return;
    }

    setIsCommitting(true);
    try {
      const result = await window.desktopAPI.gitCommitAndPush({
        taskPath: taskId,
        commitMessage: commitMessage.trim(),
        createBranchIfOnDefault: true,
        branchPrefix: 'feature',
      });

      if (result.success) {
        toast({
          title: 'Committed and Pushed',
          description: `Changes committed with message: "${commitMessage.trim()}"`,
        });
        setCommitMessage(''); // Clear the input
        await refreshChanges();
        try {
          await refreshPr();
        } catch {}
        // Proactively load branch status so the Create PR button appears immediately
        try {
          setBranchStatusLoading(true);
          const bs = await window.desktopAPI.getBranchStatus({ taskPath: taskId });
          setBranchAhead(bs?.success ? (bs?.ahead ?? 0) : 0);
        } catch {
          setBranchAhead(0);
        } finally {
          setBranchStatusLoading(false);
        }
      } else {
        toast({
          title: 'Commit Failed',
          description: result.error || 'Failed to commit and push changes.',
          variant: 'destructive',
        });
      }
    } catch (_error) {
      toast({
        title: 'Commit Failed',
        description: 'An unexpected error occurred.',
        variant: 'destructive',
      });
    } finally {
      setIsCommitting(false);
    }
  };

  const handleMergePullRequest = async () => {
    if (!taskId) return;
    setIsMerging(true);
    try {
      const res = await window.desktopAPI.mergePullRequest({
        taskPath: taskId,
        method: mergeMethod === 'default' ? undefined : mergeMethod,
      });
      if (res?.success) {
        toast({
          title: 'Pull request merged',
          description: res?.output || 'PR merged successfully.',
        });
        setMergeOpen(false);
        try {
          await refreshPr();
        } catch {}
      } else {
        toast({
          title: 'Merge Failed',
          description: res?.error || 'Failed to merge pull request.',
          variant: 'destructive',
        });
      }
    } catch (error: any) {
      toast({
        title: 'Merge Failed',
        description: error?.message || 'Failed to merge pull request.',
        variant: 'destructive',
      });
    } finally {
      setIsMerging(false);
    }
  };

  const handleArchiveWorktree = async () => {
    if (!projectPath || !worktreeId) {
      toast({
        title: 'Archive Failed',
        description: 'Project or worktree information is missing.',
        variant: 'destructive',
      });
      return;
    }

    setIsArchiving(true);
    try {
      const res = await window.desktopAPI.worktreeRemove({
        projectPath,
        worktreeId,
        worktreePath: taskId,
        branch: deleteBranchOnArchive ? worktreeBranch : undefined,
      });
      if (res?.success) {
        toast({
          title: 'Worktree archived',
          description: deleteBranchOnArchive
            ? 'Worktree removed and branch deleted.'
            : 'Worktree removed successfully.',
        });
        setArchiveOpen(false);
        setDeleteBranchOnArchive(false);
      } else {
        toast({
          title: 'Archive Failed',
          description: res?.error || 'Failed to archive worktree.',
          variant: 'destructive',
        });
      }
    } catch (error: any) {
      toast({
        title: 'Archive Failed',
        description: error?.message || 'Failed to archive worktree.',
        variant: 'destructive',
      });
    } finally {
      setIsArchiving(false);
    }
  };

  const formatReviewState = (value?: string | null) => {
    if (!value) return '';
    return value
      .toLowerCase()
      .split('_')
      .map((part) => (part ? part[0].toUpperCase() + part.slice(1) : ''))
      .filter(Boolean)
      .join(' ');
  };

  const formatTimestamp = (value?: string | null) => {
    if (!value) return '';
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return '';
    return date.toLocaleString();
  };

  const renderPath = (p: string) => {
    const last = p.lastIndexOf('/');
    const dir = last >= 0 ? p.slice(0, last + 1) : '';
    const base = last >= 0 ? p.slice(last + 1) : p;
    return (
      <span className="truncate">
        {dir && <span className="text-muted-foreground">{dir}</span>}
        <span className="font-medium text-foreground">{base}</span>
      </span>
    );
  };

  const changeTypeMeta = (value?: string | null) => {
    const normalized = value?.toLowerCase() ?? '';
    if (normalized === 'added') {
      return {
        label: 'Added',
        className:
          'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/40 dark:bg-emerald-500/10 dark:text-emerald-200',
      };
    }
    if (normalized === 'deleted') {
      return {
        label: 'Deleted',
        className:
          'border-rose-200 bg-rose-50 text-rose-700 dark:border-rose-500/40 dark:bg-rose-500/10 dark:text-rose-200',
      };
    }
    if (normalized === 'renamed') {
      return {
        label: 'Renamed',
        className:
          'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/40 dark:bg-amber-500/10 dark:text-amber-200',
      };
    }
    if (normalized === 'copied') {
      return {
        label: 'Copied',
        className:
          'border-slate-200 bg-slate-100 text-slate-700 dark:border-slate-500/40 dark:bg-slate-500/10 dark:text-slate-200',
      };
    }
    return {
      label: normalized ? normalized[0].toUpperCase() + normalized.slice(1) : 'Modified',
      className:
        'border-sky-200 bg-sky-50 text-sky-700 dark:border-sky-500/40 dark:bg-sky-500/10 dark:text-sky-200',
    };
  };

  const workingTotals = useMemo(
    () =>
      fileChanges.reduce(
        (acc, change) => ({
          additions: acc.additions + change.additions,
          deletions: acc.deletions + change.deletions,
        }),
        { additions: 0, deletions: 0 }
      ),
    [fileChanges]
  );
  const prTotals = useMemo(
    () =>
      prFiles.reduce(
        (acc, change) => ({
          additions: acc.additions + (change.additions || 0),
          deletions: acc.deletions + (change.deletions || 0),
        }),
        { additions: 0, deletions: 0 }
      ),
    [prFiles]
  );
  const prAdditions = toNumber(pr?.additions) || prTotals.additions;
  const prDeletions = toNumber(pr?.deletions) || prTotals.deletions;
  const activeTotals =
    viewMode === 'pr'
      ? { additions: prAdditions, deletions: prDeletions }
      : workingTotals;
  const activeFileCount =
    viewMode === 'pr'
      ? prFiles.length || toNumber(pr?.changedFiles)
      : fileChanges.length;

  const commentButton = (
    <button
      type="button"
      className={`relative inline-flex items-center gap-1 rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground ${
        supportsPrComments ? '' : 'cursor-not-allowed opacity-50'
      }`}
      title={supportsPrComments ? 'View PR comments' : 'PR comments unavailable'}
      aria-label="View pull request comments"
      disabled={!supportsPrComments}
    >
      <MessageSquare className="size-3" />
      {totalCommentCount > 0 ? (
        <span className="rounded-full bg-foreground/80 px-1 text-[9px] text-background">
          {totalCommentCount}
        </span>
      ) : null}
    </button>
  );

  const prActionControls = pr ? (
    <>
      {showChecks ? (
        <span
          className={`inline-flex items-center rounded border px-1.5 py-0.5 text-[10px] font-medium ${checksClass}`}
        >
          Checks {checksLabel}
        </span>
      ) : null}
      {supportsPrComments ? (
        <Popover open={prCommentsOpen} onOpenChange={setPrCommentsOpen}>
          <PopoverTrigger asChild>{commentButton}</PopoverTrigger>
          <PopoverContent
            align="end"
            className="w-80 border border-border/70 bg-popover p-3 shadow-lift"
          >
            <div className="flex items-center justify-between gap-2">
              <span className="text-xs font-semibold text-muted-foreground">Comments</span>
              {pr.url ? (
                <Button
                  variant="ghost"
                  size="sm"
                  className="h-7 px-2 text-[10px]"
                  onClick={() => {
                    const prUrl = pr.url;
                    if (prUrl) window.desktopAPI?.openExternal?.(prUrl);
                  }}
                >
                  Open PR
                  <ArrowUpRight className="ml-1 h-3 w-3" />
                </Button>
              ) : null}
            </div>
            <div className="mt-2">
              {prCommentsLoading ? (
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <Spinner size="sm" />
                  Loading comments...
                </div>
              ) : prCommentsError ? (
                <div className="text-xs text-rose-600 dark:text-rose-300">
                  {prCommentsError}
                </div>
              ) : prComments.length === 0 ? (
                <div className="text-xs text-muted-foreground">No comments yet.</div>
              ) : (
                <ScrollArea className="h-72 max-h-72">
                  <div className="space-y-2 pr-2">
                    {prComments.map((comment, index) => {
                      const authorLabel =
                        comment.author && typeof comment.author === 'object'
                          ? String(
                              (comment.author as any).login ||
                                (comment.author as any).name ||
                                'Unknown'
                            )
                          : 'Unknown';
                      const body = typeof comment.body === 'string' ? comment.body.trim() : '';
                      const reviewState = formatReviewState(comment.state || undefined);
                      const content = body || reviewState || 'No comment';
                      const timestamp = formatTimestamp(comment.createdAt || undefined);
                      const typeLabel = comment.type === 'review' ? 'Review' : 'Comment';
                      return (
                        <div
                          key={`${comment.id ?? 'comment'}-${index}`}
                          className="rounded-md border border-border/60 bg-surface/70 px-2 py-2"
                        >
                          <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                            <span className="font-medium text-foreground">{authorLabel}</span>
                            <span className="rounded border border-border/70 bg-background px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
                              {typeLabel}
                            </span>
                            {reviewState ? (
                              <span className="rounded border border-border/70 bg-background px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground">
                                {reviewState}
                              </span>
                            ) : null}
                            {timestamp ? (
                              <span className="ml-auto text-[10px] text-muted-foreground/80">
                                {timestamp}
                              </span>
                            ) : null}
                          </div>
                          <div className="mt-1 text-xs text-foreground/90">
                            <Streamdown className="streamdown text-xs leading-relaxed">
                              {content}
                            </Streamdown>
                          </div>
                        </div>
                      );
                    })}
                  </div>
                </ScrollArea>
              )}
            </div>
          </PopoverContent>
        </Popover>
      ) : (
        commentButton
      )}
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          const prUrl = pr.url;
          if (prUrl) window.desktopAPI?.openExternal?.(prUrl);
        }}
        className="inline-flex items-center gap-1 rounded border border-border bg-muted px-1.5 py-0.5 text-[10px] font-medium text-muted-foreground transition-colors hover:bg-accent hover:text-accent-foreground"
        title={`${pr.title || 'Pull Request'} (#${pr.number})`}
      >
        View PR
        <ArrowUpRight className="size-3" />
      </button>
      {isPrOpen ? (
        <AlertDialog open={mergeOpen} onOpenChange={setMergeOpen}>
          <AlertDialogTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="h-7 px-2 text-[10px]"
              disabled={!canMerge || isMerging}
              title={mergeDisabledReason}
            >
              {isMerging ? <Spinner size="sm" className="mr-1" /> : null}
              <GitMerge className="mr-1 h-3 w-3" />
              Merge
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Merge pull request</AlertDialogTitle>
              <AlertDialogDescription>
                This will merge the PR into the base branch.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <div className="space-y-3">
              <div className="space-y-1">
                <span className="text-xs font-medium text-muted-foreground">Merge method</span>
                <Select
                  value={mergeMethod}
                  onValueChange={(value) =>
                    setMergeMethod(value as 'default' | 'merge' | 'squash' | 'rebase')
                  }
                >
                  <SelectTrigger className="h-8 text-xs">
                    <SelectValue placeholder="Select method" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="default">Default</SelectItem>
                    <SelectItem value="merge">Merge commit</SelectItem>
                    <SelectItem value="squash">Squash</SelectItem>
                    <SelectItem value="rebase">Rebase</SelectItem>
                  </SelectContent>
                </Select>
              </div>
              {showChecks ? (
                <div className="text-xs text-muted-foreground">Checks: {checksLabel}</div>
              ) : null}
            </div>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={isMerging}>Cancel</AlertDialogCancel>
              <AlertDialogAction
                className="bg-primary px-4 py-2 text-primary-foreground hover:bg-primary/90"
                disabled={!canMerge || isMerging}
                onClick={(e) => {
                  e.stopPropagation();
                  void handleMergePullRequest();
                }}
              >
                {isMerging ? <Spinner className="mr-2 h-4 w-4" size="sm" /> : null}
                Merge
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      ) : null}
      {canArchive ? (
        <AlertDialog open={archiveOpen} onOpenChange={setArchiveOpen}>
          <AlertDialogTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="h-7 px-2 text-[10px]"
              disabled={isArchiving}
            >
              {isArchiving ? <Spinner size="sm" className="mr-1" /> : null}
              <Archive className="mr-1 h-3 w-3" />
              Archive
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Archive worktree</AlertDialogTitle>
              <AlertDialogDescription>
                Remove the worktree directory and optionally delete the branch.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <div className="space-y-3 text-sm">
              <label className="flex items-center gap-2">
                <Checkbox
                  checked={deleteBranchOnArchive}
                  onCheckedChange={(checked) => setDeleteBranchOnArchive(Boolean(checked))}
                />
                <span className="text-sm text-foreground">
                  Delete branch{worktreeBranch ? ` (${worktreeBranch})` : ''}
                </span>
              </label>
            </div>
            <AlertDialogFooter>
              <AlertDialogCancel disabled={isArchiving}>Cancel</AlertDialogCancel>
              <AlertDialogAction
                className="bg-destructive px-4 py-2 text-destructive-foreground hover:bg-destructive/90"
                disabled={isArchiving}
                onClick={(e) => {
                  e.stopPropagation();
                  void handleArchiveWorktree();
                }}
              >
                {isArchiving ? <Spinner className="mr-2 h-4 w-4" size="sm" /> : null}
                Archive
              </AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      ) : null}
    </>
  ) : null;

  return (
    <div className={`flex h-full flex-col bg-card/80 shadow-sm ${className}`}>
      <div className="bg-surface-2 px-3 py-2">
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-2">
            <div className="flex min-w-0 items-center gap-2">
              <span className="inline-flex min-w-0 items-center gap-1.5 rounded-md border border-border/70 bg-muted/40 px-2 py-0.5 text-[11px] font-semibold text-foreground">
                {logoSrc ? (
                  <img
                    src={logoSrc}
                    alt={logoAlt || title || 'Agent'}
                    className={`h-3.5 w-3.5 object-contain ${logoInvert ? 'dark:invert' : ''}`}
                  />
                ) : null}
                <span className="truncate">{title || 'Changes'}</span>
              </span>
              <div className="flex shrink-0 items-center gap-1 text-xs">
                <span className="font-medium text-green-600 dark:text-green-400">
                  +{activeTotals.additions}
                </span>
                <span className="text-muted-foreground/70">•</span>
                <span className="font-medium text-red-600 dark:text-red-400">
                  -{activeTotals.deletions}
                </span>
              </div>
            </div>
            <div className="flex items-center gap-2">
              {activityId ? (
                <span
                  className={`inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-medium ${
                    isBusy
                      ? 'border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-500/40 dark:bg-amber-500/10 dark:text-amber-200'
                      : 'border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-500/40 dark:bg-emerald-500/10 dark:text-emerald-200'
                  }`}
                >
                  <span
                    className={`h-1.5 w-1.5 rounded-full ${
                      isBusy ? 'bg-amber-500 animate-pulse' : 'bg-emerald-500'
                    }`}
                  />
                  {isBusy ? 'Working' : 'Idle'}
                </span>
              ) : null}
              {collapsible ? (
                <button
                  type="button"
                  onClick={() => setIsCollapsed((prev) => !prev)}
                  className="inline-flex h-7 w-7 items-center justify-center rounded-md border border-transparent text-muted-foreground transition hover:border-border/60 hover:bg-accent/30 hover:text-foreground"
                  aria-label={isCollapsed ? 'Expand changes' : 'Collapse changes'}
                  aria-expanded={!isCollapsed}
                >
                  <ChevronDown
                    className={`h-4 w-4 transition-transform ${
                      isCollapsed ? '-rotate-90' : 'rotate-0'
                    }`}
                  />
                </button>
              ) : null}
            </div>
          </div>

          {subtitle ? (
            <div className="flex min-w-0 items-center">
              <span className="truncate text-xs text-muted-foreground">{subtitle}</span>
            </div>
          ) : null}

          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex min-w-0 flex-1 items-center gap-2 overflow-hidden">
              <span className="truncate text-sm font-medium text-foreground">
                {activeFileCount} {viewMode === 'pr' ? 'PR files' : 'files changed'}
              </span>
              {viewMode === 'working' && hasStagedChanges && (
                <span className="shrink-0 rounded bg-surface-3 px-2 py-0.5 text-xs font-medium text-foreground/80">
                  {stagedCount} staged
                </span>
              )}
              {pr ? (
                <div className="shrink-0 rounded-md border border-border/70 bg-muted/30 p-0.5 text-[10px] font-semibold text-muted-foreground">
                  <button
                    type="button"
                    onClick={() => setViewMode('working')}
                    aria-pressed={viewMode === 'working'}
                    className={`rounded px-2 py-0.5 transition-colors ${
                      viewMode === 'working'
                        ? 'bg-background text-foreground shadow-sm'
                        : 'text-muted-foreground hover:text-foreground'
                    }`}
                  >
                    Working
                  </button>
                  <button
                    type="button"
                    onClick={() => setViewMode('pr')}
                    aria-pressed={viewMode === 'pr'}
                    className={`rounded px-2 py-0.5 transition-colors ${
                      viewMode === 'pr'
                        ? 'bg-background text-foreground shadow-sm'
                        : 'text-muted-foreground hover:text-foreground'
                    }`}
                  >
                    Pull Request
                  </button>
                </div>
              ) : null}
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Button
                variant="outline"
                size="sm"
                className="h-8 shrink-0 border-border/70 px-2 text-xs text-foreground/80"
                title={
                  viewMode === 'pr'
                    ? 'Refresh pull request details'
                    : 'View all changes in a single scrollable view'
                }
                disabled={viewMode === 'pr' ? prChangesLoading : false}
                onClick={() => {
                  if (viewMode === 'pr') {
                    void refreshPr();
                    void refreshPrChanges();
                    if (prCommentsOpen) {
                      void loadPrComments();
                    }
                  } else {
                    setShowAllChangesModal(true);
                  }
                }}
              >
                {viewMode === 'pr' ? (
                  <RefreshCw className="h-3.5 w-3.5 sm:mr-1.5" />
                ) : (
                  <FileDiff className="h-3.5 w-3.5 sm:mr-1.5" />
                )}
                <span className="hidden sm:inline">
                  {viewMode === 'pr'
                    ? prChangesLoading
                      ? 'Refreshing...'
                      : 'Refresh PR'
                    : 'Check Changes'}
                </span>
              </Button>
              {pr ? (
                <div className="flex flex-wrap items-center gap-1.5">
                  <span
                    className={`inline-flex items-center rounded border px-1.5 py-0.5 text-[10px] font-semibold ${prStateClass}`}
                  >
                    {prStateLabel}
                  </span>
                  {viewMode !== 'pr' ? prActionControls : null}
                </div>
              ) : (
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 shrink-0 border-border/70 px-2 text-xs text-foreground/80"
                  disabled={isCreatingPR || branchStatusLoading || !hasCommitsToPr}
                  title={
                    !hasCommitsToPr
                      ? 'No commits ahead to open a PR'
                      : 'Commit all changes and create a pull request'
                  }
                  onClick={async () => {
                    void (async () => {
                      const { captureTelemetry } = await import('../lib/telemetryClient');
                      captureTelemetry('pr_viewed');
                    })();
                    await createPR({
                      taskPath: taskId,
                      onSuccess: async () => {
                        await refreshChanges();
                        try {
                          await refreshPr();
                        } catch {}
                      },
                    });
                  }}
                >
                  {isCreatingPR || branchStatusLoading ? <Spinner size="sm" /> : 'Create PR'}
                </Button>
              )}
            </div>
          </div>
          {pr && viewMode === 'pr' ? (
            <div className="mt-1 flex flex-wrap items-center gap-2">
              {prActionControls}
            </div>
          ) : null}

          {!isCollapsed && viewMode === 'working' && hasStagedChanges && (
            <div className="flex items-center space-x-2">
              <Input
                placeholder="Enter commit message..."
                value={commitMessage}
                onChange={(e) => setCommitMessage(e.target.value)}
                className="h-8 flex-1 text-sm"
                onKeyDown={(e) => {
                  if (e.key === 'Enter' && !e.shiftKey) {
                    e.preventDefault();
                    handleCommitAndPush();
                  }
                }}
              />
              <Button
                variant="outline"
                size="sm"
                className="h-8 border-border/70 px-2 text-xs text-foreground/80"
                title="Commit all staged changes and push the branch"
                onClick={handleCommitAndPush}
                disabled={isCommitting || !commitMessage.trim()}
              >
                {isCommitting ? <Spinner size="sm" /> : 'Commit & Push'}
              </Button>
            </div>
          )}
        </div>
      </div>

      {!isCollapsed && (
        <div className="min-h-0 flex-1 overflow-y-auto">
          {viewMode === 'pr' ? (
            <div className="space-y-4 px-4 py-3">
              <div className="rounded-md border border-border/70 bg-surface/70 p-3">
                <div className="flex items-start justify-between gap-2">
                  <div className="min-w-0">
                    <div className="text-xs font-semibold text-muted-foreground">Pull Request</div>
                    <div className="mt-1 line-clamp-2 text-sm font-semibold text-foreground">
                      {pr?.title || (pr?.number ? `PR #${pr.number}` : 'Pull Request')}
                    </div>
                    {pr?.baseRefName || pr?.headRefName ? (
                      <div className="mt-1 text-[11px] text-muted-foreground">
                        {pr?.baseRefName || 'base'} {'->'} {pr?.headRefName || 'head'}
                      </div>
                    ) : null}
                  </div>
                  {pr?.url ? (
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 px-2 text-[10px]"
                      onClick={() => window.desktopAPI?.openExternal?.(pr.url as string)}
                    >
                      Open PR
                      <ArrowUpRight className="ml-1 h-3 w-3" />
                    </Button>
                  ) : null}
                </div>
                <div className="mt-3 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                  <span className="rounded border border-border/70 bg-background px-1.5 py-0.5">
                    {prCommits.length} commits
                  </span>
                  <span className="rounded border border-border/70 bg-background px-1.5 py-0.5">
                    {activeFileCount} files
                  </span>
                  <span className="rounded border border-border/70 bg-background px-1.5 py-0.5">
                    +{prAdditions} / -{prDeletions}
                  </span>
                </div>
              </div>

              {prChangesLoading && prCommits.length === 0 && prFiles.length === 0 ? (
                <div className="flex items-center gap-2 text-xs text-muted-foreground">
                  <Spinner size="sm" />
                  Loading PR details...
                </div>
              ) : prChangesError ? (
                <div className="rounded-md border border-rose-200 bg-rose-50 px-3 py-2 text-xs text-rose-700 dark:border-rose-500/40 dark:bg-rose-500/10 dark:text-rose-200">
                  {prChangesError}
                </div>
              ) : (
                <>
                  <div className="space-y-2">
                    <div className="flex items-center justify-between text-xs font-semibold text-muted-foreground">
                      <span>Commits</span>
                      <span className="text-[11px] font-medium">{prCommits.length}</span>
                    </div>
                    {prCommits.length === 0 ? (
                      <div className="rounded-md border border-border/60 bg-background/60 px-3 py-2 text-xs text-muted-foreground">
                        No commits found.
                      </div>
                    ) : (
                      prCommits.map((commit, index) => {
                        const message = commit.message?.trim() || 'Untitled commit';
                        const author = commit.author?.trim();
                        const dateLabel = formatTimestamp(commit.date);
                        const shortOid =
                          commit.shortOid?.trim() || commit.oid?.slice(0, 7) || '';
                        return (
                          <div
                            key={commit.oid || `${message}-${index}`}
                            className="rounded-md border border-border/60 bg-background/60 px-3 py-2"
                          >
                            <div className="flex items-start justify-between gap-2">
                              <div className="min-w-0">
                                <div className="truncate text-sm font-medium text-foreground">
                                  {message}
                                </div>
                                <div className="mt-1 flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
                                  {shortOid ? (
                                    <span className="rounded border border-border/70 bg-background px-1.5 py-0.5 font-mono text-[10px]">
                                      {shortOid}
                                    </span>
                                  ) : null}
                                  {author ? <span>{author}</span> : null}
                                </div>
                              </div>
                              {dateLabel ? (
                                <span className="shrink-0 text-[10px] text-muted-foreground">
                                  {dateLabel}
                                </span>
                              ) : null}
                            </div>
                          </div>
                        );
                      })
                    )}
                  </div>

                  <div className="space-y-2">
                    <div className="flex items-center justify-between text-xs font-semibold text-muted-foreground">
                      <span>Files</span>
                      <span className="text-[11px] font-medium">{prFiles.length}</span>
                    </div>
                    {prFiles.length === 0 ? (
                      <div className="rounded-md border border-border/60 bg-background/60 px-3 py-2 text-xs text-muted-foreground">
                        No files listed.
                      </div>
                    ) : (
                      prFiles.map((file, index) => {
                        const meta = changeTypeMeta(file.changeType);
                        return (
                          <div
                            key={`${file.path}-${index}`}
                            className="flex items-center justify-between gap-3 rounded-md border border-border/60 bg-background/60 px-3 py-2"
                          >
                            <div className="min-w-0 flex-1">
                              <div className="truncate text-sm">{renderPath(file.path)}</div>
                            </div>
                            <div className="flex items-center gap-2">
                              <span
                                className={`rounded border px-1.5 py-0.5 text-[10px] font-semibold ${meta.className}`}
                              >
                                {meta.label}
                              </span>
                              {file.additions && file.additions > 0 ? (
                                <span className="rounded bg-green-50 px-1.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:bg-green-900/30 dark:text-emerald-300">
                                  +{file.additions}
                                </span>
                              ) : null}
                              {file.deletions && file.deletions > 0 ? (
                                <span className="rounded bg-rose-50 px-1.5 py-0.5 text-[11px] font-medium text-rose-700 dark:bg-rose-900/30 dark:text-rose-300">
                                  -{file.deletions}
                                </span>
                              ) : null}
                            </div>
                          </div>
                        );
                      })
                    )}
                  </div>
                </>
              )}
            </div>
          ) : (
            fileChanges.map((change, index) => (
              <div
                key={index}
                className={`flex cursor-pointer items-center justify-between border-b border-border/60 px-4 py-2.5 last:border-b-0 transition-colors hover:bg-surface-2 ${
                  change.isStaged ? 'bg-surface-2/70' : ''
                }`}
                onClick={() => {
                  void (async () => {
                    const { captureTelemetry } = await import('../lib/telemetryClient');
                    captureTelemetry('changes_viewed');
                  })();
                  setSelectedPath(change.path);
                  setShowDiffModal(true);
                }}
              >
                <div className="flex min-w-0 flex-1 items-center gap-3">
                  <span className="inline-flex h-4 w-4 items-center justify-center text-muted-foreground">
                    <FileTypeIcon
                      path={change.path}
                      type={change.status === 'deleted' ? 'file' : 'file'}
                      size={14}
                    />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-sm">{renderPath(change.path)}</div>
                  </div>
                </div>
                <div className="ml-3 flex items-center gap-2">
                  {change.additions > 0 && (
                    <span className="rounded bg-green-50 px-1.5 py-0.5 text-[11px] font-medium text-emerald-700 dark:bg-green-900/30 dark:text-emerald-300">
                      +{change.additions}
                    </span>
                  )}
                  {change.deletions > 0 && (
                    <span className="rounded bg-rose-50 px-1.5 py-0.5 text-[11px] font-medium text-rose-700 dark:bg-rose-900/30 dark:text-rose-300">
                      -{change.deletions}
                    </span>
                  )}
                  <div className="flex items-center gap-1">
                    {!change.isStaged && (
                      <TooltipProvider delayDuration={100}>
                        <Tooltip>
                          <TooltipTrigger asChild>
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-8 w-8 text-muted-foreground hover:bg-accent/30 hover:text-foreground"
                              onClick={(e) => handleStageFile(change.path, e)}
                              disabled={stagingFiles.has(change.path)}
                            >
                              {stagingFiles.has(change.path) ? (
                                <Spinner size="sm" />
                              ) : (
                                <Plus className="h-4 w-4" />
                              )}
                            </Button>
                          </TooltipTrigger>
                          <TooltipContent
                            side="left"
                            className="max-w-xs border border-border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-lift"
                          >
                            <p className="font-medium">Stage file for commit</p>
                            <p className="mt-0.5 text-xs text-muted-foreground">
                              Add this file to the staging area so it will be included in the next
                              commit
                            </p>
                          </TooltipContent>
                        </Tooltip>
                      </TooltipProvider>
                    )}
                    <TooltipProvider delayDuration={100}>
                      <Tooltip>
                        <TooltipTrigger asChild>
                          <Button
                            variant="ghost"
                            size="icon"
                            className="h-8 w-8 text-muted-foreground hover:bg-accent/30 hover:text-foreground"
                            onClick={(e) => handleRevertFile(change.path, e)}
                            disabled={revertingFiles.has(change.path)}
                          >
                            {revertingFiles.has(change.path) ? (
                              <Spinner size="sm" />
                            ) : (
                              <Undo2 className="h-4 w-4" />
                            )}
                          </Button>
                        </TooltipTrigger>
                        <TooltipContent
                          side="left"
                          className="max-w-xs border border-border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-lift"
                        >
                          {change.isStaged ? (
                            <>
                              <p className="font-medium">Unstage file</p>
                              <p className="mt-0.5 text-xs text-muted-foreground">
                                Remove this file from staging. Click again to discard all changes
                                to this file.
                              </p>
                            </>
                          ) : (
                            <>
                              <p className="font-medium">Revert file changes</p>
                              <p className="mt-0.5 text-xs text-muted-foreground">
                                Discard all uncommitted changes to this file and restore it to the
                                last committed version
                              </p>
                            </>
                          )}
                        </TooltipContent>
                      </Tooltip>
                    </TooltipProvider>
                  </div>
                </div>
              </div>
            ))
          )}
        </div>
      )}
      {showDiffModal && (
        <ChangesDiffModal
          open={showDiffModal}
          onClose={() => setShowDiffModal(false)}
          taskPath={taskId}
          files={fileChanges}
          initialFile={selectedPath}
          onRefreshChanges={refreshChanges}
        />
      )}
      {showAllChangesModal && (
        <AllChangesDiffModal
          open={showAllChangesModal}
          onClose={() => setShowAllChangesModal(false)}
          taskPath={taskId}
          files={fileChanges}
          onRefreshChanges={refreshChanges}
        />
      )}
    </div>
  );
};
export const FileChangesPanel = React.memo(FileChangesPanelComponent);

export default FileChangesPanel;
