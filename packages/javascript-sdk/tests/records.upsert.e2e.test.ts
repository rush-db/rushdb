import path from 'path'
import dotenv from 'dotenv'

dotenv.config({ path: path.resolve(__dirname, '../.env') })

import RushDB from '../src/index.node'

jest.setTimeout(60_000)

describe('records.upsert (e2e)', () => {
  const apiKey = process.env.RUSHDB_API_KEY
  const apiUrl = process.env.RUSHDB_API_URL || 'http://localhost:3000'

  if (!apiKey) {
    it('skips because RUSHDB_API_KEY is not set', () => {
      expect(true).toBe(true)
    })
    return
  }

  const db = new RushDB(apiKey, { url: apiUrl })

  it('creates then appends vs rewrites using mergeBy', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    // Initial create via upsert (no existing match)
    const first = await db.records.upsert({
      label: 'Product',
      data: { sku: 'SKU-1', name: 'Widget', price: 10, tenantId },
      options: { mergeBy: ['sku', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(first.data.sku).toBe('SKU-1')
    expect(first.data.name).toBe('Widget')
    expect(first.data.price).toBe(10)

    // Append update (should keep existing name, update price only if provided)
    const second = await db.records.upsert({
      label: 'Product',
      data: { sku: 'SKU-1', tenantId, price: 15, size: 8 },
      options: { mergeBy: ['sku', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(second.data.sku).toBe('SKU-1')
    expect(second.data.price).toBe(15)
    // name should still exist because append preserves other fields
    expect(second.data.name).toBe('Widget')

    // Rewrite update (should remove unspecified fields like name and set provided ones only)
    const third = await db.records.upsert({
      label: 'Product',
      data: { sku: 'SKU-1', tenantId, price: 20 },
      options: { mergeBy: ['sku', 'tenantId'], mergeStrategy: 'rewrite', suggestTypes: true }
    })

    expect(third.data.sku).toBe('SKU-1')
    expect(third.data.price).toBe(20)
    // name should be gone after rewrite
    expect(third.data.name).toBeUndefined()

    // Cleanup
    await db.records.delete({ labels: ['Product'], where: { tenantId } })
  })

  it('creates a new record when no mergeBy match exists', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const result = await db.records.upsert({
      label: 'User',
      data: { email: 'test@example.com', name: 'John Doe', tenantId },
      options: { mergeBy: ['email', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(result.data.email).toBe('test@example.com')
    expect(result.data.name).toBe('John Doe')
    expect(result.data.tenantId).toBe(tenantId)

    // Cleanup
    await db.records.delete({ labels: ['User'], where: { tenantId } })
  })

  it('handles upsert without mergeBy (matches on all properties)', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    // First upsert
    const first = await db.records.upsert({
      label: 'Order',
      data: { orderId: 'ORD-001', status: 'pending', tenantId }
    })

    expect(first.data.orderId).toBe('ORD-001')
    expect(first.data.status).toBe('pending')

    // Second upsert with exact same properties (should match and update since mergeBy is all props)
    const second = await db.records.upsert({
      label: 'Order',
      data: { orderId: 'ORD-001', status: 'pending', tenantId }
    })

    // Same ID confirms it matched on all properties
    expect(first.id()).toBe(second.id())

    // Third upsert with different property value (should create new record since not all props match)
    const third = await db.records.upsert({
      label: 'Order',
      data: { orderId: 'ORD-001', status: 'completed', tenantId }
    })

    expect(third.data.orderId).toBe('ORD-001')
    expect(third.data.status).toBe('completed')
    // Different ID confirms it's a new record (status didn't match)
    expect(first.id()).not.toBe(third.id())

    // Cleanup
    await db.records.delete({ labels: ['Order'], where: { tenantId } })
  })

  it('handles upsert with single mergeBy field', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const first = await db.records.upsert({
      label: 'Setting',
      data: { key: 'theme', value: 'dark', tenantId },
      options: { mergeBy: ['key'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(first.data.key).toBe('theme')
    expect(first.data.value).toBe('dark')

    // Update the same setting
    const second = await db.records.upsert({
      label: 'Setting',
      data: { key: 'theme', value: 'light', tenantId },
      options: { mergeBy: ['key'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(second.data.key).toBe('theme')
    expect(second.data.value).toBe('light')
    expect(second.data.tenantId).toBe(tenantId)
    // Same ID confirms it's an update
    expect(first.id()).toBe(second.id())

    // Cleanup
    await db.records.delete({ labels: ['Setting'], where: { tenantId } })
  })

  it('handles append strategy with nested properties', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const first = await db.records.upsert({
      label: 'Customer',
      data: {
        customerId: 'CUST-1',
        firstName: 'Jane',
        lastName: 'Smith',
        email: 'jane@example.com',
        tenantId
      },
      options: { mergeBy: ['customerId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(first.data.firstName).toBe('Jane')
    expect(first.data.lastName).toBe('Smith')
    expect(first.data.email).toBe('jane@example.com')

    // Partial update - add phone, keep other fields
    const second = await db.records.upsert({
      label: 'Customer',
      data: { customerId: 'CUST-1', phone: '555-1234', tenantId },
      options: { mergeBy: ['customerId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(second.data.customerId).toBe('CUST-1')
    expect(second.data.firstName).toBe('Jane')
    expect(second.data.lastName).toBe('Smith')
    expect(second.data.email).toBe('jane@example.com')
    expect(second.data.phone).toBe('555-1234')

    // Cleanup
    await db.records.delete({ labels: ['Customer'], where: { tenantId } })
  })

  it('handles rewrite strategy removing all unspecified fields', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const first = await db.records.upsert({
      label: 'Profile',
      data: { userId: 'USER-1', bio: 'Developer', website: 'example.com', twitter: '@user', tenantId },
      options: { mergeBy: ['userId', 'tenantId'], mergeStrategy: 'rewrite', suggestTypes: true }
    })

    expect(first.data.bio).toBe('Developer')
    expect(first.data.website).toBe('example.com')
    expect(first.data.twitter).toBe('@user')

    // Rewrite with only bio - should remove website and twitter
    const second = await db.records.upsert({
      label: 'Profile',
      data: { userId: 'USER-1', bio: 'Senior Developer', tenantId },
      options: { mergeBy: ['userId', 'tenantId'], mergeStrategy: 'rewrite', suggestTypes: true }
    })

    expect(second.data.userId).toBe('USER-1')
    expect(second.data.bio).toBe('Senior Developer')
    expect(second.data.website).toBeUndefined()
    expect(second.data.twitter).toBeUndefined()
    expect(second.data.tenantId).toBe(tenantId)

    // Cleanup
    await db.records.delete({ labels: ['Profile'], where: { tenantId } })
  })

  it('handles multiple sequential upserts with append strategy', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const first = await db.records.upsert({
      label: 'Inventory',
      data: { itemCode: 'ITEM-001', quantity: 10, tenantId },
      options: { mergeBy: ['itemCode', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(first.data.quantity).toBe(10)

    const second = await db.records.upsert({
      label: 'Inventory',
      data: { itemCode: 'ITEM-001', quantity: 15, location: 'Warehouse A', tenantId },
      options: { mergeBy: ['itemCode', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(second.data.quantity).toBe(15)
    expect(second.data.location).toBe('Warehouse A')

    const third = await db.records.upsert({
      label: 'Inventory',
      data: { itemCode: 'ITEM-001', supplier: 'ABC Corp', tenantId },
      options: { mergeBy: ['itemCode', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(third.data.quantity).toBe(15)
    expect(third.data.location).toBe('Warehouse A')
    expect(third.data.supplier).toBe('ABC Corp')

    // All should be the same record
    expect(first.id()).toBe(second.id())
    expect(second.id()).toBe(third.id())

    // Cleanup
    await db.records.delete({ labels: ['Inventory'], where: { tenantId } })
  })

  it('handles upsert with different data types', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const result = await db.records.upsert({
      label: 'Config',
      data: {
        configId: 'config-1',
        enabled: true,
        maxRetries: 3,
        timeout: 30.5,
        tags: ['production', 'critical'],
        tenantId
      },
      options: { mergeBy: ['configId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(result.data.configId).toBe('config-1')
    expect(result.data.enabled).toBe(true)
    expect(result.data.maxRetries).toBe(3)
    expect(result.data.timeout).toBe(30.5)
    expect(result.data.tags).toEqual(['production', 'critical'])

    // Update with different values
    const updated = await db.records.upsert({
      label: 'Config',
      data: {
        configId: 'config-1',
        enabled: false,
        maxRetries: 5,
        tenantId
      },
      options: { mergeBy: ['configId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(updated.data.enabled).toBe(false)
    expect(updated.data.maxRetries).toBe(5)
    expect(updated.data.timeout).toBe(30.5) // Should still exist with append
    expect(updated.data.tags).toEqual(['production', 'critical']) // Should still exist with append

    // Cleanup
    await db.records.delete({ labels: ['Config'], where: { tenantId } })
  })

  it('handles upsert without label specified', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    const result = await db.records.upsert({
      data: { uniqueId: 'unlabeled-1', value: 'test', tenantId },
      options: { mergeBy: ['uniqueId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(result.data.uniqueId).toBe('unlabeled-1')
    expect(result.data.value).toBe('test')

    // Update without label
    const updated = await db.records.upsert({
      data: { uniqueId: 'unlabeled-1', value: 'updated', tenantId },
      options: { mergeBy: ['uniqueId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(updated.data.value).toBe('updated')
    expect(result.id()).toBe(updated.id())

    // Cleanup
    await db.records.delete({ where: { tenantId } })
  })

  it('handles switching between append and rewrite strategies', async () => {
    const tenantId = `upsert-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

    // Create with append
    const first = await db.records.upsert({
      label: 'Document',
      data: { docId: 'DOC-1', title: 'My Document', content: 'Initial content', version: 1, tenantId },
      options: { mergeBy: ['docId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(first.data.title).toBe('My Document')
    expect(first.data.content).toBe('Initial content')
    expect(first.data.version).toBe(1)

    // Update with append - add field
    const second = await db.records.upsert({
      label: 'Document',
      data: { docId: 'DOC-1', content: 'Updated content', author: 'John', tenantId },
      options: { mergeBy: ['docId', 'tenantId'], mergeStrategy: 'append', suggestTypes: true }
    })

    expect(second.data.title).toBe('My Document') // Still there
    expect(second.data.content).toBe('Updated content')
    expect(second.data.author).toBe('John')
    expect(second.data.version).toBe(1) // Still there

    // Update with rewrite - remove unspecified fields
    const third = await db.records.upsert({
      label: 'Document',
      data: { docId: 'DOC-1', title: 'Final Document', version: 2, tenantId },
      options: { mergeBy: ['docId', 'tenantId'], mergeStrategy: 'rewrite', suggestTypes: true }
    })

    expect(third.data.title).toBe('Final Document')
    expect(third.data.version).toBe(2)
    expect(third.data.content).toBeUndefined() // Removed
    expect(third.data.author).toBeUndefined() // Removed

    // Cleanup
    await db.records.delete({ labels: ['Document'], where: { tenantId } })
  })
})
