/** AI-related types for rule suggestions. */

/** AI status information. */
export interface AiStatus {
  /** Whether AI is available. */
  available: boolean
  /** Whether the model is loaded. */
  modelLoaded: boolean
  /** Friendly model name. */
  modelName?: string
  /** Error message if any. */
  error?: string
}

/** Request for rule suggestions. */
export interface SuggestRuleRequest {
  /** OCR text from the document. */
  ocrText: string
  /** Original filename. */
  filename: string
}

/** Summary of an existing rule for AI context. */
export interface ExistingRuleSummary {
  /** Rule name/ID. */
  name: string
  /** Category this rule assigns. */
  category: string
  /** Match type. */
  matchType: string
  /** Current match values. */
  matchValues: string[]
}

/** A suggested rule from the AI. */
export interface RuleSuggestion {
  /** Suggested rule name. */
  name?: string
  /** Suggested category name (slugified, lowercase with hyphens). */
  category: string
  /** Match type: "contains", "containsAny", "containsAll", or "pattern". */
  matchType: 'contains' | 'containsAny' | 'containsAll' | 'pattern'
  /** Match value (string or array depending on matchType). */
  matchValue: string | string[]
  /** Confidence score (0.0 - 1.0). */
  confidence: number
  /** Brief explanation of why this rule was suggested. */
  explanation?: string
  /** Brief explanation of why this rule was suggested (alias for explanation). */
  reasoning?: string
  /** Suggested output directory with variables. */
  outputDirectory?: string
  /** Suggested output filename with variables. */
  outputFilename?: string
  /** Whether this is an update to an existing rule (vs creating new). */
  isUpdate?: boolean
  /** Name of the existing rule to update (if isUpdate is true). */
  updateRuleName?: string
  /** Values to add to the existing rule (for containsAny updates). */
  addValues?: string[]
}
