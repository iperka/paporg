import { describe, it, expect } from 'vitest'
import {
  outputSettingsSchema,
  ocrSettingsSchema,
  gitAuthSettingsSchema,
  gitSettingsSchema,
  settingsSpecSchema,
  variableSpecSchema,
  ruleSpecSchema,
  matchConditionSchema,
  simpleMatchSchema,
  objectMetaSchema,
  settingsResourceSchema,
  variableResourceSchema,
  ruleResourceSchema,
  importSourceSpecSchema,
  localSourceConfigSchema,
  emailSourceConfigSchema,
  emailAuthSettingsSchema,
  fileFiltersSchema,
  attachmentFiltersSchema,
  importSourceResourceSchema,
  releaseChannelSchema,
  symlinkSettingsSchema,
  createDefaultSettingsSpec,
  createDefaultVariableSpec,
  createDefaultRuleSpec,
  createDefaultImportSourceSpec,
  createDefaultEmailSourceConfig,
  createDefaultMatchCondition,
  getMatchConditionType,
  createMatchConditionOfType,
  isSimpleMatch,
  isCompoundMatch,
  validateRegexPattern,
} from './resources'

// ============================================
// Default factories pass their own schemas
// ============================================

describe('default factory functions', () => {
  it('createDefaultSettingsSpec produces valid SettingsSpec', () => {
    const spec = createDefaultSettingsSpec()
    const result = settingsSpecSchema.safeParse(spec)
    expect(result.success).toBe(true)
  })

  it('createDefaultVariableSpec produces valid VariableSpec', () => {
    const spec = createDefaultVariableSpec()
    const result = variableSpecSchema.safeParse(spec)
    expect(result.success).toBe(true)
  })

  it('createDefaultRuleSpec produces valid RuleSpec (except empty required fields)', () => {
    const spec = createDefaultRuleSpec()
    // category is empty string, which fails min(1) - this is intentional for form defaults
    const result = ruleSpecSchema.safeParse(spec)
    expect(result.success).toBe(false)
  })

  it('createDefaultImportSourceSpec("local") produces valid ImportSourceSpec', () => {
    const spec = createDefaultImportSourceSpec('local')
    const result = importSourceSpecSchema.safeParse(spec)
    // local.path is empty, so validation fails - intentional for forms
    expect(result.success).toBe(false)
    expect(spec.type).toBe('local')
    expect(spec.local).toBeDefined()
  })

  it('createDefaultImportSourceSpec("email") returns email type', () => {
    const spec = createDefaultImportSourceSpec('email')
    expect(spec.type).toBe('email')
    expect(spec.email).toBeDefined()
  })

  it('createDefaultEmailSourceConfig returns correct structure', () => {
    const config = createDefaultEmailSourceConfig()
    expect(config.host).toBe('')
    expect(config.port).toBe(993)
    expect(config.useTls).toBe(true)
    expect(config.auth.type).toBe('password')
  })

  it('createDefaultMatchCondition returns { contains: "" }', () => {
    const cond = createDefaultMatchCondition()
    expect(cond).toEqual({ contains: '' })
  })
})

// ============================================
// outputSettingsSchema
// ============================================

describe('outputSettingsSchema', () => {
  it('accepts valid output settings', () => {
    expect(outputSettingsSchema.safeParse({ directory: 'docs', filename: 'test' }).success).toBe(true)
  })

  it('rejects empty directory', () => {
    expect(outputSettingsSchema.safeParse({ directory: '', filename: 'test' }).success).toBe(false)
  })

  it('rejects empty filename', () => {
    expect(outputSettingsSchema.safeParse({ directory: 'docs', filename: '' }).success).toBe(false)
  })

  it('rejects missing fields', () => {
    expect(outputSettingsSchema.safeParse({}).success).toBe(false)
  })
})

// ============================================
// ocrSettingsSchema
// ============================================

