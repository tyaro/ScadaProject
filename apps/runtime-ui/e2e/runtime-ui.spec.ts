import { expect, test } from '@playwright/test'

const projectionResponse = {
  screen_id: 'mock-main',
  project_id: 'demo',
  object_states: [
    {
      object_id: 'pump-001',
      svg_asset_id: 'pump-symbol',
      bindings: [
        {
          key: 'state',
          tag_id: 'mock.running.001',
          value: false,
          quality: 'Simulated',
          sequence: 5,
        },
        {
          key: 'value',
          tag_id: 'mock.temperature.001',
          value: 21.5,
          quality: 'Simulated',
          sequence: 6,
        },
      ],
    },
  ],
}

const projectionWithModifiers = {
  screen_id: 'mock-main',
  project_id: 'demo',
  object_states: [
    {
      object_id: 'pump-001',
      svg_asset_id: 'pump-symbol',
      bindings: [
        {
          key: 'state',
          tag_id: 'mock.running.001',
          value: false,
          quality: 'Simulated',
          sequence: 5,
        },
        {
          key: 'value',
          tag_id: 'mock.temperature.001',
          value: 21.5,
          quality: 'Simulated',
          sequence: 6,
        },
      ],
      modifiers: [
        {
          property: 'visible',
          binding_key: 'state',
          tag_id: 'mock.running.001',
          source_value: true,
          rendered: 'true',
          true_value: 'true',
          false_value: 'false',
        },
        {
          property: 'color',
          binding_key: 'state',
          tag_id: 'mock.running.001',
          source_value: true,
          rendered: '#22aa44',
          true_value: '#22aa44',
          false_value: '#999999',
        },
      ],
    },
    {
      object_id: 'label-001',
      svg_asset_id: 'label-symbol',
      bindings: [],
      modifiers: [
        {
          property: 'text',
          binding_key: 'state',
          tag_id: 'mock.running.001',
          source_value: true,
          rendered: 'Pump Ready',
          true_value: 'Pump Ready',
          false_value: 'Pump Stopped',
        },
      ],
    },
  ],
}

const projectionWithConditionalModifiers = {
  screen_id: 'mock-main',
  project_id: 'demo',
  object_states: [
    {
      object_id: 'pump-001',
      svg_asset_id: 'pump-symbol',
      bindings: [
        {
          key: 'value',
          tag_id: 'mock.temperature.001',
          value: 21.5,
          quality: 'Simulated',
          sequence: 6,
        },
      ],
      modifiers: [
        {
          property: 'color',
          binding_key: 'value',
          tag_id: 'mock.temperature.001',
          source_value: 21.5,
          rendered: '#00aa44',
          true_value: '#ff0000',
          false_value: '#00aa44',
          condition: {
            op: 'gte',
            value: 25,
          },
        },
      ],
    },
    {
      object_id: 'label-001',
      svg_asset_id: 'label-symbol',
      bindings: [],
      modifiers: [
        {
          property: 'text',
          binding_key: 'value',
          tag_id: 'mock.temperature.001',
          source_value: 21.5,
          rendered: 'Normal',
          true_value: 'Alarm',
          false_value: 'Normal',
          condition: {
            any: [
              { op: 'lt', value: 18 },
              { op: 'gt', value: 28 },
            ],
          },
        },
      ],
    },
  ],
}

test('renders projection and posts control command', async ({ page }) => {
  let projectionCalls = 0
  let commandCalls = 0

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    commandCalls += 1
    const request = route.request()
    const body = request.postDataJSON() as {
      tag_id: string
      requested_value: boolean
    }
    expect(body.tag_id).toBe('mock.running.001')
    expect(typeof body.requested_value).toBe('boolean')

    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Runtime Monitor' })).toBeVisible()
  await expect(page.getByText('mock-main')).toBeVisible()
  await expect(page.getByText('21.5 °C')).toBeVisible()
  await expect(page.getByText('Stopped')).toBeVisible()

  await page.getByRole('button', { name: 'Start' }).click()
  await expect(page.getByText('command DriverAck')).toBeVisible()

  expect(commandCalls).toBeGreaterThanOrEqual(1)
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})

