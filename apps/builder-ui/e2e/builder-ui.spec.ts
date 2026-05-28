import { expect, test } from '@playwright/test'

test('maps known coded condition error and shows structured response', async ({ page }) => {
  await page.route('**/api/v1/errors/map', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        code: 'MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX',
        path: 'object=pump-001 property=color',
        detail: "op 'between' requires min and max",
        user_message:
          'invalid modify rule condition at object=pump-001 property=color: between requires min and max',
        known_code: true,
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Error Mapping Lab' })).toBeVisible()

  await page.getByRole('button', { name: 'Map error' }).click()

  await expect(page.getByTestId('user-message-row')).toContainText(
    'invalid modify rule condition at object=pump-001 property=color: between requires min and max'
  )
  await expect(page.getByTestId('known-code-badge')).toHaveText('Known code')
  await expect(page.getByTestId('path-row')).toContainText('object=pump-001 property=color')
  await expect(page.getByTestId('focus-status-row')).toContainText('Focused property editor for color')
  await expect(page.getByTestId('object-field')).toHaveValue('pump-001')
  await expect(page.getByTestId('property-field')).toHaveValue('color')
  await expect(page.getByTestId('property-field')).toBeFocused()
  await expect(page.getByTestId('condition-op-field')).toHaveValue('any')
  await expect(page.getByTestId('condition-values-field')).toHaveValue('lt:18,gt:28')
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"screen_id": "mock-main"/)
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"object_id": "pump-001"/)
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"property": "color"/)
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"any"/)
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"op": "lt"/)
})

test('keeps fallback behavior for unknown codes', async ({ page }) => {
  await page.route('**/api/v1/errors/map', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        code: 'SOME_NEW_ERROR',
        path: 'object=valve-002 property=text',
        detail: 'unexpected runtime validation state',
        user_message:
          'code=SOME_NEW_ERROR path=object=valve-002 property=text detail=unexpected runtime validation state',
        known_code: false,
      }),
    })
  })

  await page.goto('/')
  await page.getByRole('button', { name: 'Use unknown sample' }).click()
  await page.getByRole('button', { name: 'Map error' }).click()

  await expect(page.getByTestId('known-code-badge')).toHaveText('Fallback mode')
  await expect(page.getByTestId('user-message-row')).toContainText(
    'code=SOME_NEW_ERROR path=object=valve-002 property=text detail=unexpected runtime validation state'
  )
  await expect(page.getByTestId('focus-status-row')).toContainText('Focused property editor for text')
  await expect(page.getByTestId('object-field')).toHaveValue('valve-002')
  await expect(page.getByTestId('property-field')).toHaveValue('text')
  await expect(page.getByTestId('condition-op-field')).toHaveValue('eq')
  await expect(page.getByTestId('condition-value-field')).toHaveValue('')
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"object_id": "valve-002"/)
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"property": "text"/)
})

test('loads and downloads screen-definition json', async ({ page }) => {
  const loadedScreenJson = {
    schema_version: '1.0.0',
    screen_id: 'loaded-screen',
    project_id: 'demo',
    name: 'Loaded Screen',
    canvas_width: 1024,
    canvas_height: 600,
    objects: [
      {
        object_id: 'tank-001',
        svg_asset_id: 'tank-symbol',
        x: 40,
        y: 50,
        width: 160,
        height: 180,
        tag_bindings: {
          value: 'mock.level.001',
        },
        modify_rules: [
          {
            property: 'visible',
            binding_key: 'value',
            true_value: 'true',
            false_value: 'false',
            condition: {
              op: 'between',
              min: 10,
              max: 90,
            },
          },
        ],
      },
    ],
  }

  await page.goto('/')

  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByTestId('download-screen-button').click(),
  ])
  await expect(download.suggestedFilename()).toContain('mock-main.screen.json')

  await page.getByTestId('screen-file-input').setInputFiles({
    name: 'loaded-screen.screen.json',
    mimeType: 'application/json',
    buffer: Buffer.from(JSON.stringify(loadedScreenJson), 'utf-8'),
  })

  await expect(page.getByTestId('io-status')).toContainText('Loaded loaded-screen.screen.json')
  await expect(page.getByTestId('object-field')).toHaveValue('tank-001')
  await expect(page.getByTestId('condition-op-field')).toHaveValue('between')
  await expect(page.getByTestId('condition-min-field')).toHaveValue('10')
  await expect(page.getByTestId('condition-max-field')).toHaveValue('90')
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"screen_id": "loaded-screen"/)
  await expect(page.getByTestId('screen-json-preview')).toHaveValue(/"object_id": "tank-001"/)
})