describe('ocrSettingsSchema', () => {
  it('accepts valid OCR settings', () => {
    expect(ocrSettingsSchema.safeParse({ enabled: true, languages: ['eng'], dpi: 300 }).success).toBe(true)
  })

  it('rejects empty languages array', () => {
    expect(ocrSettingsSchema.safeParse({ enabled: true, languages: [], dpi: 300 }).success).toBe(false)
  })

  it('rejects dpi below 72', () => {
    expect(ocrSettingsSchema.safeParse({ enabled: true, languages: ['eng'], dpi: 50 }).success).toBe(false)
  })

  it('rejects dpi above 600', () => {
    expect(ocrSettingsSchema.safeParse({ enabled: true, languages: ['eng'], dpi: 700 }).success).toBe(false)
  })
})

// ============================================
// gitAuthSettingsSchema
// ============================================

describe('gitAuthSettingsSchema', () => {
  it('accepts "none" type', () => {
    expect(gitAuthSettingsSchema.safeParse({ type: 'none' }).success).toBe(true)
  })

  it('accepts "token" type', () => {
    expect(gitAuthSettingsSchema.safeParse({ type: 'token', tokenEnvVar: 'GH_TOKEN' }).success).toBe(true)
  })

  it('accepts "ssh-key" type', () => {
    expect(gitAuthSettingsSchema.safeParse({ type: 'ssh-key', sshKeyPath: '~/.ssh/id_ed25519' }).success).toBe(true)
  })

  it('rejects invalid type', () => {
    expect(gitAuthSettingsSchema.safeParse({ type: 'basic' }).success).toBe(false)
  })
})

// ============================================
// objectMetaSchema
// ============================================

describe('objectMetaSchema', () => {
  it('accepts valid name', () => {
    expect(objectMetaSchema.safeParse({ name: 'my-resource' }).success).toBe(true)
  })

  it('accepts name starting with underscore', () => {
    expect(objectMetaSchema.safeParse({ name: '_internal' }).success).toBe(true)
  })

  it('rejects empty name', () => {
    expect(objectMetaSchema.safeParse({ name: '' }).success).toBe(false)
  })

  it('rejects name starting with digit', () => {
    expect(objectMetaSchema.safeParse({ name: '1bad' }).success).toBe(false)
  })

  it('rejects name with spaces', () => {
    expect(objectMetaSchema.safeParse({ name: 'my resource' }).success).toBe(false)
  })

  it('rejects name with special characters', () => {
    expect(objectMetaSchema.safeParse({ name: 'my.resource' }).success).toBe(false)
  })

  it('defaults labels and annotations to empty objects', () => {
    const result = objectMetaSchema.parse({ name: 'test' })
    expect(result.labels).toEqual({})
    expect(result.annotations).toEqual({})
  })
})

// ============================================
// matchConditionSchema (recursive)
// ============================================