test('applies modifier text, color, and visibility from projection', async ({ page }) => {
  let projectionCalls = 0

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    if (projectionCalls === 1) {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(projectionWithModifiers),
      })
      return
    }

    const hiddenProjection = {
      ...projectionWithModifiers,
      object_states: projectionWithModifiers.object_states.map((objectState) => {
        if (objectState.object_id === 'pump-001') {
          return {
            ...objectState,
            modifiers: (objectState.modifiers ?? []).map((modifier) =>
              modifier.property === 'visible'
                ? { ...modifier, source_value: false, rendered: 'false' }
                : modifier
            ),
          }
        }
        if (objectState.object_id === 'label-001') {
          return {
            ...objectState,
            modifiers: (objectState.modifiers ?? []).map((modifier) =>
              modifier.property === 'text'
                ? { ...modifier, source_value: false, rendered: 'Pump Hidden' }
                : modifier
            ),
          }
        }
        return objectState
      }),
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(hiddenProjection),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')

  await expect(page.locator('.state-stack strong')).toHaveText('Pump Ready')
  await expect(page.locator('.pump-body')).toHaveCSS('background-color', 'rgb(34, 170, 68)')
  await expect(page.locator('.pump-asset')).toHaveCount(1)

  await page.getByRole('button', { name: 'Refresh projection' }).click()

  await expect(page.locator('.state-stack strong')).toHaveText('Pump Hidden')
  await expect(page.locator('.pump-asset')).toHaveCount(0)
})

test('re-evaluates modifiers on mqtt delta without projection refresh', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionWithModifiers),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.locator('.state-stack strong')).toHaveText('Pump Ready')
  await expect(page.locator('.pump-asset')).toHaveCount(1)

  await page.evaluate(() => {
    ;(
      window as Window & {
        __runtimeUiTestHook__?: {
          applyDelta: (topic: string, payload: unknown) => void
        }
      }
    ).__runtimeUiTestHook__?.applyDelta('scada/demo/tag/mock.running.001/value', {
      tag_id: 'mock.running.001',
      value: false,
      quality: 'Simulated',
      sequence: 7,
    })
  })

  await expect(page.locator('.state-stack strong')).toHaveText('Pump Stopped')
  await expect(page.locator('.pump-asset')).toHaveCount(0)
})

test('re-evaluates condition-based modifiers on mqtt delta', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionWithConditionalModifiers),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.locator('.state-stack strong')).toHaveText('Normal')
  await expect(page.locator('.pump-body')).toHaveCSS('background-color', 'rgb(0, 170, 68)')

  await page.evaluate(() => {
    ;(
      window as Window & {
        __runtimeUiTestHook__?: {
          applyDelta: (topic: string, payload: unknown) => void
        }
      }
    ).__runtimeUiTestHook__?.applyDelta('scada/demo/tag/mock.temperature.001/value', {
      tag_id: 'mock.temperature.001',
      value: 30,
      quality: 'Simulated',
      sequence: 7,
    })
  })

  await expect(page.locator('.state-stack strong')).toHaveText('Alarm')
  await expect(page.locator('.pump-body')).toHaveCSS('background-color', 'rgb(255, 0, 0)')
})

test('re-fetches projection after start command', async ({ page }) => {
  let projectionCalls = 0
  let startIssued = false
  const snapshotStopped = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.running.001'
            ? { ...binding, value: false, sequence: 10 }
            : binding
        ),
      },
    ],
  }
  const snapshotRunning = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.running.001'
            ? { ...binding, value: true, sequence: 11 }
            : binding
        ),
      },
    ],
  }

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    const body = startIssued ? snapshotRunning : snapshotStopped
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(body),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    startIssued = true
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('Stopped')).toBeVisible()

  const refreshResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/screens/projection') &&
      response.request().method() === 'POST'
  )
  const commandResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/control-commands') &&
      response.request().method() === 'POST'
  )
  await page.getByRole('button', { name: 'Start' }).click()
  await commandResponse
  await refreshResponse
  await expect(page.getByText('command DriverAck')).toBeVisible()
  expect(startIssued).toBe(true)
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})

