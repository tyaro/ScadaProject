export type PickScreenRelativePathResult = {
  cancelled: boolean
  relative_path: string | null
}

export type SuperviseLogSummaryService = {
  service: string
  lines: number
  exited: number
  started_false: number
}

export type SuperviseLogSummary = {
  cycle_summaries: number
  final_summaries: number
  parse_errors: number
  services: SuperviseLogSummaryService[]
}

type TauriInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>

type TauriApi = {
  core?: {
    invoke?: TauriInvoke
  }
  invoke?: TauriInvoke
}

const PICK_SCREEN_RELATIVE_PATH_COMMAND = 'pick_screen_relative_path'
const READ_SUPERVISE_LOG_SUMMARY_COMMAND = 'read_supervise_log_summary'

function readInvoke(): TauriInvoke | null {
  const globalTauri = (globalThis as { __TAURI__?: TauriApi }).__TAURI__
  if (!globalTauri) {
    return null
  }

  if (typeof globalTauri.core?.invoke === 'function') {
    return globalTauri.core.invoke
  }

  if (typeof globalTauri.invoke === 'function') {
    return globalTauri.invoke
  }

  return null
}

function isPickScreenRelativePathResult(value: unknown): value is PickScreenRelativePathResult {
  if (!value || typeof value !== 'object') {
    return false
  }

  const record = value as Record<string, unknown>
  return (
    typeof record.cancelled === 'boolean' &&
    (record.relative_path === null || typeof record.relative_path === 'string')
  )
}

function isSuperviseLogSummaryService(value: unknown): value is SuperviseLogSummaryService {
  if (!value || typeof value !== 'object') {
    return false
  }

  const record = value as Record<string, unknown>
  return (
    typeof record.service === 'string' &&
    typeof record.lines === 'number' &&
    typeof record.exited === 'number' &&
    typeof record.started_false === 'number'
  )
}

function isSuperviseLogSummary(value: unknown): value is SuperviseLogSummary {
  if (!value || typeof value !== 'object') {
    return false
  }

  const record = value as Record<string, unknown>
  if (
    typeof record.cycle_summaries !== 'number' ||
    typeof record.final_summaries !== 'number' ||
    typeof record.parse_errors !== 'number' ||
    !Array.isArray(record.services)
  ) {
    return false
  }

  return record.services.every(isSuperviseLogSummaryService)
}

export function hasTauriFileDialogBridge(): boolean {
  return readInvoke() !== null
}

export async function pickScreenRelativePath(
  initial_path: string
): Promise<PickScreenRelativePathResult | null> {
  const invoke = readInvoke()
  if (!invoke) {
    return null
  }

  const raw = await invoke(PICK_SCREEN_RELATIVE_PATH_COMMAND, { initial_path })
  if (!isPickScreenRelativePathResult(raw)) {
    throw new Error('tauri command returned invalid pick-screen-path response contract')
  }

  return raw
}

export async function readSuperviseLogSummary(log_dir: string): Promise<SuperviseLogSummary | null> {
  const invoke = readInvoke()
  if (!invoke) {
    return null
  }

  const raw = await invoke(READ_SUPERVISE_LOG_SUMMARY_COMMAND, { log_dir })
  if (!isSuperviseLogSummary(raw)) {
    throw new Error('tauri command returned invalid supervise-log-summary response contract')
  }

  return raw
}