describe('matchConditionSchema', () => {
  it('accepts { contains }', () => {
    expect(matchConditionSchema.safeParse({ contains: 'invoice' }).success).toBe(true)
  })

  it('accepts { containsAny }', () => {
    expect(matchConditionSchema.safeParse({ containsAny: ['a', 'b'] }).success).toBe(true)
  })

  it('rejects empty containsAny', () => {
    expect(matchConditionSchema.safeParse({ containsAny: [] }).success).toBe(false)
  })

  it('accepts { containsAll }', () => {
    expect(matchConditionSchema.safeParse({ containsAll: ['a', 'b'] }).success).toBe(true)
  })

  it('rejects empty containsAll', () => {
    expect(matchConditionSchema.safeParse({ containsAll: [] }).success).toBe(false)
  })

  it('accepts { pattern }', () => {
    expect(matchConditionSchema.safeParse({ pattern: '\\d+' }).success).toBe(true)
  })

  it('accepts nested { all } with children', () => {
    const cond = {
      all: [
        { contains: 'invoice' },
        { pattern: '\\d{4}' },
      ],
    }
    expect(matchConditionSchema.safeParse(cond).success).toBe(true)
  })

  it('accepts nested { any } with children', () => {
    const cond = {
      any: [
        { contains: 'receipt' },
        { contains: 'invoice' },
      ],
    }
    expect(matchConditionSchema.safeParse(cond).success).toBe(true)
  })

  it('accepts { not } wrapping a simple condition', () => {
    const cond = { not: { contains: 'spam' } }
    expect(matchConditionSchema.safeParse(cond).success).toBe(true)
  })

  it('accepts deeply nested conditions', () => {
    const cond = {
      all: [
        { any: [{ contains: 'a' }, { not: { pattern: 'b' } }] },
        { contains: 'c' },
      ],
    }
    expect(matchConditionSchema.safeParse(cond).success).toBe(true)
  })

  it('accepts caseSensitive flag on simple match', () => {
    expect(matchConditionSchema.safeParse({ contains: 'test', caseSensitive: true }).success).toBe(true)
  })

  it('accepts caseSensitive flag on compound match', () => {
    const cond = { all: [{ contains: 'test' }], caseSensitive: false }
    expect(matchConditionSchema.safeParse(cond).success).toBe(true)
  })

  it('rejects empty object', () => {
    expect(matchConditionSchema.safeParse({}).success).toBe(false)
  })
})

// ============================================
// simpleMatchSchema refinement
// ============================================

describe('simpleMatchSchema', () => {
  it('accepts exactly one match type', () => {
    expect(simpleMatchSchema.safeParse({ contains: 'test' }).success).toBe(true)
  })

  it('rejects when no match type specified', () => {
    expect(simpleMatchSchema.safeParse({}).success).toBe(false)
  })

  it('rejects when multiple match types specified', () => {
    expect(simpleMatchSchema.safeParse({ contains: 'a', pattern: 'b' }).success).toBe(false)
  })

  it('allows caseSensitive alongside a single match type', () => {
    expect(simpleMatchSchema.safeParse({ contains: 'test', caseSensitive: true }).success).toBe(true)
  })
})

// ============================================
// ruleSpecSchema
// ============================================

describe('ruleSpecSchema', () => {
  const validRule = {
    priority: 10,
    category: 'invoices',
    match: { contains: 'invoice' },
    output: { directory: '$y/invoices', filename: '$original' },
  }

  it('accepts valid rule spec', () => {
    expect(ruleSpecSchema.safeParse(validRule).success).toBe(true)
  })

  it('defaults priority to 0', () => {
    const rest = { category: validRule.category, match: validRule.match, output: validRule.output }
    const result = ruleSpecSchema.parse(rest)
    expect(result.priority).toBe(0)
  })

  it('defaults symlinks to empty array', () => {
    const result = ruleSpecSchema.parse(validRule)
    expect(result.symlinks).toEqual([])
  })

  it('rejects empty category', () => {
    expect(ruleSpecSchema.safeParse({ ...validRule, category: '' }).success).toBe(false)
  })
})

// ============================================
// symlinkSettingsSchema
// ============================================

describe('symlinkSettingsSchema', () => {
  it('accepts valid target', () => {
    expect(symlinkSettingsSchema.safeParse({ target: '/link/path' }).success).toBe(true)
  })

  it('rejects empty target', () => {
    expect(symlinkSettingsSchema.safeParse({ target: '' }).success).toBe(false)
  })
})

// ============================================
// Full resource schemas
// ============================================

