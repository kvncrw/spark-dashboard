import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { ModelCatalog } from '../components/ModelCatalog'

describe('ModelCatalog', () => {
  it('enumerates live models and configured aliases by estate', () => {
    render(<ModelCatalog targets={[
      {
        slot_id: 'spark-glm-head',
        hardware: 'DGX Spark',
        estate: 'spark',
        models: ['glm-5.2'],
        state: 'up',
        up: true,
        aliases: ['local-strong', 'spark-glm-5.2'],
        api_base: 'http://spark/v1',
      },
      {
        slot_id: 'coredump',
        hardware: 'RTX 5090',
        estate: 'coredump',
        models: [],
        state: 'down',
        up: false,
        aliases: ['local-qwen-coredump'],
        api_base: 'http://coredump',
      },
    ]} />)

    expect(screen.getByText('glm-5.2')).toBeTruthy()
    expect(screen.getByText('local-strong · spark-glm-5.2')).toBeTruthy()
    expect(screen.getByText('local-qwen-coredump')).toBeTruthy()
    expect(screen.getByText('coredump')).toBeTruthy()
  })
})
