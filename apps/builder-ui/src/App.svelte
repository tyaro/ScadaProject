<script lang="ts">
  import { tick } from 'svelte'

  type ErrorMappingResponse = {
    code: string | null
    path: string | null
    detail: string | null
    user_message: string
    known_code: boolean
  }

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
  }

  type ScreenObjectForm = {
    objectId: string
    svgAssetId: string
    tagBindings: Record<string, string>
    modifyRules: ModifyRuleForm[]
  }

  const sampleKnownError =
    "code=MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX path=object=pump-001 property=color detail=op 'between' requires min and max"
  const sampleUnknownError =
    "code=SOME_NEW_ERROR path=object=valve-002 property=text detail=unexpected runtime validation state"
  const propertyOptions = ['visible', 'color', 'text']
  const initialScreenObjects: ScreenObjectForm[] = [
    {
      objectId: 'pump-001',
      svgAssetId: 'pump-symbol',
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
        },
        {
          property: 'color',
          bindingKey: 'value',
          trueValue: '#cc3333',
          falseValue: '#3cb371',
        },
      ],
    },
    {
      objectId: 'label-001',
      svgAssetId: 'text-label',
      tagBindings: {
        value: 'mock.temperature.001',
      },
      modifyRules: [
        {
          property: 'text',
          bindingKey: 'value',
          trueValue: 'NORMAL',
          falseValue: 'CHECK',
        },
      ],
    },
  ]

  let inputError = $state(sampleKnownError)
  let loading = $state(false)
  let apiError = $state('')
  let response = $state<ErrorMappingResponse | null>(null)
  let parsedTarget = $state<ParsedTargetPath>({ objectId: null, property: null, valid: false })
  let focusStatus = $state('No parsed target yet')
  let screenObjects = $state<ScreenObjectForm[]>(structuredClone(initialScreenObjects))
  let selectedObjectIndex = $state(0)
  let selectedRuleIndex = $state(0)
  let objectField: HTMLInputElement | null = null
  let propertyField: HTMLSelectElement | null = null

  const emptyRule = (): ModifyRuleForm => ({
    property: 'visible',
    bindingKey: 'value',
    trueValue: '',
    falseValue: '',
  })

  const emptyObject = (objectId: string): ScreenObjectForm => ({
    objectId,
    svgAssetId: 'draft-symbol',
    tagBindings: {
      value: '',
    },
    modifyRules: [emptyRule()],
  })

  function selectedObject(): ScreenObjectForm {
    return screenObjects[selectedObjectIndex]
  }

  function selectedRule(): ModifyRuleForm {
    const object = selectedObject()
    return object.modifyRules[selectedRuleIndex]
  }

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
      const httpResponse = await fetch('/api/v1/errors/map', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ error: inputError }),
      })

      if (!httpResponse.ok) {
        const errorBody = await parseError(httpResponse)
        throw new Error(errorBody)
      }

      response = (await httpResponse.json()) as ErrorMappingResponse
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
      const body = (await httpResponse.json()) as { error?: string }
      return body.error ?? `builder api ${httpResponse.status}`
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
      </div>

      {#if apiError}
        <div class="notice" role="alert">
          <strong>Builder API</strong>
          <span>{apiError}</span>
        </div>
      {/if}

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

        <div class="result-row compact" data-testid="editor-selection-row">
          <span>Selected Rule</span>
          <code>{selectedObject().objectId} / {selectedRule().property}</code>
        </div>
      </div>
    </section>
  </section>
</main>