test('loads and saves screen-definition via builder api', async ({ page }) => {
  await page.route('**/runtime-api/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        screen_id: 'mock-main',
        project_id: 'demo',
        object_states: [{ object_id: 'project-pump-001', bindings: [] }],
      }),
    })
  })

  await page.route('**/api/v1/screens/mock-main', async (route) => {
    if (route.request().method() === 'GET') {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          schema_version: '1.0.0',
          screen_id: 'mock-main',
          project_id: 'demo',
          name: 'Mock Main Screen',
          canvas_width: 1280,
          canvas_height: 720,
          objects: [
            {
              object_id: 'project-pump-001',
              svg_asset_id: 'pump-symbol',
              x: 80,
              y: 120,
              width: 120,
              height: 120,
              tag_bindings: {
                value: 'mock.temperature.001',
              },
              modify_rules: [
                {
                  property: 'color',
                  binding_key: 'value',
                  true_value: '#cc3333',
                  false_value: '#3cb371',
                  condition: {
                    op: 'gt',
                    value: 24,
                  },
                },
              ],
            },
          ],
        }),
      })
      return
    }

    const payload = JSON.parse(route.request().postData() ?? '{}') as Record<string, unknown>
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        saved_path: `config/screens/${String(payload.screen_id ?? 'unknown')}.screen.json`,
      }),
    })
  })

  await page.goto('/')

  await expect(page.getByTestId('project-path-row')).toContainText('config/screens/mock-main.screen.json')

  await page.getByTestId('load-project-screen-button').click()
  await expect(page.getByTestId('io-status')).toContainText('Loaded mock-main from project')
  await expect(page.getByTestId('object-field')).toHaveValue('project-pump-001')
  await expect(page.getByTestId('screen-id-field')).toHaveValue('mock-main')
  await expect(page.getByTestId('project-id-field')).toHaveValue('demo')
  await expect(page.getByTestId('screen-name-field')).toHaveValue('Mock Main Screen')
  await expect(page.getByTestId('condition-op-field')).toHaveValue('gt')
  await expect(page.getByTestId('condition-value-field')).toHaveValue('24')

  await page.getByTestId('save-project-screen-button').dispatchEvent('click')
  await expect(page.getByTestId('io-status')).toContainText('Saved to config/screens/mock-main.screen.json')

  await page.route('**/api/v1/screens/save-as', async (route) => {
    const payload = JSON.parse(route.request().postData() ?? '{}') as Record<string, unknown>
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        saved_path: String(payload.relative_path ?? 'config/screens/fallback.screen.json'),
      }),
    })
  })

  await page.getByTestId('save-as-path-field').fill('config/screens/custom/from-ui.screen.json')
  await page.getByTestId('save-as-project-screen-button').click()
  await expect(page.getByTestId('io-status')).toContainText('Saved to config/screens/custom/from-ui.screen.json')

  await page.getByTestId('preview-runtime-button').click()
  await expect(page.getByTestId('runtime-preview-status-row')).toContainText('Projection loaded: mock-main (1 objects)')
})

test('blocks project save when screen_id is invalid', async ({ page }) => {
  let screenApiCallCount = 0
  await page.route('**/api/v1/screens/**', async (route) => {
    screenApiCallCount += 1
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: JSON.stringify({ error: 'unexpected call' }),
    })
  })

  await page.goto('/')
  await page.getByTestId('screen-id-field').fill('bad id')

  await expect(page.getByTestId('screen-id-validation-message')).toContainText('screen_id must match [A-Za-z0-9_-]')
  await expect(page.getByTestId('save-project-screen-button')).toBeDisabled()
  await expect(page.getByTestId('project-path-row')).toContainText('config/screens/bad id.screen.json')

  await page.getByTestId('load-project-screen-button').click()
  await expect(page.getByTestId('io-status')).toContainText('Project I/O failed: screen_id must match [A-Za-z0-9_-]')
  await expect(screenApiCallCount).toBe(0)
})

