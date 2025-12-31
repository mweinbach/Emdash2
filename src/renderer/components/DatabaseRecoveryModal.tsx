import React from 'react';
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogTitle,
} from './ui/alert-dialog';
import { Button } from './ui/button';
import { Spinner } from './ui/spinner';
import { AlertTriangle, Database, RefreshCw } from 'lucide-react';

export type DbInitErrorPayload = {
  message: string;
  dbPath?: string;
  recoveryAvailable?: boolean;
};

type DatabaseRecoveryModalProps = {
  open: boolean;
  info: DbInitErrorPayload;
  busyAction?: 'retry' | 'backup' | null;
  onRetry: () => void;
  onBackupAndReset: () => void;
  onClose: () => void;
};

const DatabaseRecoveryModal: React.FC<DatabaseRecoveryModalProps> = ({
  open,
  info,
  busyAction,
  onRetry,
  onBackupAndReset,
  onClose,
}) => {
  const canBackup = info.recoveryAvailable !== false;
  const isBusy = !!busyAction;

  return (
    <AlertDialog open={open} onOpenChange={(next) => !next && onClose()}>
      <AlertDialogContent className="max-w-xl border-border/70 bg-gradient-to-br from-background via-background/95 to-muted/50 shadow-2xl">
        <div className="flex items-start gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-full border border-border/60 bg-muted/70 text-amber-500">
            <AlertTriangle className="h-5 w-5" />
          </div>
          <div className="space-y-2">
            <AlertDialogTitle className="text-lg font-semibold">
              Database needs attention
            </AlertDialogTitle>
            <AlertDialogDescription className="text-sm text-muted-foreground">
              Emdash2 couldn&apos;t open its database. You can retry the migration or create a
              backup and start fresh.
            </AlertDialogDescription>
          </div>
        </div>

        <div className="space-y-3 rounded-lg border border-border/70 bg-muted/30 p-3 text-xs">
          <div className="flex items-center gap-2 text-amber-500">
            <Database className="h-4 w-4" />
            <span className="font-medium">Startup error</span>
          </div>
          <div className="whitespace-pre-wrap text-foreground/80">{info.message}</div>
          {info.dbPath ? (
            <div className="text-muted-foreground">DB path: {info.dbPath}</div>
          ) : null}
        </div>

        <div className="space-y-2 text-xs text-muted-foreground">
          <p>
            Backup creates a zip in your Downloads folder containing the database, a README,
            and a restore script.
          </p>
          {!canBackup ? (
            <p className="text-amber-500">Database file not found, so backup is unavailable.</p>
          ) : null}
        </div>

        <AlertDialogFooter>
          <Button type="button" variant="ghost" onClick={onClose} disabled={isBusy}>
            Dismiss
          </Button>
          <Button
            type="button"
            variant="outline"
            onClick={onRetry}
            disabled={isBusy}
            className="gap-2"
          >
            {busyAction === 'retry' ? <Spinner size="sm" /> : <RefreshCw className="h-4 w-4" />}
            Retry migration
          </Button>
          <Button
            type="button"
            onClick={onBackupAndReset}
            disabled={isBusy || !canBackup}
            className="gap-2"
          >
            {busyAction === 'backup' ? <Spinner size="sm" /> : <Database className="h-4 w-4" />}
            Backup & start fresh
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
};

export default DatabaseRecoveryModal;
