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
