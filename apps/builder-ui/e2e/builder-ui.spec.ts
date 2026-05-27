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