import React from 'react';
import { Button } from './ui/button';
import { Switch } from './ui/switch';
import { CONTEXT7_INTEGRATION } from '../mcp/context7';
import context7Logo from '../../assets/images/context7.png';

const Context7SettingsCard: React.FC = () => {
  const [enabled, setEnabled] = React.useState<boolean>(false);
  const [busy, setBusy] = React.useState<boolean>(false);

  const refresh = React.useCallback(async () => {
    try {
      const res = await window.desktopAPI.getSettings();
      if (res?.success && res.settings) {
        const flag = Boolean(res.settings.mcp?.context7?.enabled);
        setEnabled(flag);
      }
    } catch {}
  }, []);

  React.useEffect(() => {
    void refresh();
  }, [refresh]);

  const onToggle = async (next: boolean) => {
    setBusy(true);
    try {
      await window.desktopAPI.updateSettings({ mcp: { context7: { enabled: next } } as any });
      setEnabled(next);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="grid gap-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex items-start gap-3">
          <img
            src={context7Logo}
            alt="Context7"
            className="mt-0.5 h-6 w-6 rounded-sm border border-border/50 object-contain"
          />
          <div className="space-y-1 text-xs text-muted-foreground">
            <p>
              Enable {CONTEXT7_INTEGRATION.label} to enrich prompts with up‑to‑date library docs.
            </p>
            <p>
              Recommended: add a rule in your client to auto‑invoke Context7 for code questions.
            </p>
          </div>
        </div>
        <Switch
          checked={enabled}
          onCheckedChange={onToggle}
          disabled={busy}
          aria-label="Enable Context7 MCP"
        />
      </div>

      <div className="flex items-start gap-2 rounded-md border border-border/60 bg-muted/20 px-3 py-2">
        <div className="text-[11px] leading-snug text-muted-foreground">
          <p>
            You must configure Context7 MCP in your coding agent (Codex, Claude Code, Cursor, etc.)
            before using it in Emdash2.
          </p>
          <p className="mt-1">
            After setup, enabling Context7 here lets Emdash2 invoke it in your terminal sessions so
            agents can fetch up‑to‑date docs when needed. Use the Docs link for per‑agent setup.
          </p>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          variant="link"
          size="sm"
          className="h-auto p-0 text-xs text-muted-foreground underline-offset-2 hover:text-foreground hover:underline"
          onClick={() => window.desktopAPI.openExternal(CONTEXT7_INTEGRATION.docsUrl)}
        >
          Docs ↗
        </Button>
      </div>
    </div>
  );
};

export default Context7SettingsCard;
