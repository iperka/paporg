const EMPTY_YAML_PLACEHOLDER = '# Rule YAML missing'

export function buildRuleShareText(ruleName: string, yaml: string): string {
  const trimmedName = ruleName.trim()
  const safeName = trimmedName.length > 0 ? trimmedName : 'unnamed-rule'
  const trimmedYaml = yaml.trim()
  const safeYaml = trimmedYaml.length > 0 ? trimmedYaml : EMPTY_YAML_PLACEHOLDER

  return [
    `Paporg rule: ${safeName}`,
    '',
    'Paste into your config at:',
    `rules/${safeName}.yaml`,
    '',
    safeYaml,
    '',
    'How to use:',
    '1. Save the file under your Paporg config directory.',
    '2. Reload config in the app (Settings -> Reload).',
  ].join('\n')
}
