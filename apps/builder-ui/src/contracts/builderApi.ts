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

export type ScreenModifyRuleCondition = {
  op?: string
  value?: unknown
  min?: number
  max?: number
  values?: unknown[]
  all?: ScreenModifyRuleCondition[]
  any?: ScreenModifyRuleCondition[]
}

export type ScreenModifyRuleDefinition = {
  property: string
  binding_key: string
  true_value: string
  false_value: string
  condition: ScreenModifyRuleCondition
}

export type SerializedScreenObject = {
  object_id: string
  svg_asset_id: string
  x: number
  y: number
  width: number
  height: number
  tag_bindings: Record<string, string>
  modify_rules: ScreenModifyRuleDefinition[]
}

export type SerializedScreenDefinition = {
  schema_version: string
  screen_id: string
  project_id: string
  name: string
  canvas_width: number
  canvas_height: number
  objects: SerializedScreenObject[]
}

export type BuilderSaveScreenResponse = {
  saved_path: string
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

export function isSerializedScreenDefinition(value: unknown): value is SerializedScreenDefinition {
  if (!value || typeof value !== 'object') {
    return false
  }

  const record = value as Record<string, unknown>
  return (
    typeof record.schema_version === 'string' &&
    typeof record.screen_id === 'string' &&
    typeof record.project_id === 'string' &&
    typeof record.name === 'string' &&
    typeof record.canvas_width === 'number' &&
    typeof record.canvas_height === 'number' &&
    Array.isArray(record.objects)
  )
}

export function isBuilderSaveScreenResponse(value: unknown): value is BuilderSaveScreenResponse {
  if (!value || typeof value !== 'object') {
    return false
  }

  const record = value as Record<string, unknown>
  return typeof record.saved_path === 'string'
}
