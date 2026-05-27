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
  await expect(page.getByText(/21\.5/)).toBeVisible()
  await expect(page.getByText('Stopped')).toBeVisible()

  await page.getByRole('button', { name: 'Start' }).click()
  await expect(page.getByText('command DriverAck')).toBeVisible()

  expect(commandCalls).toBeGreaterThanOrEqual(1)
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
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
  await expect(page.getByText('driver manager unavailable')).toBeVisible()

  expect(commandCalls).toBeGreaterThanOrEqual(1)
  // Initial snapshot only. Failure path should not perform an extra refresh.
  expect(projectionCalls).toBe(1)
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
  await expect(page.getByText(/21\.5/)).toBeVisible()

  const refreshResponse = page.waitForResponse(
    (response) =>
      response.url().includes('/api/v1/screens/projection') &&
      response.request().method() === 'POST'
  )
  await page.getByRole('button', { name: 'Refresh projection' }).click()
  await refreshResponse

  await expect(page.getByText(/24\.2/)).toBeVisible()
  expect(projectionCalls).toBeGreaterThanOrEqual(2)
})