test('re-fetches projection after stop command', async ({ page }) => {
  let projectionCalls = 0
  let stopIssued = false
  const snapshotRunning = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.running.001'
            ? { ...binding, value: true, sequence: 12 }
            : binding
        ),
      },
    ],
  }
  const snapshotStopped = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.running.001'
            ? { ...binding, value: false, sequence: 13 }
            : binding
        ),
      },
    ],
  }

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    const body = stopIssued ? snapshotStopped : snapshotRunning
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(body),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    const request = route.request().postDataJSON() as {
      requested_value: boolean
      tag_id: string
    }
    expect(request.tag_id).toBe('mock.running.001')
    expect(request.requested_value).toBe(false)
    stopIssued = true
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')

  const commandResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/control-commands') &&
      response.request().method() === 'POST'
  )
  const refreshResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/screens/projection') &&
      response.request().method() === 'POST'
  )
  await page.getByRole('button', { name: 'Stop' }).click()
  await commandResponse
  await refreshResponse

  await expect(page.getByText('command DriverAck')).toBeVisible()
  expect(stopIssued).toBe(true)
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})

test('shows error when control command fails', async ({ page }) => {
  let projectionCalls = 0
  let commandCalls = 0

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    commandCalls += 1
    await route.fulfill({
      status: 502,
      contentType: 'application/json',
      body: JSON.stringify({
        error: 'driver manager unavailable',
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByRole('heading', { name: 'Runtime Monitor' })).toBeVisible()

  await page.getByRole('button', { name: 'Start' }).click()
  await expect(page.getByRole('alert')).toContainText('Command')
  await expect(page.getByText('driver manager unavailable')).toBeVisible()

  expect(commandCalls).toBeGreaterThanOrEqual(1)
  // Initial snapshot only. Failure path should not perform an extra refresh.
  expect(projectionCalls).toBe(1)
})

test('falls back to status when control command error body is not json', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 502,
      contentType: 'text/plain',
      body: 'bad gateway',
    })
  })

  await page.goto('/')
  await expect(page.getByText('Stopped')).toBeVisible()

  await page.getByRole('button', { name: 'Start' }).click()
  await expect(page.getByRole('alert')).toContainText('Command')
  await expect(page.getByText('command 502')).toBeVisible()
})

test('shows first publish error when control command error body has publish_errors', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 502,
      contentType: 'application/json',
      body: JSON.stringify({
        publish_errors: ['mqtt broker unavailable', 'publish timeout'],
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('Stopped')).toBeVisible()

  await page.getByRole('button', { name: 'Start' }).click()
  await expect(page.getByRole('alert')).toContainText('Command')
  await expect(page.getByText('mqtt broker unavailable')).toBeVisible()
  await expect(page.getByText('command 502')).toHaveCount(0)
})

test('falls back to accepted when control command success body is empty', async ({ page }) => {
  let commandIssued = false
  let projectionCalls = 0
  const snapshotStopped = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.running.001'
            ? { ...binding, value: false, sequence: 14 }
            : binding
        ),
      },
    ],
  }
  const snapshotRunning = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.running.001'
            ? { ...binding, value: true, sequence: 15 }
            : binding
        ),
      },
    ],
  }

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(commandIssued ? snapshotRunning : snapshotStopped),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    commandIssued = true
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: '',
    })
  })

  await page.goto('/')
  await expect(page.getByText('Stopped')).toBeVisible()

  await page.getByRole('button', { name: 'Start' }).click()

  await expect(page.getByText('command accepted')).toBeVisible()
  await expect(page.getByText('Running', { exact: true })).toBeVisible()
  await expect(page.getByLabel('Bindings').getByText('true', { exact: true })).toBeVisible()
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})

test('falls back to accepted when control command success body is not json', async ({ page }) => {
  let commandIssued = false

  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    commandIssued = true
    await route.fulfill({
      status: 202,
      contentType: 'text/plain',
      body: 'accepted',
    })
  })

  await page.goto('/')
  await expect(page.getByText('Stopped')).toBeVisible()

  await page.getByRole('button', { name: 'Start' }).click()

  await expect(page.getByText('command accepted')).toBeVisible()
  expect(commandIssued).toBe(true)
})

