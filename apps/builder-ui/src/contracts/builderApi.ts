export type BuilderErrorMapRequest = {
  error: string
}

export type BuilderErrorMapResponse = {
  code: string | null
  path: string | null
  detail: string | null
  user_message: string
  known_code: boolean
}

export type BuilderErrorResponse = {
  error: string
}

export function isBuilderErrorMapResponse(value: unknown): value is BuilderErrorMapResponse {
  if (!value || typeof value !== 'object') {
    return false
  }

  const record = value as Record<string, unknown>
  return (
    (record.code === null || typeof record.code === 'string') &&
    (record.path === null || typeof record.path === 'string') &&
    (record.detail === null || typeof record.detail === 'string') &&
    typeof record.user_message === 'string' &&
    typeof record.known_code === 'boolean'
  )
}

export function isBuilderErrorResponse(value: unknown): value is BuilderErrorResponse {
  if (!value || typeof value !== 'object') {
    return false
  }

  const record = value as Record<string, unknown>
  return typeof record.error === 'string'
}