describe('settingsResourceSchema', () => {
  it('accepts valid settings resource', () => {
    const resource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Settings',
      metadata: { name: 'default' },
      spec: createDefaultSettingsSpec(),
    }
    expect(settingsResourceSchema.safeParse(resource).success).toBe(true)
  })

  it('rejects wrong kind', () => {
    const resource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Rule',
      metadata: { name: 'default' },
      spec: createDefaultSettingsSpec(),
    }
    expect(settingsResourceSchema.safeParse(resource).success).toBe(false)
  })

  it('rejects wrong apiVersion', () => {
    const resource = {
      apiVersion: 'v2',
      kind: 'Settings',
      metadata: { name: 'default' },
      spec: createDefaultSettingsSpec(),
    }
    expect(settingsResourceSchema.safeParse(resource).success).toBe(false)
  })
})

describe('variableResourceSchema', () => {
  it('accepts valid variable resource', () => {
    const resource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Variable',
      metadata: { name: 'year' },
      spec: { pattern: '(?P<value>\\d{4})' },
    }
    expect(variableResourceSchema.safeParse(resource).success).toBe(true)
  })
})

describe('ruleResourceSchema', () => {
  it('accepts valid rule resource', () => {
    const resource = {
      apiVersion: 'paporg.io/v1',
      kind: 'Rule',
      metadata: { name: 'invoices' },
      spec: {
        priority: 10,
        category: 'invoices',
        match: { contains: 'invoice' },
        output: { directory: '$y/invoices', filename: '$original' },
      },
    }
    expect(ruleResourceSchema.safeParse(resource).success).toBe(true)
  })
})

// ============================================
// emailAuthSettingsSchema (superRefine)
// ============================================

describe('emailAuthSettingsSchema', () => {
  it('accepts password auth with passwordEnvVar', () => {
    const result = emailAuthSettingsSchema.safeParse({
      type: 'password',
      passwordEnvVar: 'EMAIL_PASS',
    })
    expect(result.success).toBe(true)
  })

  it('accepts password auth with passwordInsecure', () => {
    const result = emailAuthSettingsSchema.safeParse({
      type: 'password',
      passwordInsecure: 'secret123',
    })
    expect(result.success).toBe(true)
  })

  it('accepts password auth with passwordFile', () => {
    const result = emailAuthSettingsSchema.safeParse({
      type: 'password',
      passwordFile: '/run/secrets/email_pass',
    })
    expect(result.success).toBe(true)
  })

  it('rejects password auth with no credentials', () => {
    const result = emailAuthSettingsSchema.safeParse({
      type: 'password',
    })
    expect(result.success).toBe(false)
  })

  it('rejects password auth with empty credential strings', () => {
    const result = emailAuthSettingsSchema.safeParse({
      type: 'password',
      passwordEnvVar: '',
      passwordInsecure: '',
      passwordFile: '',
    })
    expect(result.success).toBe(false)
  })

  it('accepts oauth2 auth with oauth2 settings', () => {
    const result = emailAuthSettingsSchema.safeParse({
      type: 'oauth2',
      oauth2: { provider: 'gmail' },
    })
    expect(result.success).toBe(true)
  })

  it('rejects oauth2 auth without oauth2 settings', () => {
    const result = emailAuthSettingsSchema.safeParse({
      type: 'oauth2',
    })
    expect(result.success).toBe(false)
  })
})

// ============================================
// emailSourceConfigSchema
// ============================================

describe('emailSourceConfigSchema', () => {
  const validEmailConfig = {
    host: 'imap.gmail.com',
    port: 993,
    useTls: true,
    username: 'user@gmail.com',
    auth: { type: 'password' as const, passwordEnvVar: 'PASS' },
  }

  it('accepts valid email source config', () => {
    expect(emailSourceConfigSchema.safeParse(validEmailConfig).success).toBe(true)
  })

  it('rejects empty host', () => {
    expect(emailSourceConfigSchema.safeParse({ ...validEmailConfig, host: '' }).success).toBe(false)
  })

  it('rejects port out of range', () => {
    expect(emailSourceConfigSchema.safeParse({ ...validEmailConfig, port: 0 }).success).toBe(false)
    expect(emailSourceConfigSchema.safeParse({ ...validEmailConfig, port: 70000 }).success).toBe(false)
  })

  it('rejects minAttachmentSize > maxAttachmentSize', () => {
    const result = emailSourceConfigSchema.safeParse({
      ...validEmailConfig,
      minAttachmentSize: 1000,
      maxAttachmentSize: 500,
    })
    expect(result.success).toBe(false)
  })
})

