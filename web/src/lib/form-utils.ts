import type { z } from 'zod'
import type { ReactFormExtendedApi } from '@tanstack/react-form'

/**
 * A React form API type with all generics relaxed for use as component props.
 *
 * We use Pick to select only the properties form components actually need.
 * This avoids invariance issues with ReactFormExtendedApi's generic parameters
 * (array-manipulation methods produce `never` types when TFormData=any) and
 * prevents TS2589 deep instantiation errors with recursive types like MatchCondition.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type BaseFormApi = ReactFormExtendedApi<any, any, any, any, any, any, any, any, any, any, any, any>

export type FormInstance = Pick<
  BaseFormApi,
  'Field' | 'Subscribe' | 'store' | 'state' | 'getFieldValue' | 'setFieldValue' | 'reset' | 'handleSubmit'
>

/**
 * Wraps a Zod schema as a TanStack Form function validator with per-field error distribution.
 *
 * This solves the type mismatch when Zod schemas use `.default()`, which causes
 * the Standard Schema input type to differ from the form data type (output type).
 * By using a function validator, we bypass the StandardSchemaV1<TFormData> constraint
 * while still distributing validation errors to individual fields.
 */
export function zodFormValidator<TFormData>(schema: z.ZodType<TFormData>) {
  return ({ value }: { value: TFormData }) => {
    const result = schema.safeParse(value)
    if (result.success) return undefined

    const fields: Record<string, string> = {}
    const formErrors: string[] = []
    for (const issue of result.error.issues) {
      const path = issue.path.join('.')
      if (!path) {
        formErrors.push(issue.message)
      } else if (!fields[path]) {
        fields[path] = issue.message
      }
    }

    if (formErrors.length > 0) {
      return { fields, form: formErrors.join('; ') }
    }
    return { fields }
  }
}
