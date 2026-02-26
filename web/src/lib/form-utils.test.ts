import { describe, it, expect } from 'vitest'
import { z } from 'zod'
import { zodFormValidator } from './form-utils'

describe('zodFormValidator', () => {
  const simpleSchema = z.object({
    name: z.string().min(1, 'Name is required'),
    age: z.number().min(0, 'Age must be non-negative'),
  })

  const validator = zodFormValidator(simpleSchema)

  it('returns undefined for valid input', () => {
    const result = validator({ value: { name: 'Alice', age: 30 } })
    expect(result).toBeUndefined()
  })

  it('returns field error for single invalid field', () => {
    const result = validator({ value: { name: '', age: 30 } })
    expect(result).toBeDefined()
    expect(result!.fields['name']).toBe('Name is required')
  })

  it('returns field errors for multiple invalid fields', () => {
    const result = validator({ value: { name: '', age: -1 } })
    expect(result).toBeDefined()
    expect(result!.fields['name']).toBe('Name is required')
    expect(result!.fields['age']).toBe('Age must be non-negative')
  })

  it('handles nested field paths', () => {
    const nestedSchema = z.object({
      profile: z.object({
        email: z.string().email('Invalid email'),
      }),
    })
    const nestedValidator = zodFormValidator(nestedSchema)

    const result = nestedValidator({ value: { profile: { email: 'not-an-email' } } })
    expect(result).toBeDefined()
    expect(result!.fields['profile.email']).toBe('Invalid email')
  })

  it('returns root-level refinement errors as form error', () => {
    const refinedSchema = z.object({
      password: z.string(),
      confirm: z.string(),
    }).refine(
      (data) => data.password === data.confirm,
      { message: 'Passwords must match' }
    )
    const refinedValidator = zodFormValidator(refinedSchema)

    const result = refinedValidator({ value: { password: 'abc', confirm: 'xyz' } })
    expect(result).toBeDefined()
    expect(result!.form).toBe('Passwords must match')
  })

  it('returns both field and form errors when both present', () => {
    const schema = z.object({
      password: z.string().min(8, 'Password too short'),
      confirm: z.string(),
    }).refine(
      (data) => data.password === data.confirm,
      { message: 'Passwords must match' }
    )
    const v = zodFormValidator(schema)

    const result = v({ value: { password: 'ab', confirm: 'xyz' } })
    expect(result).toBeDefined()
    expect(result!.fields['password']).toBe('Password too short')
    expect(result!.form).toBe('Passwords must match')
  })

  it('keeps only first error per field path', () => {
    const schema = z.object({
      email: z.string()
        .min(1, 'Email is required')
        .email('Invalid email'),
    })
    const v = zodFormValidator(schema)

    // Empty string triggers both min and email errors
    const result = v({ value: { email: '' } })
    expect(result).toBeDefined()
    // Only the first error for 'email' is kept
    expect(result!.fields['email']).toBe('Email is required')
  })

  it('joins multiple root-level errors with semicolons', () => {
    const schema = z.object({
      a: z.string(),
      b: z.string(),
    }).superRefine((data, ctx) => {
      if (!data.a && !data.b) {
        ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Need a' })
        ctx.addIssue({ code: z.ZodIssueCode.custom, message: 'Need b' })
      }
    })
    const v = zodFormValidator(schema)

    const result = v({ value: { a: '', b: '' } })
    expect(result).toBeDefined()
    expect(result!.form).toBe('Need a; Need b')
  })

  it('works with schema that uses .default()', () => {
    const schema = z.object({
      count: z.number().default(0),
      label: z.string().min(1, 'Label required'),
    })
    const v = zodFormValidator(schema)

    // Valid with default
    expect(v({ value: { count: 5, label: 'test' } })).toBeUndefined()

    // Invalid label
    const result = v({ value: { count: 5, label: '' } })
    expect(result).toBeDefined()
    expect(result!.fields['label']).toBe('Label required')
  })
})