test('blocks save-as when relative_path is invalid', async ({ page }) => {
  let saveAsApiCallCount = 0
  await page.route('**/api/v1/screens/save-as', async (route) => {
    saveAsApiCallCount += 1
    await route.fulfill({
      status: 500,
      contentType: 'application/json',
      body: JSON.stringify({ error: 'unexpected call' }),
    })
  })

  await page.goto('/')
  await page.getByTestId('save-as-path-field').fill('../outside.screen.json')

  await expect(page.getByTestId('save-as-path-validation-message')).toContainText(
    'relative_path must match config/screens/*.screen.json'
  )
  await expect(page.getByTestId('save-as-project-screen-button')).toBeDisabled()

  await page.getByTestId('use-screen-id-path-button').click()
  await expect(page.getByTestId('save-as-path-field')).toHaveValue('config/screens/mock-main.screen.json')
  await expect(page.getByTestId('save-as-project-screen-button')).toBeEnabled()
  await expect(page.getByTestId('preview-runtime-button')).toBeEnabled()

  await page.getByTestId('save-as-path-field').fill('')
  await expect(page.getByTestId('preview-runtime-button')).toBeEnabled()
  await expect(saveAsApiCallCount).toBe(0)
})

test('uses tauri picker result for save-as path', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __TAURI__?: Record<string, unknown> }).__TAURI__ = {
      core: {
        invoke: async (command: string, args: Record<string, unknown>) => {
          if (command !== 'pick_screen_relative_path') {
            throw new Error(`unexpected command: ${command}`)
          }

          if (typeof args.initial_path !== 'string') {
            throw new Error('initial_path must be string')
          }

          return {
            cancelled: false,
            relative_path: 'config/screens/picked/by-tauri.screen.json',
          }
        },
      },
    }
  })

  await page.goto('/')

  await expect(page.getByTestId('pick-save-as-path-button')).toContainText('Pick via Tauri')
  await page.getByTestId('pick-save-as-path-button').click()

  await expect(page.getByTestId('save-as-path-field')).toHaveValue('config/screens/picked/by-tauri.screen.json')
  await expect(page.getByTestId('io-status')).toContainText('Selected config/screens/picked/by-tauri.screen.json')
})

test('uses legacy tauri invoke path for save-as picker', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __TAURI__?: Record<string, unknown> }).__TAURI__ = {
      invoke: async (command: string) => {
        if (command !== 'pick_screen_relative_path') {
          throw new Error(`unexpected command: ${command}`)
        }
        return {
          cancelled: false,
          relative_path: 'config/screens/picked/by-legacy.screen.json',
        }
      },
    }
  })

  await page.goto('/')

  await page.getByTestId('pick-save-as-path-button').click()
  await expect(page.getByTestId('save-as-path-field')).toHaveValue('config/screens/picked/by-legacy.screen.json')
  await expect(page.getByTestId('io-status')).toContainText('Selected config/screens/picked/by-legacy.screen.json')
})

test('shows path selection failure when tauri picker returns invalid contract', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __TAURI__?: Record<string, unknown> }).__TAURI__ = {
      core: {
        invoke: async () => {
          return {
            cancelled: false,
            relative_path: '../outside.screen.json',
          }
        },
      },
    }
  })

  await page.goto('/')

  await page.getByTestId('pick-save-as-path-button').click()
  await expect(page.getByTestId('io-status')).toContainText('Path selection failed: invalid relative_path from tauri picker')
})

test('loads supervise log summary through tauri bridge', async ({ page }) => {
  await page.addInitScript(() => {
    ;(window as unknown as { __TAURI__?: Record<string, unknown> }).__TAURI__ = {
      core: {
        invoke: async (command: string, args: Record<string, unknown>) => {
          if (command === 'read_supervise_log_summary') {
            if (typeof args.log_dir !== 'string') {
              throw new Error('log_dir must be string')
            }
            return {
              cycle_summaries: 3,
              final_summaries: 1,
              parse_errors: 0,
              services: [
                {
                  service: 'tag-server',
                  lines: 10,
                  exited: 1,
                  started_false: 0,
                },
              ],
            }
          }

          if (command === 'pick_screen_relative_path') {
            return {
              cancelled: true,
              relative_path: null,
            }
          }

          throw new Error(`unexpected command: ${command}`)
        },
      },
    }
  })

  await page.goto('/')
  await page.getByTestId('supervise-log-dir-field').fill('/tmp/scada-supervise-log')
  await page.getByTestId('load-supervise-summary-button').click()

  await expect(page.getByTestId('supervise-summary-status-row')).toContainText('Loaded summary from /tmp/scada-supervise-log')
  await expect(page.getByTestId('supervise-summary-health-badge')).toHaveText('Issue detected')
  await expect(page.getByTestId('supervise-summary-counts-row')).toContainText('cycle=3 final=1 parse_errors=0')
  await expect(page.getByTestId('supervise-summary-issues-row')).toContainText(
    'issues=1 exited_total=1 started_false_total=0'
  )
  await expect(page.getByTestId('supervise-summary-services-list')).toContainText(
    'tag-server: lines=10 exited=1 started_false=0'
  )
})