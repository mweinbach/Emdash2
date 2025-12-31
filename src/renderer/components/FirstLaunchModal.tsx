import React from 'react';
import { AlertDialog, AlertDialogContent, AlertDialogTitle } from './ui/alert-dialog';
import { Button } from './ui/button';
import { ArrowRight } from 'lucide-react';

type FirstLaunchModalProps = {
  open: boolean;
  onClose: () => void;
};

const FirstLaunchModal: React.FC<FirstLaunchModalProps> = ({ open, onClose }) => {
  return (
    <AlertDialog open={open} onOpenChange={(next) => !next && onClose()}>
      <AlertDialogContent className="max-w-3xl border-border/70 bg-gradient-to-br from-background via-background/90 to-muted/60 shadow-2xl">
        <AlertDialogTitle className="text-center text-2xl font-semibold leading-tight">
          Welcome to Emdash2
        </AlertDialogTitle>
        <div className="space-y-4">
          <p className="text-center text-sm text-muted-foreground">
            Open a project to get started, then launch a task in the workspace.
          </p>
          <div className="flex justify-center">
            <Button type="button" onClick={onClose} className="gap-2">
              Start building
              <ArrowRight className="h-4 w-4" />
            </Button>
          </div>
        </div>
      </AlertDialogContent>
    </AlertDialog>
  );
};

export default FirstLaunchModal;
