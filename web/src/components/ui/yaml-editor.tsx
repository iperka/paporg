import CodeMirror from '@uiw/react-codemirror'
import { yaml } from '@codemirror/lang-yaml'
import { oneDark } from '@codemirror/theme-one-dark'

interface YamlEditorProps {
  value: string
  onChange: (value: string) => void
  height?: string
  placeholder?: string
  readOnly?: boolean
}

export function YamlEditor({
  value,
  onChange,
  height = '400px',
  placeholder,
  readOnly = false,
}: YamlEditorProps) {
  return (
    <CodeMirror
      value={value}
      height={height}
      extensions={[yaml()]}
      theme={oneDark}
      onChange={onChange}
      readOnly={readOnly}
      placeholder={placeholder}
      basicSetup={{
        lineNumbers: true,
        foldGutter: true,
        highlightActiveLine: true,
        bracketMatching: true,
      }}
    />
  )
}
