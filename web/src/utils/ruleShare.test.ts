import { describe, it, expect } from 'vitest'
import { buildRuleShareText } from './ruleShare'

describe('buildRuleShareText', () => {
  it('includes the rule name, path, and yaml content', () => {
    const result = buildRuleShareText('invoice_rule', 'apiVersion: paporg.io/v1')

    expect(result).toContain('Paporg rule: invoice_rule')
    expect(result).toContain('rules/invoice_rule.yaml')
    expect(result).toContain('apiVersion: paporg.io/v1')
  })

  it('trims inputs and falls back when yaml is missing', () => {
    const result = buildRuleShareText('  receipts  ', '   ')

    expect(result).toContain('Paporg rule: receipts')
    expect(result).toContain('# Rule YAML missing')
  })
})
