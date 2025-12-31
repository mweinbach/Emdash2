// Telemetry has been removed; keep a no-op to avoid churn in call sites.
export function captureTelemetry(_event: string, _properties?: Record<string, any>): void {}
