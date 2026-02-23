import { useState, useMemo } from 'react'
import { Link } from '@tanstack/react-router'
import {
  HelpCircle,
  FolderInput,
  Variable,
  FileText,
  Briefcase,
  ArrowRight,
  ChevronRight,
  Lightbulb,
  Code,
  Zap,
} from 'lucide-react'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'

const flowSteps = [
  { id: 'source', label: 'Source', icon: FolderInput, description: 'Where documents come from' },
  { id: 'ocr', label: 'OCR', icon: Zap, description: 'Extract text from documents' },
  { id: 'variables', label: 'Variables', icon: Variable, description: 'Extract data with patterns' },
  { id: 'rules', label: 'Rules', icon: FileText, description: 'Categorize and route documents' },
  { id: 'output', label: 'Output', icon: Briefcase, description: 'Organized file storage' },
]

// Static data moved outside component - dynamic date examples computed inside component
const staticBuiltInVariables = [
  { name: '$original', description: 'Original filename without extension', example: 'invoice_scan' },
  { name: '$uuid', description: 'Unique identifier', example: 'a1b2c3d4-...' },
]

const matchConditionTypes = [
  { type: 'contains', description: 'Document contains exact text', example: 'contains: "Invoice"' },
  { type: 'containsAny', description: 'Contains any of the specified texts', example: 'containsAny: ["Invoice", "Bill"]' },
  { type: 'containsAll', description: 'Contains all of the specified texts', example: 'containsAll: ["Invoice", "VAT"]' },
  { type: 'pattern', description: 'Matches a regex pattern', example: 'pattern: "INV-\\d+"' },
  { type: 'all', description: 'All conditions must match (AND)', example: 'all: [{...}, {...}]' },
  { type: 'any', description: 'Any condition can match (OR)', example: 'any: [{...}, {...}]' },
  { type: 'not', description: 'Condition must NOT match', example: 'not: {contains: "Draft"}' },
]

const transforms = [
  { name: 'slugify', description: 'Convert to URL-friendly format', example: 'Invoice #123 → invoice-123' },
  { name: 'uppercase', description: 'Convert to uppercase', example: 'invoice → INVOICE' },
  { name: 'lowercase', description: 'Convert to lowercase', example: 'INVOICE → invoice' },
  { name: 'trim', description: 'Remove leading/trailing whitespace', example: '  text  → text' },
]