test('shows error when projection fetch fails', async ({ page }) => {
  let projectionCalls = 0

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    await route.fulfill({
      status: 502,
      contentType: 'application/json',
      body: JSON.stringify({
        error: 'tag server snapshot failed',
      }),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')

  await expect(page.getByRole('heading', { name: 'Runtime Monitor' })).toBeVisible()
  await expect(page.getByRole('alert')).toContainText('Projection')
  await expect(page.getByText('tag server snapshot failed')).toBeVisible()
  await expect(page.getByText('Offline')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Start' })).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Stop' })).toBeDisabled()
  await expect(page.getByText('mock-main')).toHaveCount(0)
  expect(projectionCalls).toBe(1)
})

test('falls back to status when projection error body is not json', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 503,
      contentType: 'text/plain',
      body: 'maintenance',
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')

  await expect(page.getByRole('alert')).toContainText('Projection')
  await expect(page.getByText('projection 503')).toBeVisible()
  await expect(page.getByText('Offline')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Start' })).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Stop' })).toBeDisabled()
})

test('maps coded modify-rule validation errors to user-friendly message', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 400,
      contentType: 'application/json',
      body: JSON.stringify({
        error:
          'screen definition error: invalid config/screens/mock-main.screen.json: code=MODIFY_RULE_CONDITION_BETWEEN_REQUIRES_MIN_MAX path=object=pump-001 property=color detail=op \'between\' requires min and max',
      }),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')

  await expect(page.getByRole('alert')).toContainText('Projection')
  await expect(
    page.getByText('invalid modify rule condition at object=pump-001 property=color: between requires min and max')
  ).toBeVisible()
})

test('recovers from projection fetch failure after manual refresh', async ({ page }) => {
  let projectionCalls = 0

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    if (projectionCalls === 1) {
      await route.fulfill({
        status: 502,
        contentType: 'application/json',
        body: JSON.stringify({
          error: 'tag server snapshot failed',
        }),
      })
      return
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByRole('alert')).toContainText('Projection')
  await expect(page.getByText('tag server snapshot failed')).toBeVisible()

  const refreshResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/screens/projection') &&
      response.request().method() === 'POST'
  )
  await page.getByRole('button', { name: 'Refresh projection' }).click()
  await refreshResponse

  await expect(page.getByText('tag server snapshot failed')).toHaveCount(0)
  await expect(page.getByText('mock-main')).toBeVisible()
  await expect(page.getByText('21.5 °C')).toBeVisible()
  await expect(page.getByText('Online')).toBeVisible()
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})

test('keeps last projection visible when manual refresh fails', async ({ page }) => {
  let projectionCalls = 0

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    if (projectionCalls === 1) {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify(projectionResponse),
      })
      return
    }

    await route.fulfill({
      status: 502,
      contentType: 'application/json',
      body: JSON.stringify({
        error: 'tag server snapshot failed',
      }),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('mock-main')).toBeVisible()
  await expect(page.getByText('21.5 °C')).toBeVisible()

  const refreshResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/screens/projection') &&
      response.request().method() === 'POST'
  )
  await page.getByRole('button', { name: 'Refresh projection' }).click()
  await refreshResponse

  await expect(page.getByRole('alert')).toContainText('Projection')
  await expect(page.getByRole('alert')).toContainText('tag server snapshot failed')
  await expect(page.getByText('mock-main')).toBeVisible()
  await expect(page.getByText('21.5 °C')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Start' })).toBeEnabled()
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})

test('updates displayed values after manual refresh', async ({ page }) => {
  let projectionCalls = 0
  const firstSnapshot = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.temperature.001'
            ? { ...binding, value: 21.5, sequence: 20 }
            : binding
        ),
      },
    ],
  }
  const secondSnapshot = {
    ...projectionResponse,
    object_states: [
      {
        ...projectionResponse.object_states[0],
        bindings: projectionResponse.object_states[0].bindings.map((binding) =>
          binding.tag_id === 'mock.temperature.001'
            ? { ...binding, value: 24.2, sequence: 21 }
            : binding
        ),
      },
    ],
  }

  await page.route('**/api/v1/screens/projection', async (route) => {
    projectionCalls += 1
    const body = projectionCalls <= 1 ? firstSnapshot : secondSnapshot
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(body),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('21.5 °C')).toBeVisible()

  const refreshResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/screens/projection') &&
      response.request().method() === 'POST'
  )
  await page.getByRole('button', { name: 'Refresh projection' }).click()
  await refreshResponse

  await expect(page.getByText('24.2 °C')).toBeVisible()
  await expect(page.getByLabel('Bindings').getByText('24.2', { exact: true })).toBeVisible()
  await expect(page.getByLabel('Bindings').getByText('21.5', { exact: true })).toHaveCount(0)
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})

