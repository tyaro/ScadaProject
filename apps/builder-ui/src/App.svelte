<script lang="ts">
  type ErrorMappingResponse = {
    code: string | null
    path: string | null
    detail: string | null
    user_message: string
    known_code: boolean
  }

  const sampleKnownError =
    "code=MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX path=object=pump-001 property=color detail=op 'between' requires min and max"
  const sampleUnknownError =
    "code=SOME_NEW_ERROR path=object=valve-002 property=text detail=unexpected runtime validation state"

  let inputError = sampleKnownError
  let loading = false
  let apiError = ''
  let response: ErrorMappingResponse | null = null

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
    } catch (error) {
      response = null
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
        <button class="primary" type="button" on:click={mapError} disabled={loading}>
          {loading ? 'Mapping...' : 'Map error'}
        </button>
        <button class="secondary" type="button" on:click={() => useSample(sampleKnownError)}>
          Use known sample
        </button>
        <button class="secondary" type="button" on:click={() => useSample(sampleUnknownError)}>
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
    </section>
  </section>
</main>