export function HelpPage() {
  const [activeStep, setActiveStep] = useState<string | null>(null)

  // Generate date-based examples inside component to keep them fresh
  const builtInVariables = useMemo(() => {
    const now = new Date()
    const currentYear = now.getFullYear().toString()
    const lastYear = (now.getFullYear() - 1).toString()
    const currentMonth = String(now.getMonth() + 1).padStart(2, '0')
    const currentDay = String(now.getDate()).padStart(2, '0')

    return [
      { name: '$y', description: 'Current year (4 digits)', example: currentYear },
      { name: '$l', description: 'Last year (4 digits)', example: lastYear },
      { name: '$m', description: 'Current month (2 digits)', example: currentMonth },
      { name: '$d', description: 'Current day (2 digits)', example: currentDay },
      ...staticBuiltInVariables,
      { name: '$timestamp', description: 'ISO 8601 timestamp', example: `${currentYear}-${currentMonth}-${currentDay}T14:30:00` },
    ]
  }, [])

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <HelpCircle className="h-8 w-8" />
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Help & Guide</h1>
          <p className="text-muted-foreground">
            Learn how to use Paporg to organize your documents automatically
          </p>
        </div>
      </div>

      {/* Interactive Flow Diagram */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Lightbulb className="h-5 w-5" />
            How Paporg Works
          </CardTitle>
          <CardDescription>
            Click on each step to learn more about the document processing flow
          </CardDescription>
        </CardHeader>
        <CardContent>
          <div className="flex flex-wrap items-center justify-center gap-2 md:gap-4 py-4">
            {flowSteps.map((step, index) => (
              <div key={step.id} className="flex items-center gap-2 md:gap-4">
                <button
                  onClick={() => setActiveStep(activeStep === step.id ? null : step.id)}
                  className={`flex flex-col items-center gap-2 p-4 rounded-lg border transition-all hover:bg-accent ${
                    activeStep === step.id
                      ? 'border-primary bg-accent'
                      : 'border-border hover:border-primary/50'
                  }`}
                >
                  <div className="p-3 rounded-full bg-muted">
                    <step.icon className="h-5 w-5" />
                  </div>
                  <span className="font-medium text-sm">{step.label}</span>
                </button>
                {index < flowSteps.length - 1 && (
                  <ChevronRight className="h-5 w-5 text-muted-foreground hidden md:block" />
                )}
              </div>
            ))}
          </div>

          {/* Step Details */}
          {activeStep && (
            <div className="mt-4 p-4 rounded-lg border bg-muted/30 animate-in fade-in slide-in-from-top-2">
              {activeStep === 'source' && (
                <div className="space-y-2">
                  <h4 className="font-semibold flex items-center gap-2">
                    <FolderInput className="h-4 w-4" /> Sources
                  </h4>
                  <p className="text-sm text-muted-foreground">
                    Sources define where Paporg looks for new documents. Configure a folder (like your Downloads)
                    and Paporg will automatically detect new files matching your patterns.
                  </p>
                  <Button variant="outline" size="sm" asChild>
                    <Link to="/sources">
                      Configure Sources <ArrowRight className="h-4 w-4 ml-2" />
                    </Link>
                  </Button>
                </div>
              )}
              {activeStep === 'ocr' && (
                <div className="space-y-2">
                  <h4 className="font-semibold flex items-center gap-2">
                    <Zap className="h-4 w-4" /> OCR Processing
                  </h4>
                  <p className="text-sm text-muted-foreground">
                    Optical Character Recognition extracts text from your documents, including scanned PDFs and images.
                    This text is then used for variable extraction and rule matching.
                  </p>
                </div>
              )}
              {activeStep === 'variables' && (
                <div className="space-y-2">
                  <h4 className="font-semibold flex items-center gap-2">
                    <Variable className="h-4 w-4" /> Variables
                  </h4>
                  <p className="text-sm text-muted-foreground">
                    Variables use regex patterns to extract specific data from documents, like invoice numbers,
                    dates, or vendor names. These values can then be used in your output file paths.
                  </p>
                  <Button variant="outline" size="sm" asChild>
                    <Link to="/variables">
                      Configure Variables <ArrowRight className="h-4 w-4 ml-2" />
                    </Link>
                  </Button>
                </div>
              )}
              {activeStep === 'rules' && (
                <div className="space-y-2">
                  <h4 className="font-semibold flex items-center gap-2">
                    <FileText className="h-4 w-4" /> Rules
                  </h4>
                  <p className="text-sm text-muted-foreground">
                    Rules determine how documents are categorized and where they're saved. Each rule has match
                    conditions and output templates using variables.
                  </p>
                  <Button variant="outline" size="sm" asChild>
                    <Link to="/rules">
                      Configure Rules <ArrowRight className="h-4 w-4 ml-2" />
                    </Link>
                  </Button>
                </div>
              )}
              {activeStep === 'output' && (
                <div className="space-y-2">
                  <h4 className="font-semibold flex items-center gap-2">
                    <Briefcase className="h-4 w-4" /> Output
                  </h4>
                  <p className="text-sm text-muted-foreground">
                    Documents are saved to the output directory with the path and filename defined by the matching rule.
                    Variables are replaced with actual values extracted from the document.
                  </p>
                  <Button variant="outline" size="sm" asChild>
                    <Link to="/jobs">
                      View Jobs <ArrowRight className="h-4 w-4 ml-2" />
                    </Link>
                  </Button>
                </div>
              )}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Detailed Tabs */}
      <Tabs defaultValue="getting-started" className="space-y-4">
        <TabsList className="grid w-full grid-cols-2 md:grid-cols-4">
          <TabsTrigger value="getting-started">Getting Started</TabsTrigger>
          <TabsTrigger value="concepts">Concepts</TabsTrigger>
          <TabsTrigger value="reference">Reference</TabsTrigger>
          <TabsTrigger value="examples">Examples</TabsTrigger>
        </TabsList>

        <TabsContent value="getting-started" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Quick Start Guide</CardTitle>
              <CardDescription>Get up and running in 3 simple steps</CardDescription>
            </CardHeader>
            <CardContent className="space-y-6">
              <div className="grid gap-4 md:grid-cols-3">
                <Card>
                  <CardHeader className="pb-2">
                    <div className="flex items-center gap-2">
                      <Badge variant="secondary">Step 1</Badge>
                      <FolderInput className="h-4 w-4" />
                    </div>
                    <CardTitle className="text-lg">Add Sources</CardTitle>
                  </CardHeader>
                  <CardContent className="text-sm text-muted-foreground">
                    <p>Configure where Paporg should look for documents. Add multiple sources - Downloads, email attachments, scanner output, etc.</p>
                    <Button variant="link" className="px-0 mt-2" asChild>
                      <Link to="/sources">Go to Sources <ArrowRight className="h-3 w-3 ml-1" /></Link>
                    </Button>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="pb-2">
                    <div className="flex items-center gap-2">
                      <Badge variant="secondary">Step 2</Badge>
                      <Variable className="h-4 w-4" />
                    </div>
                    <CardTitle className="text-lg">Define Variables</CardTitle>
                  </CardHeader>
                  <CardContent className="text-sm text-muted-foreground">
                    <p>Create patterns to extract data like invoice numbers, dates, or vendor names from your documents.</p>
                    <Button variant="link" className="px-0 mt-2" asChild>
                      <Link to="/variables">Go to Variables <ArrowRight className="h-3 w-3 ml-1" /></Link>
                    </Button>
                  </CardContent>
                </Card>

                <Card>
                  <CardHeader className="pb-2">
                    <div className="flex items-center gap-2">
                      <Badge variant="secondary">Step 3</Badge>
                      <FileText className="h-4 w-4" />
                    </div>
                    <CardTitle className="text-lg">Create Rules</CardTitle>
                  </CardHeader>
                  <CardContent className="text-sm text-muted-foreground">
                    <p>Set up multiple rules for different document types - invoices, contracts, receipts, etc. Each rule defines where matching documents go.</p>
                    <Button variant="link" className="px-0 mt-2" asChild>
                      <Link to="/rules">Go to Rules <ArrowRight className="h-3 w-3 ml-1" /></Link>
                    </Button>
                  </CardContent>
                </Card>
              </div>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="concepts" className="space-y-4">
          <Accordion type="single" collapsible className="w-full">
            <AccordionItem value="sources">
              <AccordionTrigger>
                <div className="flex items-center gap-2">
                  <FolderInput className="h-5 w-5" />
                  <span>Sources</span>
                </div>
              </AccordionTrigger>
              <AccordionContent className="space-y-4">
                <p>Sources define where Paporg imports documents from. They are your input locations.</p>
                <div className="rounded-lg border bg-muted/30 p-4">
                  <p className="text-sm font-medium mb-2">You can configure multiple sources</p>
                  <p className="text-sm text-muted-foreground">
                    Set up different sources for different document types - one for your Downloads folder,
                    another for email attachments, and another for scanned documents. Each source can have
                    its own file patterns and polling settings.
                  </p>
                </div>
                <div className="grid gap-2 text-sm">
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Path</Badge>
                    <span>Directory to watch for new files</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Recursive</Badge>
                    <span>Whether to include subdirectories</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Patterns</Badge>
                    <span>Glob patterns to filter files (e.g., *.pdf, *.png)</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Poll Interval</Badge>
                    <span>How often to check for new files</span>
                  </div>
                </div>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="variables">
              <AccordionTrigger>
                <div className="flex items-center gap-2">
                  <Variable className="h-5 w-5" />
                  <span>Variables</span>
                </div>
              </AccordionTrigger>
              <AccordionContent className="space-y-4">
                <p>Variables extract specific data from document text using regex patterns with named capture groups.</p>
                <div className="rounded-lg border bg-muted/30 p-4">
                  <p className="text-sm font-medium mb-2">Example pattern for invoice numbers:</p>
                  <code className="text-sm bg-background p-2 rounded block border">
                    {`(?i)invoice\\s*#?\\s*(?P<invoice_number>[A-Z0-9-]+)`}
                  </code>
                  <p className="text-xs text-muted-foreground mt-2">
                    Use <code className="bg-background px-1 rounded border">(?P&lt;name&gt;...)</code> to create named capture groups
                  </p>
                </div>
                <div className="grid gap-2 text-sm">
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Pattern</Badge>
                    <span>Regex with named capture groups</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Transform</Badge>
                    <span>Optional: slugify, uppercase, lowercase, trim</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Default</Badge>
                    <span>Fallback value if pattern doesn't match</span>
                  </div>
                </div>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="rules">
              <AccordionTrigger>
                <div className="flex items-center gap-2">
                  <FileText className="h-5 w-5" />
                  <span>Rules</span>
                </div>
              </AccordionTrigger>
              <AccordionContent className="space-y-4">
                <p>Rules determine how documents are categorized and where they're stored based on their content.</p>
                <div className="rounded-lg border bg-muted/30 p-4">
                  <p className="text-sm font-medium mb-2">Create multiple rules for different document types</p>
                  <p className="text-sm text-muted-foreground">
                    Set up separate rules for invoices, contracts, receipts, tax documents, and more.
                    Rules are evaluated by priority - higher priority rules are checked first.
                    The first matching rule determines where the document is saved.
                  </p>
                </div>
                <div className="grid gap-2 text-sm">
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Priority</Badge>
                    <span>Higher priority rules are checked first</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Match</Badge>
                    <span>Conditions that must be met (contains, pattern, etc.)</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Output</Badge>
                    <span>Directory and filename templates with variables</span>
                  </div>
                  <div className="flex items-start gap-2">
                    <Badge variant="outline">Symlinks</Badge>
                    <span>Optional alternative paths for organized access</span>
                  </div>
                </div>
                <div className="rounded-lg border bg-muted/30 p-4">
                  <p className="text-sm font-medium mb-2">Example output template:</p>
                  <code className="text-sm bg-background p-2 rounded block border">
                    Directory: $y/Tax/Invoices<br />
                    Filename: $invoice_number_$original.pdf
                  </code>
                </div>
              </AccordionContent>
            </AccordionItem>

            <AccordionItem value="jobs">
              <AccordionTrigger>
                <div className="flex items-center gap-2">
                  <Briefcase className="h-5 w-5" />
                  <span>Jobs</span>
                </div>
              </AccordionTrigger>
              <AccordionContent className="space-y-4">
                <p>Jobs represent document processing tasks. Each document becomes a job that progresses through phases.</p>
                <div className="flex flex-wrap gap-2">
                  {['queued', 'processing', 'extract_variables', 'categorizing', 'substituting', 'storing', 'completed'].map((phase) => (
                    <Badge key={phase} variant="outline">{phase}</Badge>
                  ))}
                </div>
                <p className="text-sm text-muted-foreground">
                  Monitor your jobs in real-time from the <Link to="/jobs" className="text-primary hover:underline">Jobs page</Link>.
                </p>
              </AccordionContent>
            </AccordionItem>
          </Accordion>
        </TabsContent>

        <TabsContent value="reference" className="space-y-4">
          <div className="grid gap-4 md:grid-cols-2">
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Variable className="h-5 w-5" />
                  Built-in Variables
                </CardTitle>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Variable</TableHead>
                      <TableHead>Description</TableHead>
                      <TableHead>Example</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {builtInVariables.map((v) => (
                      <TableRow key={v.name}>
                        <TableCell className="font-mono text-sm">{v.name}</TableCell>
                        <TableCell className="text-sm">{v.description}</TableCell>
                        <TableCell className="text-sm text-muted-foreground">{v.example}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>

            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Code className="h-5 w-5" />
                  Transforms
                </CardTitle>
              </CardHeader>
              <CardContent>
                <Table>
                  <TableHeader>
                    <TableRow>
                      <TableHead>Transform</TableHead>
                      <TableHead>Description</TableHead>
                      <TableHead>Example</TableHead>
                    </TableRow>
                  </TableHeader>
                  <TableBody>
                    {transforms.map((t) => (
                      <TableRow key={t.name}>
                        <TableCell className="font-mono text-sm">{t.name}</TableCell>
                        <TableCell className="text-sm">{t.description}</TableCell>
                        <TableCell className="text-sm text-muted-foreground">{t.example}</TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </CardContent>
            </Card>
          </div>

          <Card>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <FileText className="h-5 w-5" />
                Match Condition Types
              </CardTitle>
            </CardHeader>
            <CardContent>
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Type</TableHead>
                    <TableHead>Description</TableHead>
                    <TableHead>Example</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {matchConditionTypes.map((c) => (
                    <TableRow key={c.type}>
                      <TableCell className="font-mono text-sm">{c.type}</TableCell>
                      <TableCell className="text-sm">{c.description}</TableCell>
                      <TableCell className="font-mono text-xs text-muted-foreground">{c.example}</TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </CardContent>
          </Card>
        </TabsContent>

        <TabsContent value="examples" className="space-y-4">
          <Card>
            <CardHeader>
              <CardTitle>Example: Multi-Source Document Organization</CardTitle>
              <CardDescription>A complete setup with multiple sources and rules for different document types</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <Accordion type="single" collapsible>
                <AccordionItem value="sources">
                  <AccordionTrigger>
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">1</Badge>
                      Multiple Sources
                    </div>
                  </AccordionTrigger>
                  <AccordionContent className="space-y-4">
                    <p className="text-sm text-muted-foreground">Configure multiple input locations for different document sources:</p>
                    <pre className="bg-muted p-4 rounded-lg text-sm overflow-x-auto border">
{`# Source 1: Downloads folder
apiVersion: paporg.io/v1
kind: ImportSource
metadata:
  name: downloads
spec:
  path: ~/Downloads
  includePatterns: ["*.pdf", "*.png"]
  pollInterval: 30

---
# Source 2: Scanner output
apiVersion: paporg.io/v1
kind: ImportSource
metadata:
  name: scanner
spec:
  path: ~/Documents/Scans
  includePatterns: ["*.pdf", "*.jpg"]
  pollInterval: 60

---
# Source 3: Email attachments
apiVersion: paporg.io/v1
kind: ImportSource
metadata:
  name: email-attachments
spec:
  path: ~/Mail Downloads
  includePatterns: ["*.pdf"]
  pollInterval: 120`}
                    </pre>
                  </AccordionContent>
                </AccordionItem>

                <AccordionItem value="rules-multi">
                  <AccordionTrigger>
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">2</Badge>
                      Multiple Rules
                    </div>
                  </AccordionTrigger>
                  <AccordionContent className="space-y-4">
                    <p className="text-sm text-muted-foreground">Create rules for each document type. Higher priority rules are checked first:</p>
                    <pre className="bg-muted p-4 rounded-lg text-sm overflow-x-auto border">
{`# Rule 1: Invoices (priority 100)
apiVersion: paporg.io/v1
kind: Rule
metadata:
  name: invoices
spec:
  priority: 100
  category: Finance
  match:
    containsAny: ["Invoice", "Rechnung", "Facture"]
  output:
    directory: "$y/Invoices"
    filename: "$invoice_number_$original"

---
# Rule 2: Contracts (priority 90)
apiVersion: paporg.io/v1
kind: Rule
metadata:
  name: contracts
spec:
  priority: 90
  category: Legal
  match:
    containsAny: ["Contract", "Agreement", "Vertrag"]
  output:
    directory: "$y/Contracts"
    filename: "$original"

---
# Rule 3: Tax Documents (priority 80)
apiVersion: paporg.io/v1
kind: Rule
metadata:
  name: tax-documents
spec:
  priority: 80
  category: Tax
  match:
    containsAny: ["Tax", "Steuer", "W-2", "1099"]
  output:
    directory: "$y/Tax"
    filename: "$original"

---
# Rule 4: Catch-all (priority 0)
apiVersion: paporg.io/v1
kind: Rule
metadata:
  name: uncategorized
spec:
  priority: 0
  category: Other
  match:
    pattern: ".*"  # Matches everything
  output:
    directory: "$y/Uncategorized"
    filename: "$original"`}
                    </pre>
                  </AccordionContent>
                </AccordionItem>
              </Accordion>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Example: Single Invoice Workflow</CardTitle>
              <CardDescription>Step-by-step configuration for organizing invoices</CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <Accordion type="single" collapsible>
                <AccordionItem value="source">
                  <AccordionTrigger>
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">1</Badge>
                      Source Configuration
                    </div>
                  </AccordionTrigger>
                  <AccordionContent>
                    <pre className="bg-muted p-4 rounded-lg text-sm overflow-x-auto border">
{`apiVersion: paporg.io/v1
kind: ImportSource
metadata:
  name: downloads
spec:
  type: local
  path: ~/Downloads
  recursive: false
  pollInterval: 30
  includePatterns:
    - "*.pdf"
    - "*.png"
  enabled: true`}
                    </pre>
                  </AccordionContent>
                </AccordionItem>

                <AccordionItem value="variable">
                  <AccordionTrigger>
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">2</Badge>
                      Variable: Extract Invoice Number
                    </div>
                  </AccordionTrigger>
                  <AccordionContent>
                    <pre className="bg-muted p-4 rounded-lg text-sm overflow-x-auto border">
{`apiVersion: paporg.io/v1
kind: Variable
metadata:
  name: invoice_number
spec:
  pattern: "(?i)invoice\\s*#?\\s*(?P<invoice_number>[A-Z0-9-]+)"
  transform: uppercase
  default: "UNKNOWN"`}
                    </pre>
                  </AccordionContent>
                </AccordionItem>

                <AccordionItem value="rule">
                  <AccordionTrigger>
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">3</Badge>
                      Rule: Categorize & Organize
                    </div>
                  </AccordionTrigger>
                  <AccordionContent>
                    <pre className="bg-muted p-4 rounded-lg text-sm overflow-x-auto border">
{`apiVersion: paporg.io/v1
kind: Rule
metadata:
  name: invoices
spec:
  priority: 10
  category: Tax
  match:
    all:
      - contains: "Invoice"
      - containsAny: ["Total", "Amount Due"]
  output:
    directory: "$y/Tax/Invoices"
    filename: "$invoice_number_$original"
  symlinks:
    - "ByCategory/Invoices"`}
                    </pre>
                  </AccordionContent>
                </AccordionItem>

                <AccordionItem value="result">
                  <AccordionTrigger>
                    <div className="flex items-center gap-2">
                      <Badge variant="outline">4</Badge>
                      Result
                    </div>
                  </AccordionTrigger>
                  <AccordionContent>
                    <div className="space-y-2 text-sm">
                      <p><strong>Input:</strong> <code className="bg-muted px-1 rounded">~/Downloads/scan_march.pdf</code></p>
                      <p><strong>Extracted:</strong> Invoice #INV-2024-001</p>
                      <p><strong>Output:</strong> <code className="bg-muted px-1 rounded">2024/Tax/Invoices/INV-2024-001_scan_march.pdf</code></p>
                      <p><strong>Symlink:</strong> <code className="bg-muted px-1 rounded">ByCategory/Invoices/INV-2024-001_scan_march.pdf</code></p>
                    </div>
                  </AccordionContent>
                </AccordionItem>
              </Accordion>
            </CardContent>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  )
}