test('updates displayed values after mqtt delta', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('21.5 °C')).toBeVisible()

  await page.evaluate(() => {
    ;(
      window as Window & {
        __runtimeUiTestHook__?: {
          applyDelta: (topic: string, payload: unknown) => void
        }
      }
    ).__runtimeUiTestHook__?.applyDelta('scada/demo/tag/mock.temperature.001/value', {
      tag_id: 'mock.temperature.001',
      value: 25.3,
      quality: 'Simulated',
      sequence: 7,
    })
  })

  await expect(page.getByText('25.3 °C')).toBeVisible()
  await expect(page.getByLabel('Bindings').getByText('25.3', { exact: true })).toBeVisible()
  await expect(page.getByText('delta mock.temperature.001 seq 7')).toBeVisible()
})

test('ignores stale mqtt delta by sequence', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('21.5 °C')).toBeVisible()

  await page.evaluate(() => {
    ;(
      window as Window & {
        __runtimeUiTestHook__?: {
          applyDelta: (topic: string, payload: unknown) => void
        }
      }
    ).__runtimeUiTestHook__?.applyDelta('scada/demo/tag/mock.temperature.001/value', {
      tag_id: 'mock.temperature.001',
      value: 19.9,
      quality: 'Simulated',
      sequence: 6,
    })
  })

  await expect(page.getByText('21.5 °C')).toBeVisible()
  await expect(page.getByText(/19\.9/)).toHaveCount(0)
  await expect(page.getByText('delta mock.temperature.001 seq 6')).toHaveCount(0)
})

test('shows MQTT error when delta payload is invalid json', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('21.5 °C')).toBeVisible()

  await page.evaluate(() => {
    ;(
      window as Window & {
        __runtimeUiTestHook__?: {
          applyDeltaRaw: (topic: string, payload: string) => void
        }
      }
    ).__runtimeUiTestHook__?.applyDeltaRaw(
      'scada/demo/tag/mock.temperature.001/value',
      '{not-json'
    )
  })

  await expect(page.getByRole('alert')).toContainText('MQTT')
  await expect(page.getByText('21.5 °C')).toBeVisible()
  await expect(page.getByText(/^Deltas$/)).toBeVisible()
  await expect(
    page.getByText('delta mock.temperature.001', { exact: false })
  ).toHaveCount(0)
})

test('ignores mqtt delta when topic does not match tag', async ({ page }) => {
  await page.route('**/api/v1/screens/projection', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(projectionResponse),
    })
  })

  await page.route('**/api/v1/control-commands', async (route) => {
    await route.fulfill({
      status: 202,
      contentType: 'application/json',
      body: JSON.stringify({
        command: { status: 'DriverAck' },
        driver_response: { accepted: true, message: 'accepted' },
      }),
    })
  })

  await page.goto('/')
  await expect(page.getByText('21.5 °C')).toBeVisible()

  await page.evaluate(() => {
    ;(
      window as Window & {
        __runtimeUiTestHook__?: {
          applyDelta: (topic: string, payload: unknown) => void
        }
      }
    ).__runtimeUiTestHook__?.applyDelta('scada/demo/tag/mock.other.999/value', {
      tag_id: 'mock.temperature.001',
      value: 99.9,
      quality: 'Simulated',
      sequence: 100,
    })
  })

  await expect(page.getByText('21.5 °C')).toBeVisible()
  await expect(page.getByText(/99\.9/)).toHaveCount(0)
  await expect(page.getByText('delta mock.temperature.001 seq 100')).toHaveCount(0)
  await expect(page.getByRole('alert')).toHaveCount(0)
})
