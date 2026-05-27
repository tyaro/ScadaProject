<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import mqtt, { type MqttClient } from 'mqtt'

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

  type ErrorResponse = {
    error?: string
  }

  type RuntimeTagValue = {
    tag_id: string
    value: unknown
    quality: string
    sequence: number
  }

  type RuntimeUiTestHook = {
    applyDelta: (topic: string, payload: RuntimeTagValue) => void
  }

  let projection: ScreenProjection | null = null
  let loading = true
  let errorMessage = ''
  let commandMessage = ''
  let lastUpdated = ''
  let mqttState = 'disconnected'
  let mqttMessage = ''
  let deltaCount = 0
  let mqttClient: MqttClient | null = null
  let mqttProjectId = ''

  const mqttUrl = import.meta.env.VITE_MQTT_URL ?? 'ws://127.0.0.1:8083/mqtt'

  onMount(() => {
    if (import.meta.env.DEV) {
      ;(window as typeof window & { __runtimeUiTestHook__?: RuntimeUiTestHook }).__runtimeUiTestHook__ = {
        applyDelta: (topic, payload) => {
          applyMqttDelta(mqttProjectId || projection?.project_id || 'demo', topic, JSON.stringify(payload))
        },
      }
    }

    void loadProjection()
  })

  onDestroy(() => {
    mqttClient?.end(true)
    if (import.meta.env.DEV) {
      delete (window as typeof window & { __runtimeUiTestHook__?: RuntimeUiTestHook }).__runtimeUiTestHook__
    }
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
        throw new Error(await responseErrorMessage(response, 'projection'))
      }

      projection = (await response.json()) as ScreenProjection
      lastUpdated = new Date().toLocaleTimeString()
      connectMqtt(projection.project_id)
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
      if (!response.ok) {
        throw new Error(await responseErrorMessage(response, 'command'))
      }

      const body = (await response.json()) as CommandResponse
      commandMessage = `command ${body.command?.status ?? 'accepted'}`
      await loadProjection()
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : 'command failed'
    }
  }

  function connectMqtt(projectId: string) {
    if (mqttClient && mqttProjectId === projectId) return

    mqttClient?.end(true)
    mqttProjectId = projectId
    mqttState = 'connecting'
    mqttMessage = `connecting ${mqttUrl}`

    const client = mqtt.connect(mqttUrl, {
      clientId: `runtime-ui-${Math.random().toString(16).slice(2)}`,
      reconnectPeriod: 1500,
      connectTimeout: 5000,
      clean: true,
    })
    mqttClient = client

    client.on('connect', () => {
      mqttState = 'connected'
      mqttMessage = `subscribed scada/${projectId}/tag/+/value`
      client.subscribe(`scada/${projectId}/tag/+/value`, { qos: 0 }, (error) => {
        if (error) {
          mqttState = 'error'
          mqttMessage = error.message
        }
      })
    })

    client.on('reconnect', () => {
      mqttState = 'reconnecting'
      mqttMessage = `reconnecting ${mqttUrl}`
    })

    client.on('offline', () => {
      mqttState = 'offline'
      mqttMessage = 'mqtt offline'
    })

    client.on('error', (error) => {
      mqttState = 'error'
      mqttMessage = error.message
    })

    client.on('message', (topic, payload) => {
      applyMqttDelta(projectId, topic, payload.toString())
    })
  }

  async function responseErrorMessage(response: Response, label: string): Promise<string> {
    try {
      const body = (await response.clone().json()) as ErrorResponse
      return body.error ?? `${label} ${response.status}`
    } catch {
      return `${label} ${response.status}`
    }
  }

  function applyMqttDelta(projectId: string, topic: string, payload: string) {
    if (!projection) return

    let delta: RuntimeTagValue
    try {
      delta = JSON.parse(payload) as RuntimeTagValue
    } catch (error) {
      mqttState = 'error'
      mqttMessage = error instanceof Error ? error.message : 'invalid mqtt payload'
      return
    }

    const expectedTopic = `scada/${projectId}/tag/${delta.tag_id}/value`
    if (topic !== expectedTopic || typeof delta.sequence !== 'number') return

    let applied = false
    projection = {
      ...projection,
      object_states: projection.object_states.map((object) => ({
        ...object,
        bindings: object.bindings.map((binding) => {
          if (binding.tag_id !== delta.tag_id) return binding
          if (binding.sequence !== null && delta.sequence <= binding.sequence) return binding
          applied = true
          return {
            ...binding,
            value: delta.value,
            quality: delta.quality,
            sequence: delta.sequence,
          }
        }),
      })),
    }

    if (applied) {
      deltaCount += 1
      lastUpdated = new Date().toLocaleTimeString()
      mqttMessage = `delta ${delta.tag_id} seq ${delta.sequence}`
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
      <span class:online={mqttState === 'connected'} class="status-pill">
        MQTT {mqttState}
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
    <div>
      <span>Deltas</span>
      <strong>{deltaCount}</strong>
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
      <p class="command-state">{mqttMessage || 'MQTT waiting'}</p>
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