// ============================================
// importSourceSpecSchema
// ============================================

describe('importSourceSpecSchema', () => {
  it('accepts valid local import source', () => {
    const result = importSourceSpecSchema.safeParse({
      type: 'local',
      enabled: true,
      local: { path: '/data/inbox', recursive: false, filters: { include: ['*'], exclude: [] }, pollInterval: 60 },
    })
    expect(result.success).toBe(true)
  })

  it('rejects local type without local config', () => {
    const result = importSourceSpecSchema.safeParse({
      type: 'local',
      enabled: true,
    })
    expect(result.success).toBe(false)
  })

  it('rejects email type without email config', () => {
    const result = importSourceSpecSchema.safeParse({
      type: 'email',
      enabled: true,
    })
    expect(result.success).toBe(false)
  })
})

// ============================================
// importSourceResourceSchema
// ============================================

describe('importSourceResourceSchema', () => {
  it('accepts valid import source resource', () => {
    const resource = {
      apiVersion: 'paporg.io/v1',
      kind: 'ImportSource',
      metadata: { name: 'local-inbox' },
      spec: {
        type: 'local',
        enabled: true,
        local: {
          path: '/data/inbox',
          recursive: false,
          filters: { include: ['*'], exclude: [] },
          pollInterval: 60,
        },
      },
    }
    expect(importSourceResourceSchema.safeParse(resource).success).toBe(true)
  })
})

// ============================================
// localSourceConfigSchema
// ============================================

describe('localSourceConfigSchema', () => {
  it('accepts valid config', () => {
    expect(localSourceConfigSchema.safeParse({
      path: '/inbox',
      recursive: true,
      filters: { include: ['*.pdf'], exclude: [] },
      pollInterval: 120,
    }).success).toBe(true)
  })

  it('rejects empty path', () => {
    expect(localSourceConfigSchema.safeParse({ path: '' }).success).toBe(false)
  })

  it('rejects pollInterval below 1', () => {
    expect(localSourceConfigSchema.safeParse({ path: '/inbox', pollInterval: 0 }).success).toBe(false)
  })

  it('rejects pollInterval above 86400', () => {
    expect(localSourceConfigSchema.safeParse({ path: '/inbox', pollInterval: 100000 }).success).toBe(false)
  })
})

// ============================================
// fileFiltersSchema
// ============================================

describe('fileFiltersSchema', () => {
  it('defaults include to ["*"] and exclude to []', () => {
    const result = fileFiltersSchema.parse({})
    expect(result.include).toEqual(['*'])
    expect(result.exclude).toEqual([])
  })
})

// ============================================
// attachmentFiltersSchema
// ============================================

describe('attachmentFiltersSchema', () => {
  it('defaults all arrays to empty', () => {
    const result = attachmentFiltersSchema.parse({})
    expect(result.include).toEqual([])
    expect(result.exclude).toEqual([])
    expect(result.filenameInclude).toEqual([])
    expect(result.filenameExclude).toEqual([])
  })
})

// ============================================
// releaseChannelSchema
// ============================================

describe('releaseChannelSchema', () => {
  it('accepts "stable"', () => {
    expect(releaseChannelSchema.safeParse('stable').success).toBe(true)
  })

  it('accepts "pre-release"', () => {
    expect(releaseChannelSchema.safeParse('pre-release').success).toBe(true)
  })

  it('rejects invalid channel', () => {
    expect(releaseChannelSchema.safeParse('nightly').success).toBe(false)
  })

  it('defaults to "stable"', () => {
    expect(releaseChannelSchema.parse(undefined)).toBe('stable')
  })
})

