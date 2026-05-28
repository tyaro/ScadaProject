<script lang="ts">
  import { onMount, tick } from 'svelte'
  import {
    isBuilderSaveScreenResponse,
    isBuilderErrorMapResponse,
    isBuilderErrorResponse,
    isSerializedScreenDefinition,
    type BuilderSaveScreenAsRequest,
    type BuilderErrorMapRequest,
    type BuilderErrorMapResponse,
    type SerializedScreenDefinition,
    type SerializedScreenObject,
    type ScreenModifyRuleCondition,
  } from './contracts/builderApi'
  import {
    hasTauriFileDialogBridge,
    pickScreenRelativePath,
    readSuperviseLogSummary,
    type SuperviseLogSummary,
  } from './tauriFileDialog'

  type ParsedTargetPath = {
    objectId: string | null
    property: string | null
    valid: boolean
  }

  type ModifyRuleForm = {
    property: string
    bindingKey: string
    trueValue: string
    falseValue: string
    conditionOp: string
    conditionValue: string
    conditionMin: string
    conditionMax: string
    conditionValues: string
  }

  type ScreenObjectForm = {
    objectId: string
    svgAssetId: string
    x: number
    y: number
    width: number
    height: number
    tagBindings: Record<string, string>
    modifyRules: ModifyRuleForm[]
  }

  type RuntimeProjectionSummary = {
    screen_id: string
    project_id: string
    object_states: unknown[]
  }

  type SupervisePolicyPreset = 'strict' | 'balanced' | 'observe' | 'custom'
  type SupervisePolicySource = 'url' | 'localStorage' | 'default'

  const sampleKnownError =
    "code=MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX path=object=pump-001 property=color detail=op 'between' requires min and max"
  const sampleUnknownError =
    "code=SOME_NEW_ERROR path=object=valve-002 property=text detail=unexpected runtime validation state"
  const supervisePolicyStorageKey = 'scada.builder.supervise.policy.v1'
  const supervisePolicyQueryKey = 'supervisePolicy'
  const propertyOptions = ['visible', 'color', 'text']
  const conditionOpOptions = ['eq', 'ne', 'gt', 'gte', 'lt', 'lte', 'between', 'in', 'any', 'all']
  const screenIdPattern = /^[A-Za-z0-9_-]+$/
  const saveAsPathPattern = /^config\/screens\/[A-Za-z0-9_\/-]+\.screen\.json$/
  const initialScreenObjects: ScreenObjectForm[] = [
    {
      objectId: 'pump-001',
      svgAssetId: 'pump-symbol',
      x: 80,
      y: 120,
      width: 120,
      height: 120,
      tagBindings: {
        state: 'mock.running.001',
        value: 'mock.temperature.001',
      },
      modifyRules: [
        {
          property: 'visible',
          bindingKey: 'state',
          trueValue: 'true',
          falseValue: 'false',
          conditionOp: 'eq',
          conditionValue: 'true',
          conditionMin: '',
          conditionMax: '',
          conditionValues: '',
        },
        {
          property: 'color',
          bindingKey: 'value',
          trueValue: '#cc3333',
          falseValue: '#3cb371',
          conditionOp: 'any',
          conditionValue: '',
          conditionMin: '',
          conditionMax: '',
          conditionValues: 'lt:18,gt:28',
        },
      ],
    },
    {
      objectId: 'label-001',
      svgAssetId: 'text-label',
      x: 240,
      y: 140,
      width: 280,
      height: 48,
      tagBindings: {
        value: 'mock.temperature.001',
      },
      modifyRules: [
        {
          property: 'text',
          bindingKey: 'value',
          trueValue: 'NORMAL',
          falseValue: 'CHECK',
          conditionOp: 'all',
          conditionValue: '',
          conditionMin: '',
          conditionMax: '',
          conditionValues: 'gte:20,lte:26',
        },
      ],
    },
  ]

  let inputError = $state(sampleKnownError)
  let loading = $state(false)
  let apiError = $state('')
  let response = $state<BuilderErrorMapResponse | null>(null)
  let parsedTarget = $state<ParsedTargetPath>({ objectId: null, property: null, valid: false })
  let focusStatus = $state('No parsed target yet')
  let ioStatus = $state('No file loaded yet')
  let screenObjects = $state<ScreenObjectForm[]>(structuredClone(initialScreenObjects))
  let selectedObjectIndex = $state(0)
  let selectedRuleIndex = $state(0)
  let schemaVersion = $state('1.0.0')
  let screenId = $state('mock-main')
  let projectId = $state('demo')
  let screenName = $state('Mock Main Screen')
  let canvasWidth = $state(1280)
  let canvasHeight = $state(720)
  let saveAsRelativePath = $state('config/screens/mock-main.screen.json')
  let runtimePreviewStatus = $state('No runtime preview yet')
  let superviseLogDir = $state('/tmp/scada-supervise-log')
  let superviseSummary = $state<SuperviseLogSummary | null>(null)
  let superviseSummaryStatus = $state('No supervise log summary loaded')
  let superviseSummaryLoading = $state(false)
  let supervisePolicyPreset = $state<SupervisePolicyPreset>('strict')
  let superviseFailOnParseError = $state(true)
  let superviseFailOnServiceExit = $state(true)
  let supervisePolicyHydrating = false
  let supervisePolicyUrlOverrideActive = $state(false)
  let supervisePolicySource = $state<SupervisePolicySource>('default')
  let supervisePolicyAppliedAt = $state<string | null>(null)
  let objectField: HTMLInputElement | null = null
  let propertyField: HTMLSelectElement | null = null
  let jsonFileInput: HTMLInputElement | null = null

  const emptyRule = (): ModifyRuleForm => ({
    property: 'visible',
    bindingKey: 'value',
    trueValue: '',
    falseValue: '',
    conditionOp: 'eq',
    conditionValue: '',
    conditionMin: '',
    conditionMax: '',
    conditionValues: '',
  })

  const emptyObject = (objectId: string): ScreenObjectForm => ({
    objectId,
    svgAssetId: 'draft-symbol',
    x: 0,
    y: 0,
    width: 120,
    height: 60,
    tagBindings: {
      value: '',
    },
    modifyRules: [emptyRule()],
  })

  function parsePrimitive(raw: string): unknown {
    const text = raw.trim()
    if (text === '') {
      return ''
    }
    if (text === 'true') {
      return true
    }
    if (text === 'false') {
      return false
    }

    const maybeNumber = Number(text)
    if (!Number.isNaN(maybeNumber)) {
      return maybeNumber
    }

    return text
  }

  function parseConditionClause(clause: string): ScreenModifyRuleCondition | null {
    const trimmed = clause.trim()
    if (trimmed === '') {
      return null
    }

    const [op, ...rest] = trimmed.split(':')
    const valueToken = rest.join(':')
    if (!op || valueToken.trim() === '') {
      return null
    }

    return {
      op: op.trim(),
      value: parsePrimitive(valueToken),
    }
  }

  function buildCondition(rule: ModifyRuleForm): ScreenModifyRuleCondition {
    const op = rule.conditionOp

    if (usesSingleValueOp(op)) {
      return {
        op,
        value: parsePrimitive(rule.conditionValue),
      }
    }

    if (op === 'between') {
      return {
        op,
        min: Number(rule.conditionMin),
        max: Number(rule.conditionMax),
      }
    }

    if (op === 'in') {
      return {
        op,
        values: rule.conditionValues
          .split(',')
          .map((item) => item.trim())
          .filter((item) => item !== '')
          .map(parsePrimitive),
      }
    }

    const clauses = rule.conditionValues
      .split(',')
      .map(parseConditionClause)
      .filter((clause): clause is ScreenModifyRuleCondition => clause !== null)

    if (op === 'all') {
      return { all: clauses }
    }

    if (op === 'any') {
      return { any: clauses }
    }

    return { op: 'eq', value: '' }
  }

  function buildScreenDefinition(): SerializedScreenDefinition {
    return {
      schema_version: schemaVersion,
      screen_id: screenId,
      project_id: projectId,
      name: screenName,
      canvas_width: canvasWidth,
      canvas_height: canvasHeight,
      objects: screenObjects.map((object) => ({
        object_id: object.objectId,
        svg_asset_id: object.svgAssetId,
        x: object.x,
        y: object.y,
        width: object.width,
        height: object.height,
        tag_bindings: object.tagBindings,
        modify_rules: object.modifyRules.map((rule) => ({
          property: rule.property,
          binding_key: rule.bindingKey,
          true_value: rule.trueValue,
          false_value: rule.falseValue,
          condition: buildCondition(rule),
        })),
      })),
    }
  }

  function primitiveToString(value: unknown): string {
    if (value === null || value === undefined) {
      return ''
    }
    if (typeof value === 'string') {
      return value
    }
    if (typeof value === 'number' || typeof value === 'boolean') {
      return String(value)
    }
    return JSON.stringify(value)
  }

  function parseFormCondition(condition: ScreenModifyRuleCondition | undefined): Omit<ModifyRuleForm, 'property' | 'bindingKey' | 'trueValue' | 'falseValue'> {
    if (!condition) {
      return {
        conditionOp: 'eq',
        conditionValue: '',
        conditionMin: '',
        conditionMax: '',
        conditionValues: '',
      }
    }

    if (condition.op && usesSingleValueOp(condition.op)) {
      return {
        conditionOp: condition.op,
        conditionValue: primitiveToString(condition.value),
        conditionMin: '',
        conditionMax: '',
        conditionValues: '',
      }
    }

    if (condition.op === 'between') {
      return {
        conditionOp: 'between',
        conditionValue: '',
        conditionMin: primitiveToString(condition.min),
        conditionMax: primitiveToString(condition.max),
        conditionValues: '',
      }
    }

    if (condition.op === 'in') {
      return {
        conditionOp: 'in',
        conditionValue: '',
        conditionMin: '',
        conditionMax: '',
        conditionValues: (condition.values ?? []).map(primitiveToString).join(','),
      }
    }

    if (condition.all) {
      return {
        conditionOp: 'all',
        conditionValue: '',
        conditionMin: '',
        conditionMax: '',
        conditionValues: condition.all
          .map((clause) => {
            if (!clause.op) {
              return ''
            }
            return `${clause.op}:${primitiveToString(clause.value)}`
          })
          .filter((text) => text !== '')
          .join(','),
      }
    }

    if (condition.any) {
      return {
        conditionOp: 'any',
        conditionValue: '',
        conditionMin: '',
        conditionMax: '',
        conditionValues: condition.any
          .map((clause) => {
            if (!clause.op) {
              return ''
            }
            return `${clause.op}:${primitiveToString(clause.value)}`
          })
          .filter((text) => text !== '')
          .join(','),
      }
    }

    return {
      conditionOp: 'eq',
      conditionValue: '',
      conditionMin: '',
      conditionMax: '',
      conditionValues: '',
    }
  }

  function mapLoadedObject(object: SerializedScreenObject): ScreenObjectForm {
    return {
      objectId: object.object_id,
      svgAssetId: object.svg_asset_id,
      x: object.x,
      y: object.y,
      width: object.width,
      height: object.height,
      tagBindings: object.tag_bindings ?? {},
      modifyRules: (object.modify_rules ?? []).map((rule) => ({
        property: rule.property,
        bindingKey: rule.binding_key,
        trueValue: rule.true_value,
        falseValue: rule.false_value,
        ...parseFormCondition(rule.condition),
      })),
    }
  }

  function downloadScreenDefinition() {
    const screen = buildScreenDefinition()
    const blob = new Blob([JSON.stringify(screen, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)

    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = `${screen.screen_id}.screen.json`
    anchor.click()
    URL.revokeObjectURL(url)
    ioStatus = `Downloaded ${anchor.download}`
  }

  async function loadScreenDefinitionFile(event: Event) {
    const target = event.target as HTMLInputElement
    const file = target.files?.[0]
    if (!file) {
      return
    }

    try {
      const text = await file.text()
      const parsed = JSON.parse(text) as unknown
      if (!isSerializedScreenDefinition(parsed)) {
        throw new Error('invalid screen-definition shape')
      }

      applyLoadedScreenDefinition(parsed)
      ioStatus = `Loaded ${file.name}`
    } catch (error) {
      ioStatus = error instanceof Error ? `Load failed: ${error.message}` : 'Load failed'
    } finally {
      target.value = ''
    }
  }

  function openLoadDialog() {
    jsonFileInput?.click()
  }

  function isScreenIdValid(value: string): boolean {
    const trimmed = value.trim()
    return trimmed !== '' && screenIdPattern.test(trimmed)
  }

  function isSaveAsPathValid(value: string): boolean {
    const trimmed = value.trim()
    if (trimmed === '') {
      return false
    }
    if (!saveAsPathPattern.test(trimmed)) {
      return false
    }
    if (trimmed.includes('..') || trimmed.includes('//') || trimmed.includes('\\')) {
      return false
    }
    return true
  }

  function normalizedSaveAsPathOrError(): string | null {
    const trimmed = saveAsRelativePath.trim()
    if (trimmed === '') {
      ioStatus = 'Project save-as failed: relative_path is required'
      return null
    }

    if (!isSaveAsPathValid(trimmed)) {
      ioStatus = 'Project save-as failed: relative_path must match config/screens/*.screen.json'
      return null
    }

    if (trimmed !== saveAsRelativePath) {
      saveAsRelativePath = trimmed
    }

    return trimmed
  }

  function normalizedScreenIdOrError(action: 'load' | 'save'): string | null {
    const trimmed = screenId.trim()
    if (trimmed === '') {
      ioStatus = `Project ${action} failed: screen_id is required`
      return null
    }

    if (!screenIdPattern.test(trimmed)) {
      ioStatus = 'Project I/O failed: screen_id must match [A-Za-z0-9_-]'
      return null
    }

    if (trimmed !== screenId) {
      screenId = trimmed
    }

    return trimmed
  }

  function projectScreenPath(value: string): string {
    const trimmed = value.trim()
    if (trimmed === '') {
      return 'config/screens/<screen_id>.screen.json'
    }
    return `config/screens/${trimmed}.screen.json`
  }

  function previewRequestPath(): string | null {
    const preferred = saveAsRelativePath.trim()
    if (isSaveAsPathValid(preferred)) {
      return preferred
    }

    const fallback = projectScreenPath(screenId)
    if (fallback.includes('<screen_id>')) {
      return null
    }

    return fallback
  }

  function isRuntimeProjectionSummary(value: unknown): value is RuntimeProjectionSummary {
    if (!value || typeof value !== 'object') {
      return false
    }

    const record = value as Record<string, unknown>
    return (
      typeof record.screen_id === 'string' &&
      typeof record.project_id === 'string' &&
      Array.isArray(record.object_states)
    )
  }

  function syncSaveAsPathToScreenId() {
    const nextPath = projectScreenPath(screenId)
    if (!nextPath.includes('<screen_id>')) {
      saveAsRelativePath = nextPath
    }
  }

  async function pickSaveAsPathViaTauri() {
    const initialPath = previewRequestPath() ?? projectScreenPath(screenId)
    try {
      const picked = await pickScreenRelativePath(initialPath)
      if (picked === null) {
        ioStatus = 'Tauri file dialog unavailable: running in web mode'
        return
      }

      if (picked.cancelled) {
        ioStatus = 'File selection cancelled'
        return
      }

      if (!picked.relative_path || !isSaveAsPathValid(picked.relative_path)) {
        throw new Error('invalid relative_path from tauri picker')
      }

      saveAsRelativePath = picked.relative_path
      ioStatus = `Selected ${picked.relative_path}`
    } catch (error) {
      ioStatus = error instanceof Error ? `Path selection failed: ${error.message}` : 'Path selection failed'
    }
  }

  async function loadSuperviseLogSummaryViaTauri() {
    const logDir = superviseLogDir.trim()
    if (logDir === '') {
      superviseSummaryStatus = 'Supervise summary failed: log directory is required'
      return
    }

    superviseSummaryLoading = true
    try {
      const summary = await readSuperviseLogSummary(logDir)
      if (!summary) {
        superviseSummaryStatus = 'Tauri bridge unavailable: running in web mode'
        superviseSummary = null
        return
      }

      superviseSummary = summary
      superviseSummaryStatus = `Loaded summary from ${logDir}`
    } catch (error) {
      superviseSummary = null
      superviseSummaryStatus =
        error instanceof Error
          ? `Supervise summary failed: ${error.message}`
          : 'Supervise summary failed'
    } finally {
      superviseSummaryLoading = false
    }
  }

  function applyLoadedScreenDefinition(parsed: SerializedScreenDefinition) {
    schemaVersion = parsed.schema_version
    screenId = parsed.screen_id
    projectId = parsed.project_id
    screenName = parsed.name
    canvasWidth = parsed.canvas_width
    canvasHeight = parsed.canvas_height

    const mappedObjects = parsed.objects.map((item) => mapLoadedObject(item as SerializedScreenObject))
    screenObjects = mappedObjects.length > 0 ? mappedObjects : [emptyObject('draft-object')]
    selectedObjectIndex = 0
    selectedRuleIndex = 0
  }

  async function loadScreenDefinitionFromProject() {
    const targetScreenId = normalizedScreenIdOrError('load')
    if (!targetScreenId) {
      return
    }

    ioStatus = `Loading ${targetScreenId} from project...`
    try {
      const httpResponse = await fetch(`/api/v1/screens/${encodeURIComponent(targetScreenId)}`)
      if (!httpResponse.ok) {
        const errorBody = await parseError(httpResponse)
        throw new Error(errorBody)
      }

      const body = await httpResponse.json()
      if (!isSerializedScreenDefinition(body)) {
        throw new Error('builder api returned invalid screen-definition contract')
      }

      applyLoadedScreenDefinition(body)
      ioStatus = `Loaded ${body.screen_id} from project`
    } catch (error) {
      ioStatus = error instanceof Error ? `Project load failed: ${error.message}` : 'Project load failed'
    }
  }

  async function saveScreenDefinitionToProject() {
    const targetScreenId = normalizedScreenIdOrError('save')
    if (!targetScreenId) {
      return
    }

    ioStatus = `Saving ${targetScreenId} to project...`
    try {
      const payload = buildScreenDefinition()
      payload.screen_id = targetScreenId
      const httpResponse = await fetch(`/api/v1/screens/${encodeURIComponent(payload.screen_id)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })

      if (!httpResponse.ok) {
        const errorBody = await parseError(httpResponse)
        throw new Error(errorBody)
      }

      const body = await httpResponse.json()
      if (!isBuilderSaveScreenResponse(body)) {
        throw new Error('builder api returned invalid save response contract')
      }

      ioStatus = `Saved to ${body.saved_path}`
    } catch (error) {
      ioStatus = error instanceof Error ? `Project save failed: ${error.message}` : 'Project save failed'
    }
  }

  async function saveScreenDefinitionAsProjectPath() {
    const targetScreenId = normalizedScreenIdOrError('save')
    if (!targetScreenId) {
      return
    }

    const relativePath = normalizedSaveAsPathOrError()
    if (!relativePath) {
      return
    }

    ioStatus = `Saving ${targetScreenId} to ${relativePath}...`
    try {
      const payload: BuilderSaveScreenAsRequest = {
        relative_path: relativePath,
        screen: {
          ...buildScreenDefinition(),
          screen_id: targetScreenId,
        },
      }
      const httpResponse = await fetch('/api/v1/screens/save-as', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })

      if (!httpResponse.ok) {
        const errorBody = await parseError(httpResponse)
        throw new Error(errorBody)
      }

      const body = await httpResponse.json()
      if (!isBuilderSaveScreenResponse(body)) {
        throw new Error('builder api returned invalid save-as response contract')
      }

      ioStatus = `Saved to ${body.saved_path}`
    } catch (error) {
      ioStatus = error instanceof Error ? `Project save-as failed: ${error.message}` : 'Project save-as failed'
    }
  }

  async function previewScreenInRuntime() {
    const screenPath = previewRequestPath()
    if (!screenPath) {
      runtimePreviewStatus = 'Runtime preview failed: valid screen path is required'
      return
    }

    runtimePreviewStatus = `Previewing ${screenPath}...`
    try {
      const httpResponse = await fetch('/runtime-api/api/v1/screens/projection', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ screen_path: screenPath }),
      })

      if (!httpResponse.ok) {
        const errorBody = await parseError(httpResponse)
        throw new Error(errorBody)
      }

      const body = await httpResponse.json()
      if (!isRuntimeProjectionSummary(body)) {
        throw new Error('runtime api returned invalid projection contract')
      }

      runtimePreviewStatus = `Projection loaded: ${body.screen_id} (${body.object_states.length} objects)`
    } catch (error) {
      runtimePreviewStatus = error instanceof Error ? `Runtime preview failed: ${error.message}` : 'Runtime preview failed'
    }
  }

  function selectedObject(): ScreenObjectForm {
    return screenObjects[selectedObjectIndex]
  }

  function selectedRule(): ModifyRuleForm {
    const object = selectedObject()
    return object.modifyRules[selectedRuleIndex]
  }

  function usesSingleValueOp(op: string): boolean {
    return ['eq', 'ne', 'gt', 'gte', 'lt', 'lte'].includes(op)
  }

  function superviseSummaryIssueCount(summary: SuperviseLogSummary): number {
    return summary.parse_errors + summary.services.reduce((total, service) => total + service.exited + service.started_false, 0)
  }

  function superviseSummaryFlaggedIssueCount(summary: SuperviseLogSummary): number {
    let total = 0
    if (superviseFailOnParseError) {
      total += summary.parse_errors
    }
    if (superviseFailOnServiceExit) {
      total += summary.services.reduce((sum, service) => sum + service.exited + service.started_false, 0)
    }
    return total
  }

  function superviseSummaryHasIssues(summary: SuperviseLogSummary): boolean {
    return superviseSummaryFlaggedIssueCount(summary) > 0
  }

  function markSupervisePolicyApplied() {
    supervisePolicyAppliedAt = new Date().toISOString()
  }

  function persistSupervisePolicy() {
    try {
      localStorage.setItem(
        supervisePolicyStorageKey,
        JSON.stringify({
          fail_on_parse_error: superviseFailOnParseError,
          fail_on_service_exit: superviseFailOnServiceExit,
        })
      )
    } catch {
      // Ignore persistence failures in restricted browser contexts.
    }
  }

  function readSupervisePolicyFromQuery(): SupervisePolicyPreset | null {
    try {
      const params = new URLSearchParams(window.location.search)
      const raw = params.get(supervisePolicyQueryKey)
      if (raw === 'strict' || raw === 'balanced' || raw === 'observe') {
        return raw
      }
      return null
    } catch {
      return null
    }
  }

  function persistSupervisePolicyQuery() {
    try {
      const url = new URL(window.location.href)
      if (supervisePolicyPreset === 'custom') {
        url.searchParams.delete(supervisePolicyQueryKey)
        supervisePolicyUrlOverrideActive = false
        supervisePolicySource = 'localStorage'
      } else {
        url.searchParams.set(supervisePolicyQueryKey, supervisePolicyPreset)
        supervisePolicyUrlOverrideActive = true
        supervisePolicySource = 'url'
      }
      const nextUrl = `${url.pathname}${url.search}${url.hash}`
      window.history.replaceState({}, '', nextUrl)
    } catch {
      // Ignore URL update failures in restricted browser contexts.
    }
  }

  function restoreSupervisePolicy(): boolean {
    try {
      const raw = localStorage.getItem(supervisePolicyStorageKey)
      if (!raw) {
        return false
      }
      const parsed = JSON.parse(raw) as Record<string, unknown>
      if (
        typeof parsed.fail_on_parse_error !== 'boolean' ||
        typeof parsed.fail_on_service_exit !== 'boolean'
      ) {
        return false
      }

      supervisePolicyHydrating = true
      superviseFailOnParseError = parsed.fail_on_parse_error
      superviseFailOnServiceExit = parsed.fail_on_service_exit
      syncSupervisePolicyPresetFromFlags()
      supervisePolicyHydrating = false
      supervisePolicySource = 'localStorage'
      return true
    } catch {
      supervisePolicyHydrating = false
      return false
    }
  }

  function applySupervisePolicyPreset(preset: SupervisePolicyPreset) {
    if (preset === 'custom') {
      return
    }

    supervisePolicyPreset = preset
    if (preset === 'strict') {
      superviseFailOnParseError = true
      superviseFailOnServiceExit = true
      markSupervisePolicyApplied()
      if (!supervisePolicyHydrating) {
        persistSupervisePolicy()
        persistSupervisePolicyQuery()
      }
      return
    }

    if (preset === 'balanced') {
      superviseFailOnParseError = false
      superviseFailOnServiceExit = true
      markSupervisePolicyApplied()
      if (!supervisePolicyHydrating) {
        persistSupervisePolicy()
        persistSupervisePolicyQuery()
      }
      return
    }

    superviseFailOnParseError = false
    superviseFailOnServiceExit = false
    markSupervisePolicyApplied()
    if (!supervisePolicyHydrating) {
      persistSupervisePolicy()
      persistSupervisePolicyQuery()
    }
  }

  function syncSupervisePolicyPresetFromFlags() {
    if (superviseFailOnParseError && superviseFailOnServiceExit) {
      supervisePolicyPreset = 'strict'
      markSupervisePolicyApplied()
      if (!supervisePolicyHydrating) {
        persistSupervisePolicy()
        persistSupervisePolicyQuery()
      }
      return
    }

    if (!superviseFailOnParseError && superviseFailOnServiceExit) {
      supervisePolicyPreset = 'balanced'
      markSupervisePolicyApplied()
      if (!supervisePolicyHydrating) {
        persistSupervisePolicy()
        persistSupervisePolicyQuery()
      }
      return
    }

    if (!superviseFailOnParseError && !superviseFailOnServiceExit) {
      supervisePolicyPreset = 'observe'
      markSupervisePolicyApplied()
      if (!supervisePolicyHydrating) {
        persistSupervisePolicy()
        persistSupervisePolicyQuery()
      }
      return
    }

    supervisePolicyPreset = 'custom'
    markSupervisePolicyApplied()
    if (!supervisePolicyHydrating) {
      persistSupervisePolicy()
      persistSupervisePolicyQuery()
    }
  }

  onMount(() => {
    const policyFromQuery = readSupervisePolicyFromQuery()
    if (policyFromQuery) {
      supervisePolicyUrlOverrideActive = true
      supervisePolicySource = 'url'
      supervisePolicyHydrating = true
      applySupervisePolicyPreset(policyFromQuery)
      supervisePolicyHydrating = false
      persistSupervisePolicy()
      persistSupervisePolicyQuery()
      return
    }

    supervisePolicyUrlOverrideActive = false
    const restored = restoreSupervisePolicy()
    if (!restored) {
      supervisePolicySource = 'default'
      markSupervisePolicyApplied()
    }
  })

  function selectObject(index: number) {
    selectedObjectIndex = index
    selectedRuleIndex = 0
  }

  function addDraftObject(objectId: string): number {
    screenObjects = [
      ...screenObjects,
      emptyObject(objectId),
    ]
    return screenObjects.length - 1
  }

  function ensureRule(objectIndex: number, property: string | null): number {
    if (!property) {
      return 0
    }

    const ruleIndex = screenObjects[objectIndex].modifyRules.findIndex((rule) => rule.property === property)
    if (ruleIndex >= 0) {
      return ruleIndex
    }

    screenObjects[objectIndex].modifyRules = [
      ...screenObjects[objectIndex].modifyRules,
      {
        ...emptyRule(),
        property,
      },
    ]
    return screenObjects[objectIndex].modifyRules.length - 1
  }

  function parseTargetPath(path: string | null): ParsedTargetPath {
    if (!path) {
      return { objectId: null, property: null, valid: false }
    }

    const objectId = path.match(/(?:^|\s)object=([^\s]+)/)?.[1] ?? null
    const property = path.match(/(?:^|\s)property=([^\s]+)/)?.[1] ?? null

    return {
      objectId,
      property,
      valid: objectId !== null || property !== null,
    }
  }

  async function focusParsedTarget(path: string | null) {
    parsedTarget = parseTargetPath(path)

    if (!parsedTarget.valid) {
      focusStatus = 'Path could not be parsed into object/property tokens'
      return
    }

    await tick()

    let nextObjectIndex = parsedTarget.objectId
      ? screenObjects.findIndex((object) => object.objectId === parsedTarget.objectId)
      : selectedObjectIndex

    if (nextObjectIndex < 0) {
      nextObjectIndex = addDraftObject(parsedTarget.objectId ?? 'draft-object')
    }

    selectedObjectIndex = nextObjectIndex
    selectedRuleIndex = ensureRule(nextObjectIndex, parsedTarget.property)

    if (parsedTarget.objectId) {
      selectedObject().objectId = parsedTarget.objectId
    }

    if (parsedTarget.property) {
      selectedRule().property = parsedTarget.property
    }

    await tick()

    const targetField = parsedTarget.property ? propertyField : objectField
    targetField?.focus()
    focusStatus = parsedTarget.property
      ? `Focused property editor for ${parsedTarget.property}`
      : `Focused object editor for ${parsedTarget.objectId}`
  }

  async function mapError() {
    loading = true
    apiError = ''

    try {
      const payload: BuilderErrorMapRequest = { error: inputError }
      const httpResponse = await fetch('/api/v1/errors/map', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      })

      if (!httpResponse.ok) {
        const errorBody = await parseError(httpResponse)
        throw new Error(errorBody)
      }

      const mapped = await httpResponse.json()
      if (!isBuilderErrorMapResponse(mapped)) {
        throw new Error('builder api returned invalid map response contract')
      }

      response = mapped
      await focusParsedTarget(response.path)
    } catch (error) {
      response = null
      parsedTarget = { objectId: null, property: null, valid: false }
      focusStatus = 'No parsed target yet'
      apiError = error instanceof Error ? error.message : 'builder api unavailable'
    } finally {
      loading = false
    }
  }

  function useSample(value: string) {
    inputError = value
    apiError = ''
  }

  async function parseError(httpResponse: Response): Promise<string> {
    try {
      const body = await httpResponse.json()
      if (isBuilderErrorResponse(body)) {
        return body.error
      }
      return `builder api ${httpResponse.status}`
    } catch {
      return `builder api ${httpResponse.status}`
    }
  }
</script>

<main class="shell">
  <section class="hero">
    <div class="panel headline">
      <p class="eyebrow">SCADA Builder</p>
      <h1>Error Mapping Lab</h1>
      <p>
        Builder UI が本実装に入る前の最小画面です。Builder API の
        <strong>POST /api/v1/errors/map</strong> を呼び、condition エラーをユーザー向け表示に
        変換した結果を確認できます。
      </p>
    </div>

    <aside class="panel summary">
      <div class="summary-card">
        <span>Endpoint</span>
        <strong>/api/v1/errors/map</strong>
      </div>
      <div class="summary-card">
        <span>Known Codes</span>
        <strong>{response?.known_code ? 'Matched' : 'Fallback'}</strong>
      </div>
      <div class="summary-card">
        <span>UI Strategy</span>
        <strong>{response?.known_code ? 'Template' : 'Raw message'}</strong>
      </div>
    </aside>
  </section>

  <section class="workspace">
    <section class="panel editor">
      <header>
        <div>
          <h2>Raw Error</h2>
          <p>Builder API へ渡す生エラー文字列</p>
        </div>
        <span class="chip muted">JSON request</span>
      </header>

      <textarea bind:value={inputError} aria-label="Raw condition error"></textarea>

      <div class="project-io" data-testid="project-io-panel">
        <h3>Project Screen Save</h3>
        <div class="binding-grid two-up">
          <label class="field">
            <span>Screen ID</span>
            <input data-testid="screen-id-field" bind:value={screenId} type="text" />
          </label>
          <label class="field">
            <span>Project ID</span>
            <input data-testid="project-id-field" bind:value={projectId} type="text" />
          </label>
          <label class="field">
            <span>Screen Name</span>
            <input data-testid="screen-name-field" bind:value={screenName} type="text" />
          </label>
          <label class="field">
            <span>Schema Version</span>
            <input data-testid="schema-version-field" bind:value={schemaVersion} type="text" />
          </label>
          <label class="field">
            <span>Canvas Width</span>
            <input data-testid="canvas-width-field" bind:value={canvasWidth} type="number" min="1" />
          </label>
          <label class="field">
            <span>Canvas Height</span>
            <input data-testid="canvas-height-field" bind:value={canvasHeight} type="number" min="1" />
          </label>
        </div>
        <div class="result-row compact project-path-row" data-testid="project-path-row">
          <span>Project Path</span>
          <code>{projectScreenPath(screenId)}</code>
        </div>
        <label class="field">
          <span>Save As Relative Path</span>
          <input data-testid="save-as-path-field" bind:value={saveAsRelativePath} type="text" />
        </label>
        <div class="project-io-actions">
          <button class="secondary" type="button" data-testid="pick-save-as-path-button" onclick={pickSaveAsPathViaTauri}>
            {hasTauriFileDialogBridge() ? 'Pick via Tauri' : 'Pick via Tauri (web fallback)'}
          </button>
          <button class="secondary" type="button" data-testid="use-screen-id-path-button" onclick={syncSaveAsPathToScreenId}>
            Use screen_id path
          </button>
          <button
            class="secondary"
            type="button"
            data-testid="save-as-project-screen-button"
            onclick={saveScreenDefinitionAsProjectPath}
            disabled={!isScreenIdValid(screenId) || !isSaveAsPathValid(saveAsRelativePath)}
          >
            Save as path
          </button>
          <button
            class="secondary"
            type="button"
            data-testid="preview-runtime-button"
            onclick={previewScreenInRuntime}
            disabled={previewRequestPath() === null}
          >
            Preview in runtime
          </button>
        </div>
        <div class="result-row compact project-path-row" data-testid="runtime-preview-status-row">
          <span>Runtime Preview</span>
          <code>{runtimePreviewStatus}</code>
        </div>
        <div class="supervise-summary" data-testid="supervise-summary-panel">
          <div class="supervise-summary-header">
            <h4>Supervisor Log Summary</h4>
            {#if superviseSummary}
              <span
                class:good={!superviseSummaryHasIssues(superviseSummary)}
                class:warn={superviseSummaryHasIssues(superviseSummary)}
                class="chip"
                data-testid="supervise-summary-health-badge"
              >
                {superviseSummaryHasIssues(superviseSummary) ? 'Issue detected' : 'Healthy'}
              </span>
            {/if}
          </div>
          <label class="field">
            <span>Log Directory</span>
            <input data-testid="supervise-log-dir-field" bind:value={superviseLogDir} type="text" />
          </label>
          <label class="field">
            <span>Policy Preset</span>
            <select
              data-testid="supervise-policy-preset-select"
              bind:value={supervisePolicyPreset}
              onchange={() => applySupervisePolicyPreset(supervisePolicyPreset)}
            >
              <option value="strict">strict (parse + service exit)</option>
              <option value="balanced">balanced (service exit only)</option>
              <option value="observe">observe (no fail conditions)</option>
              <option value="custom" disabled>custom (manual)</option>
            </select>
          </label>
          <span class="chip muted" data-testid="supervise-policy-source-badge">
            Source: {supervisePolicySource}
          </span>
          <span class="chip muted" data-testid="supervise-policy-applied-at-badge">
            Applied: {supervisePolicyAppliedAt ?? 'n/a'}
          </span>
          {#if supervisePolicyUrlOverrideActive}
            <span class="chip warn" data-testid="supervise-policy-url-override-badge">URL override active</span>
          {/if}
          <div class="supervise-summary-toggles" data-testid="supervise-summary-policy-row">
            <label>
              <input
                data-testid="supervise-fail-parse-checkbox"
                bind:checked={superviseFailOnParseError}
                type="checkbox"
                onchange={syncSupervisePolicyPresetFromFlags}
              />
              fail on parse error
            </label>
            <label>
              <input
                data-testid="supervise-fail-service-exit-checkbox"
                bind:checked={superviseFailOnServiceExit}
                type="checkbox"
                onchange={syncSupervisePolicyPresetFromFlags}
              />
              fail on service exit
            </label>
          </div>
          <div class="project-io-actions">
            <button
              class="secondary"
              type="button"
              data-testid="load-supervise-summary-button"
              onclick={loadSuperviseLogSummaryViaTauri}
              disabled={superviseSummaryLoading}
            >
              {superviseSummaryLoading ? 'Loading summary...' : 'Load supervise summary'}
            </button>
          </div>
          <div class="result-row compact project-path-row" data-testid="supervise-summary-status-row">
            <span>Summary Status</span>
            <code>{superviseSummaryStatus}</code>
          </div>
          {#if superviseSummary}
            <div class="result-row compact project-path-row" data-testid="supervise-summary-counts-row">
              <span>Counts</span>
              <code>
                cycle={superviseSummary.cycle_summaries} final={superviseSummary.final_summaries} parse_errors={superviseSummary.parse_errors}
              </code>
            </div>
            <div class="result-row compact project-path-row" data-testid="supervise-summary-issues-row">
              <span>Health</span>
              <code>
                preset={supervisePolicyPreset} flagged_issues={superviseSummaryFlaggedIssueCount(superviseSummary)} raw_issues={superviseSummaryIssueCount(superviseSummary)} exited_total={superviseSummary.services.reduce((total, service) => total + service.exited, 0)} started_false_total={superviseSummary.services.reduce((total, service) => total + service.started_false, 0)}
              </code>
            </div>
            <ul class="hint-list" data-testid="supervise-summary-services-list">
              {#each superviseSummary.services as service}
                <li>
                  {service.service}: lines={service.lines} exited={service.exited} started_false={service.started_false}
                </li>
              {/each}
            </ul>
          {/if}
        </div>
        {#if !isScreenIdValid(screenId)}
          <p class="inline-error" data-testid="screen-id-validation-message">
            screen_id must match [A-Za-z0-9_-]
          </p>
        {/if}
        {#if !isSaveAsPathValid(saveAsRelativePath)}
          <p class="inline-error" data-testid="save-as-path-validation-message">
            relative_path must match config/screens/*.screen.json
          </p>
        {/if}
      </div>

      <div class="actions">
        <button class="primary" type="button" onclick={mapError} disabled={loading}>
          {loading ? 'Mapping...' : 'Map error'}
        </button>
        <button class="secondary" type="button" onclick={() => useSample(sampleKnownError)}>
          Use known sample
        </button>
        <button class="secondary" type="button" onclick={() => useSample(sampleUnknownError)}>
          Use unknown sample
        </button>
        <button class="secondary" type="button" data-testid="load-screen-button" onclick={openLoadDialog}>
          Load screen JSON
        </button>
        <button class="secondary" type="button" data-testid="download-screen-button" onclick={downloadScreenDefinition}>
          Download screen JSON
        </button>
        <button class="secondary" type="button" data-testid="load-project-screen-button" onclick={loadScreenDefinitionFromProject}>
          Load from project
        </button>
        <button
          class="secondary"
          type="button"
          data-testid="save-project-screen-button"
          onclick={saveScreenDefinitionToProject}
          disabled={!isScreenIdValid(screenId)}
        >
          Save to project
        </button>
      </div>

      <input
        bind:this={jsonFileInput}
        class="visually-hidden"
        data-testid="screen-file-input"
        type="file"
        accept="application/json,.json"
        onchange={loadScreenDefinitionFile}
      />

      {#if apiError}
        <div class="notice" role="alert">
          <strong>Builder API</strong>
          <span>{apiError}</span>
        </div>
      {/if}

      <div class="result-row io-status-row">
        <span>I/O Status</span>
        <code data-testid="io-status">{ioStatus}</code>
      </div>

      <ul class="hint-list">
        <li>`known_code=true` のときはテンプレート文言を表示します。</li>
        <li>未知コードは `user_message` に生エラーを残し、UIが最低限の原因を表示できます。</li>
        <li>`path` が `object` / `property` を含むときは、右側の Screen Object Editor で該当フォームへ移動します。</li>
      </ul>
    </section>

    <section class="panel preview" aria-live="polite">
      <header>
        <div>
          <h2>Mapped Result</h2>
          <p>Builder UI がそのまま利用する想定のJSON要約</p>
        </div>
        <span class:good={response?.known_code} class="chip" data-testid="known-code-badge">
          {response?.known_code ? 'Known code' : 'Fallback mode'}
        </span>
      </header>

      {#if response}
        <div class="result-grid">
          <div class="result-row" data-testid="user-message-row">
            <span>User Message</span>
            <strong>{response.user_message}</strong>
          </div>
          <div class="result-row">
            <span>Code</span>
            <code>{response.code ?? '-'}</code>
          </div>
          <div class="result-row" data-testid="path-row">
            <span>Path</span>
            <code>{response.path ?? '-'}</code>
          </div>
          <div class="result-row" data-testid="focus-status-row">
            <span>Editor focus</span>
            <code>{focusStatus}</code>
          </div>
          <div class="result-row">
            <span>Detail</span>
            <code>{response.detail ?? '-'}</code>
          </div>
          <div class="result-row">
            <span>Raw JSON contract</span>
            <code class="raw">{JSON.stringify(response)}</code>
          </div>
        </div>
      {:else}
        <div class="result-row">
          <span>Status</span>
          <strong>No mapped result yet</strong>
          <code>Call the endpoint to preview Builder UI error rendering.</code>
        </div>
      {/if}

      <div class="editor-stub" data-testid="editor-stub">
        <div class="editor-stub-header">
          <div>
            <h3>Screen Object Editor</h3>
            <p>`mock-main.screen.json` 相当の object と modify rule を編集する最小フォーム</p>
          </div>
          <span class:good={parsedTarget.valid} class="chip">
            {parsedTarget.valid ? 'Parsed target' : 'No target'}
          </span>
        </div>

        <div class="object-list" data-testid="object-list">
          {#each screenObjects as object, index}
            <button
              class:active={index === selectedObjectIndex}
              class="object-pill"
              data-testid={`object-pill-${object.objectId}`}
              type="button"
              onclick={() => selectObject(index)}
            >
              <strong>{object.objectId}</strong>
              <span>{object.svgAssetId}</span>
            </button>
          {/each}
        </div>

        <label class="field">
          <span>Object ID</span>
          <input
            bind:this={objectField}
            data-testid="object-field"
            type="text"
            bind:value={screenObjects[selectedObjectIndex].objectId}
          />
        </label>

        <label class="field">
          <span>SVG Asset</span>
          <input bind:value={screenObjects[selectedObjectIndex].svgAssetId} type="text" />
        </label>

        <div class="binding-grid two-up">
          <label class="field">
            <span>X</span>
            <input bind:value={screenObjects[selectedObjectIndex].x} type="number" />
          </label>
          <label class="field">
            <span>Y</span>
            <input bind:value={screenObjects[selectedObjectIndex].y} type="number" />
          </label>
          <label class="field">
            <span>Width</span>
            <input bind:value={screenObjects[selectedObjectIndex].width} type="number" />
          </label>
          <label class="field">
            <span>Height</span>
            <input bind:value={screenObjects[selectedObjectIndex].height} type="number" />
          </label>
        </div>

        <div class="binding-grid">
          {#each Object.entries(selectedObject().tagBindings) as [bindingKey, tagId]}
            <label class="field">
              <span>{bindingKey} binding</span>
              <input value={tagId} type="text" readonly />
            </label>
          {/each}
        </div>

        <label class="field">
          <span>Rule Property</span>
          <select
            bind:this={propertyField}
            data-testid="property-field"
            bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].property}
          >
            {#each propertyOptions as property}
              <option value={property}>{property}</option>
            {/each}
          </select>
        </label>

        <div class="binding-grid two-up">
          <label class="field">
            <span>Binding Key</span>
            <input bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].bindingKey} type="text" />
          </label>

          <label class="field">
            <span>True Value</span>
            <input bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].trueValue} type="text" />
          </label>

          <label class="field">
            <span>False Value</span>
            <input bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].falseValue} type="text" />
          </label>
        </div>

        <div class="condition-editor" data-testid="condition-editor">
          <h4>Condition</h4>

          <label class="field">
            <span>Operator</span>
            <select
              data-testid="condition-op-field"
              bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].conditionOp}
            >
              {#each conditionOpOptions as op}
                <option value={op}>{op}</option>
              {/each}
            </select>
          </label>

          {#if usesSingleValueOp(selectedRule().conditionOp)}
            <label class="field">
              <span>Value</span>
              <input
                data-testid="condition-value-field"
                bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].conditionValue}
                type="text"
              />
            </label>
          {:else if selectedRule().conditionOp === 'between'}
            <div class="binding-grid two-up">
              <label class="field">
                <span>Min</span>
                <input
                  data-testid="condition-min-field"
                  bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].conditionMin}
                  type="text"
                />
              </label>
              <label class="field">
                <span>Max</span>
                <input
                  data-testid="condition-max-field"
                  bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].conditionMax}
                  type="text"
                />
              </label>
            </div>
          {:else if selectedRule().conditionOp === 'in' || selectedRule().conditionOp === 'all' || selectedRule().conditionOp === 'any'}
            <label class="field">
              <span>Values / Clauses</span>
              <input
                data-testid="condition-values-field"
                bind:value={screenObjects[selectedObjectIndex].modifyRules[selectedRuleIndex].conditionValues}
                type="text"
              />
            </label>
          {/if}
        </div>

        <div class="result-row compact" data-testid="editor-selection-row">
          <span>Selected Rule</span>
          <code>{selectedObject().objectId} / {selectedRule().property}</code>
        </div>

        <div class="result-row compact" data-testid="screen-json-row">
          <span>Serialized Screen JSON</span>
          <textarea
            class="json-preview"
            data-testid="screen-json-preview"
            readonly
            value={JSON.stringify(buildScreenDefinition(), null, 2)}
          ></textarea>
        </div>
      </div>
    </section>
  </section>
</main>