export async function logPlanEvent(taskPath: string, message: string) {
  try {
    const ts = new Date().toISOString();
    const line = `[${ts}] ${message}\n`;
    const fp = `${taskPath}/.emdash/plan.log`;
    await (window as any).desktopAPI.debugAppendLog(fp, line, { reset: false });
  } catch {}
}
