<script lang="ts">
  import { onMount } from 'svelte'

  type BindingState = {
    key: string
    tag_id: string
    value: unknown | null
    quality: string | null
    sequence: number | null
  }

  type ObjectState = {
    object_id: string
    svg_asset_id: string
    bindings: BindingState[]
  }

  type ScreenProjection = {
    screen_id: string
    project_id: string
    object_states: ObjectState[]
  }

  type CommandResponse = {
    command?: {
      status?: string
    }
    driver_response?: {
      accepted?: boolean
      message?: string
    }
    error?: string
  }

  let projection: ScreenProjection | null = null
  let loading = true
  let errorMessage = ''
  let commandMessage = ''
  let lastUpdated = ''

  onMount(() => {
    void loadProjection()
  })

  async function loadProjection() {
    loading = true
    errorMessage = ''

    try {
      const response = await fetch('/api/v1/screens/projection', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: '{}',
      })

      if (!response.ok) {
        throw new Error(`projection ${response.status}`)
      }

      projection = (await response.json()) as ScreenProjection
      lastUpdated = new Date().toLocaleTimeString()
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'projection unavailable'
    } finally {
      loading = false
    }
  }

  async function writeRunning(value: boolean) {
    commandMessage = ''
    errorMessage = ''

    const commandId = `ui-${Date.now()}`
    try {
      const response = await fetch('/api/v1/control-commands', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          command_id: commandId,
          idempotency_key: commandId,
          user_id: 'runtime-ui',
          tag_id: 'mock.running.001',
          requested_value: value,
          status: 'Requested',
          requested_at: new Date().toISOString(),
          timeout_ms: 1000,
        }),
      })
      const body = (await response.json()) as CommandResponse

      if (!response.ok) {
        throw new Error(body.error ?? `command ${response.status}`)
      }

      commandMessage = `command ${body.command?.status ?? 'accepted'}`
      await loadProjection()
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'command failed'
    }
  }

  $: runningBinding = findBinding('mock.running.001')
  $: temperatureBinding = findBinding('mock.temperature.001')
  $: isRunning = runningBinding?.value === true
  $: temperature = typeof temperatureBinding?.value === 'number'
    ? temperatureBinding.value.toFixed(1)
    : formatValue(temperatureBinding?.value)
  $: objectCount = projection?.object_states.length ?? 0
  $: bindingCount =
    projection?.object_states.reduce((total, object) => total + object.bindings.length, 0) ?? 0
  $: staleCount =
    projection?.object_states
      .flatMap((object) => object.bindings)
      .filter((binding) => binding.quality !== 'Simulated').length ?? 0

  function findBinding(tagId: string): BindingState | undefined {
    return projection?.object_states
      .flatMap((object) => object.bindings)
      .find((binding) => binding.tag_id === tagId)
  }

  function formatValue(value: unknown): string {
    if (value === null || value === undefined) return '-'
    if (typeof value === 'boolean') return value ? 'true' : 'false'
    if (typeof value === 'number') return value.toString()
    if (typeof value === 'string') return value
    return JSON.stringify(value)
  }
</script>

<main class="runtime-shell">
  <header class="topbar">
    <div>
      <p class="eyebrow">SCADA Runtime</p>
      <h1>Runtime Monitor</h1>
    </div>
    <div class="topbar-actions">
      <span class:online={!errorMessage} class="status-pill">
        {errorMessage ? 'Offline' : 'Online'}
      </span>
      <button class="icon-button" type="button" aria-label="Refresh projection" on:click={loadProjection}>
        ↻
      </button>
    </div>
  </header>

  {#if errorMessage}
    <section class="notice" aria-live="polite">{errorMessage}</section>
  {/if}

  <section class="summary-grid" aria-label="Runtime summary">
    <div>
      <span>Screen</span>
      <strong>{projection?.screen_id ?? '-'}</strong>
    </div>
    <div>
      <span>Objects</span>
      <strong>{objectCount}</strong>
    </div>
    <div>
      <span>Bindings</span>
      <strong>{bindingCount}</strong>
    </div>
    <div>
      <span>Alerts</span>
      <strong>{staleCount}</strong>
    </div>
  </section>

  <section class="process-view" aria-busy={loading}>
    <div class="process-stage">
      <div class:running={isRunning} class="pump-asset">
        <div class="pump-body"></div>
        <div class="pump-motor"></div>
        <div class="flow-line"></div>
      </div>
      <div class="value-stack">
        <span>mock.temperature.001</span>
        <strong>{temperature} °C</strong>
        <small>{temperatureBinding?.quality ?? 'Missing'} · seq {temperatureBinding?.sequence ?? '-'}</small>
      </div>
      <div class="state-stack">
        <span>mock.running.001</span>
        <strong>{isRunning ? 'Running' : 'Stopped'}</strong>
        <small>{runningBinding?.quality ?? 'Missing'} · seq {runningBinding?.sequence ?? '-'}</small>
      </div>
    </div>

    <aside class="control-panel">
      <h2>Control</h2>
      <div class="segmented">
        <button type="button" class:active={isRunning} on:click={() => writeRunning(true)}>
          Start
        </button>
        <button type="button" class:active={!isRunning} on:click={() => writeRunning(false)}>
          Stop
        </button>
      </div>
      <p class="command-state">{commandMessage || `Updated ${lastUpdated || '-'}`}</p>
    </aside>
  </section>

  <section class="binding-table" aria-label="Bindings">
    <header>
      <h2>Bindings</h2>
      <span>{projection?.project_id ?? 'demo'}</span>
    </header>
    <div class="table-head">
      <span>Object</span>
      <span>Key</span>
      <span>Tag</span>
      <span>Value</span>
      <span>Quality</span>
      <span>Seq</span>
    </div>
    {#each projection?.object_states ?? [] as object}
      {#each object.bindings as binding}
        <div class="table-row">
          <span>{object.object_id}</span>
          <span>{binding.key}</span>
          <span>{binding.tag_id}</span>
          <span>{formatValue(binding.value)}</span>
          <span>{binding.quality ?? 'Missing'}</span>
          <span>{binding.sequence ?? '-'}</span>
        </div>
      {/each}
    {/each}
  </section>
</main>