// ============================================
// variableSpecSchema
// ============================================

describe('variableSpecSchema', () => {
  it('accepts valid pattern', () => {
    expect(variableSpecSchema.safeParse({ pattern: '\\d+' }).success).toBe(true)
  })

  it('rejects empty pattern', () => {
    expect(variableSpecSchema.safeParse({ pattern: '' }).success).toBe(false)
  })

  it('rejects invalid regex pattern', () => {
    expect(variableSpecSchema.safeParse({ pattern: '(?P<bad' }).success).toBe(false)
  })

  it('accepts Rust-style named groups', () => {
    expect(variableSpecSchema.safeParse({ pattern: '(?P<year>\\d{4})' }).success).toBe(true)
  })

  it('accepts optional transform', () => {
    expect(variableSpecSchema.safeParse({ pattern: '\\w+', transform: 'uppercase' }).success).toBe(true)
  })

  it('rejects invalid transform', () => {
    expect(variableSpecSchema.safeParse({ pattern: '\\w+', transform: 'invalid' }).success).toBe(false)
  })
})

// ============================================
// settingsSpecSchema
// ============================================

describe('settingsSpecSchema', () => {
  it('rejects workerCount below 1', () => {
    const spec = { ...createDefaultSettingsSpec(), workerCount: 0 }
    expect(settingsSpecSchema.safeParse(spec).success).toBe(false)
  })

  it('rejects workerCount above 32', () => {
    const spec = { ...createDefaultSettingsSpec(), workerCount: 64 }
    expect(settingsSpecSchema.safeParse(spec).success).toBe(false)
  })

  it('rejects empty inputDirectory', () => {
    const spec = { ...createDefaultSettingsSpec(), inputDirectory: '' }
    expect(settingsSpecSchema.safeParse(spec).success).toBe(false)
  })

  it('rejects empty outputDirectory', () => {
    const spec = { ...createDefaultSettingsSpec(), outputDirectory: '' }
    expect(settingsSpecSchema.safeParse(spec).success).toBe(false)
  })
})

// ============================================
// gitSettingsSchema
// ============================================

describe('gitSettingsSchema', () => {
  it('defaults branch to "main"', () => {
    const result = gitSettingsSchema.parse({
      enabled: false,
      repository: '',
      auth: { type: 'none' },
    })
    expect(result.branch).toBe('main')
  })

  it('defaults syncInterval to 300', () => {
    const result = gitSettingsSchema.parse({
      enabled: false,
      repository: '',
      auth: { type: 'none' },
    })
    expect(result.syncInterval).toBe(300)
  })

  it('rejects negative syncInterval', () => {
    expect(gitSettingsSchema.safeParse({
      enabled: false,
      repository: '',
      auth: { type: 'none' },
      syncInterval: -1,
    }).success).toBe(false)
  })
})

// ============================================
// Match condition helpers
// ============================================

describe('getMatchConditionType', () => {
  it('identifies contains', () => {
    expect(getMatchConditionType({ contains: 'test' })).toBe('contains')
  })

  it('identifies containsAny', () => {
    expect(getMatchConditionType({ containsAny: ['a'] })).toBe('containsAny')
  })

  it('identifies containsAll', () => {
    expect(getMatchConditionType({ containsAll: ['a'] })).toBe('containsAll')
  })

  it('identifies pattern', () => {
    expect(getMatchConditionType({ pattern: '\\d+' })).toBe('pattern')
  })

  it('identifies all', () => {
    expect(getMatchConditionType({ all: [{ contains: '' }] })).toBe('all')
  })

  it('identifies any', () => {
    expect(getMatchConditionType({ any: [{ contains: '' }] })).toBe('any')
  })

  it('identifies not', () => {
    expect(getMatchConditionType({ not: { contains: '' } })).toBe('not')
  })
})

