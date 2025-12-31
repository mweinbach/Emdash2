import React, { useCallback } from 'react';
import { Command, Settings as SettingsIcon, KanbanSquare } from 'lucide-react';
import SidebarLeftToggleButton from './SidebarLeftToggleButton';
import SidebarRightToggleButton from './SidebarRightToggleButton';
import { Button } from '../ui/button';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '../ui/tooltip';
import OpenInMenu from './OpenInMenu';
import BrowserToggleButton from './BrowserToggleButton';

interface GithubUser {
  login?: string;
  name?: string;
  html_url?: string;
  email?: string;
}

interface TitlebarProps {
  onToggleSettings: () => void;
  isSettingsOpen?: boolean;
  currentPath?: string | null;
  githubUser?: GithubUser | null;
  defaultPreviewUrl?: string | null;
  taskId?: string | null;
  taskPath?: string | null;
  projectPath?: string | null;
  isTaskMultiAgent?: boolean;
  onToggleKanban?: () => void;
  isKanbanOpen?: boolean;
  kanbanAvailable?: boolean;
}

const Titlebar: React.FC<TitlebarProps> = ({
  onToggleSettings,
  isSettingsOpen = false,
  currentPath,
  githubUser,
  defaultPreviewUrl,
  taskId,
  taskPath,
  projectPath,
  isTaskMultiAgent,
  onToggleKanban,
  isKanbanOpen = false,
  kanbanAvailable = false,
}) => {
  return (
    <>
      <header
        className="fixed inset-x-0 top-0 z-[80] flex h-[var(--tb,36px)] items-center justify-end bg-surface/95 pr-3 shadow-[inset_0_-1px_0_hsl(var(--border))] [-webkit-app-region:drag]"
        data-tauri-drag-region
      >
        <div className="flex-1 h-full" data-tauri-drag-region />
        <div
          className="pointer-events-auto flex items-center gap-1 [-webkit-app-region:no-drag]"
          data-tauri-drag-region="false"
        >
          {currentPath ? <OpenInMenu path={currentPath} align="right" /> : null}
          {kanbanAvailable ? (
            <TooltipProvider delayDuration={200}>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label="Toggle Kanban view"
                    onClick={async () => {
                      const newState = !isKanbanOpen;
                      void import('../../lib/telemetryClient').then(({ captureTelemetry }) => {
                        captureTelemetry('toolbar_kanban_toggled', {
                          state: newState ? 'open' : 'closed',
                        });
                      });
                      onToggleKanban?.();
                    }}
                    className="h-8 w-8 text-muted-foreground hover:bg-background/80"
                  >
                    <KanbanSquare className="h-4 w-4" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" className="text-xs font-medium">
                  <div className="flex flex-col gap-1">
                    <span>Toggle Kanban view</span>
                    <span className="flex items-center gap-1 text-muted-foreground">
                      <Command className="h-3 w-3" aria-hidden="true" />
                      <span>P</span>
                    </span>
                  </div>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
          ) : null}
          {taskId && !isTaskMultiAgent ? (
            <BrowserToggleButton
              defaultUrl={defaultPreviewUrl || undefined}
              taskId={taskId}
              taskPath={taskPath}
              parentProjectPath={projectPath}
            />
          ) : null}
          <SidebarLeftToggleButton />
          <SidebarRightToggleButton />
          <TooltipProvider delayDuration={200}>
            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant={isSettingsOpen ? 'secondary' : 'ghost'}
                  size="icon"
                  aria-label="Open settings"
                  aria-pressed={isSettingsOpen}
                  onClick={async () => {
                    void import('../../lib/telemetryClient').then(({ captureTelemetry }) => {
                      captureTelemetry('toolbar_settings_clicked');
                    });
                    onToggleSettings();
                  }}
                  className="h-8 w-8 text-muted-foreground hover:bg-background/80"
                >
                  <SettingsIcon className="h-4 w-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom" className="text-xs font-medium">
                <div className="flex flex-col gap-1">
                  <span>Open settings</span>
                  <span className="flex items-center gap-1 text-muted-foreground">
                    <Command className="h-3 w-3" aria-hidden="true" />
                    <span>,</span>
                  </span>
                </div>
              </TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </header>
    </>
  );
};

export default Titlebar;
