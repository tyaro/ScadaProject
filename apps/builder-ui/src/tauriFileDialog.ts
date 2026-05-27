export type PickScreenRelativePathResult = {
  cancelled: boolean
  relative_path: string | null
}

type TauriInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>

type TauriApi = {
  core?: {
    invoke?: TauriInvoke
  }
  invoke?: TauriInvoke
}

const PICK_SCREEN_RELATIVE_PATH_COMMAND = 'pick_screen_relative_path'

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