describe('createMatchConditionOfType', () => {
  it('creates contains condition', () => {
    expect(createMatchConditionOfType('contains')).toEqual({ contains: '' })
  })

  it('creates containsAny condition', () => {
    expect(createMatchConditionOfType('containsAny')).toEqual({ containsAny: [''] })
  })

  it('creates containsAll condition', () => {
    expect(createMatchConditionOfType('containsAll')).toEqual({ containsAll: [''] })
  })

  it('creates pattern condition', () => {
    expect(createMatchConditionOfType('pattern')).toEqual({ pattern: '' })
  })

  it('creates all condition', () => {
    expect(createMatchConditionOfType('all')).toEqual({ all: [{ contains: '' }] })
  })

  it('creates any condition', () => {
    expect(createMatchConditionOfType('any')).toEqual({ any: [{ contains: '' }] })
  })

  it('creates not condition', () => {
    expect(createMatchConditionOfType('not')).toEqual({ not: { contains: '' } })
  })
})

describe('isSimpleMatch', () => {
  it('returns true for contains', () => {
    expect(isSimpleMatch({ contains: 'test' })).toBe(true)
  })

  it('returns true for containsAny', () => {
    expect(isSimpleMatch({ containsAny: ['a'] })).toBe(true)
  })

  it('returns true for containsAll', () => {
    expect(isSimpleMatch({ containsAll: ['a'] })).toBe(true)
  })

  it('returns true for pattern', () => {
    expect(isSimpleMatch({ pattern: '\\d+' })).toBe(true)
  })

  it('returns false for all', () => {
    expect(isSimpleMatch({ all: [{ contains: '' }] })).toBe(false)
  })

  it('returns false for any', () => {
    expect(isSimpleMatch({ any: [{ contains: '' }] })).toBe(false)
  })

  it('returns false for not', () => {
    expect(isSimpleMatch({ not: { contains: '' } })).toBe(false)
  })
})

describe('isCompoundMatch', () => {
  it('returns true for all', () => {
    expect(isCompoundMatch({ all: [{ contains: '' }] })).toBe(true)
  })

  it('returns true for any', () => {
    expect(isCompoundMatch({ any: [{ contains: '' }] })).toBe(true)
  })

  it('returns true for not', () => {
    expect(isCompoundMatch({ not: { contains: '' } })).toBe(true)
  })

  it('returns false for contains', () => {
    expect(isCompoundMatch({ contains: 'test' })).toBe(false)
  })

  it('returns false for pattern', () => {
    expect(isCompoundMatch({ pattern: '\\d+' })).toBe(false)
  })
})

// ============================================
// validateRegexPattern
// ============================================

describe('validateRegexPattern', () => {
  it('returns valid for simple pattern', () => {
    expect(validateRegexPattern('\\d+')).toEqual({ valid: true })
  })

  it('returns valid for JS-style named groups', () => {
    expect(validateRegexPattern('(?<year>\\d{4})')).toEqual({ valid: true })
  })

  it('converts and validates Rust-style named groups', () => {
    expect(validateRegexPattern('(?P<year>\\d{4})')).toEqual({ valid: true })
  })

  it('returns invalid for unclosed group', () => {
    const result = validateRegexPattern('(?P<bad')
    expect(result.valid).toBe(false)
    expect(result.error).toBeDefined()
  })

  it('returns invalid for unbalanced brackets', () => {
    const result = validateRegexPattern('[abc')
    expect(result.valid).toBe(false)
    expect(result.error).toBeDefined()
  })

  it('returns valid for empty pattern', () => {
    expect(validateRegexPattern('')).toEqual({ valid: true })
  })

  it('returns valid for complex Rust regex with multiple named groups', () => {
    expect(validateRegexPattern('(?P<month>\\d{2})-(?P<day>\\d{2})-(?P<year>\\d{4})')).toEqual({ valid: true })
  })
})
