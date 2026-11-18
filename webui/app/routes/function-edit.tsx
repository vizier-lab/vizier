import { Button, CodeEditor, Notice, Select, Skeleton, TextField } from '@vizier/design'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useNavigate, useParams } from 'react-router'
import { ApiError } from '../api/client'
import { editFunction, getFunction, listFunctions, type FunctionDetail, type FunctionEdit, type FunctionProblem } from '../api/endpoints/functions'
import { highlightCode } from '../lib/highlight'
import { NAME_PATTERN, isOutOfDate, parseSchema, problemText, problemsOf, schemaText } from '../lib/functions'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

const FORM = 'function-edit'
const NEW_CATEGORY = '__new__'

/** A person's edit of a function (spec 061): saved as a new version they author. No sample run;
 * every problem is reported at once. */
export default function FunctionEditPage() {
    const { agentId = '', name: routeName = '' } = useParams()
    const navigate = useNavigate()
    const [fn, setFn] = useState<FunctionDetail | null>(null)
    const [categories, setCategories] = useState<string[]>([])
    const [name, setName] = useState('')
    const [description, setDescription] = useState('')
    const [category, setCategory] = useState('')
    const [newCategory, setNewCategory] = useState('')
    const [newCategoryDescription, setNewCategoryDescription] = useState('')
    const [schema, setSchema] = useState('')
    const [output, setOutput] = useState('')
    const [script, setScript] = useState('')
    const [reason, setReason] = useState('')
    const [busy, setBusy] = useState(false)
    const [problems, setProblems] = useState<FunctionProblem[]>([])
    const [stale, setStale] = useState(false)

    const fill = useCallback((f: FunctionDetail) => {
        setFn(f)
        setName(f.name)
        setDescription(f.description)
        setCategory(f.category)
        setSchema(schemaText(f.input_schema))
        setOutput(f.output_description ?? '')
        setScript(f.script)
        setStale(false)
        setProblems([])
    }, [])

    const load = useCallback(
        () =>
            getFunction(agentId, routeName).then(fill, (e) => {
                toast.error('Couldn’t load the function', e instanceof ApiError || e instanceof Error ? e.message : undefined)
            }),
        [agentId, routeName, fill]
    )

    useEffect(() => {
        void load()
        listFunctions(agentId).then(
            (l) => setCategories(l.categories.map((c) => c.name)),
            () => {}
        )
    }, [agentId, load])

    const scriptTokens = useMemo(() => highlightCode(script, 'python'), [script])
    const schemaTokens = useMemo(() => highlightCode(schema, 'json'), [schema])

    const back = `/${agentId}/functions/${encodeURIComponent(routeName)}`
    const creating = category === NEW_CATEGORY
    const nameOk = NAME_PATTERN.test(name)
    const newCategoryOk = !creating || (NAME_PATTERN.test(newCategory) && newCategoryDescription.trim().length > 0)
    const changed =
        !!fn &&
        (name !== fn.name ||
            description.trim() !== fn.description ||
            creating ||
            category !== fn.category ||
            schema !== schemaText(fn.input_schema) ||
            output !== (fn.output_description ?? '') ||
            script !== fn.script)
    const valid = !!fn && changed && nameOk && newCategoryOk && description.trim().length > 0 && !busy

    const submit = async () => {
        if (!fn || !valid) return
        const parsed = parseSchema(schema)
        if (!parsed.ok) {
            setProblems([{ code: 'schema_json', message: parsed.message }])
            return
        }
        const targetCategory = creating ? newCategory : category
        const edit: FunctionEdit = {
            expected_version: fn.current_version,
            name: name !== fn.name ? name : undefined,
            description: description.trim() !== fn.description ? description.trim() : undefined,
            category: targetCategory !== fn.category ? targetCategory : undefined,
            category_description: creating ? newCategoryDescription.trim() : undefined,
            input_schema: schema !== schemaText(fn.input_schema) ? parsed.value : undefined,
            output_description: output !== (fn.output_description ?? '') ? output : undefined,
            script: script !== fn.script ? script : undefined,
            reason: reason.trim() || undefined,
        }
        setBusy(true)
        setProblems([])
        try {
            const saved = await editFunction(agentId, fn.id, edit)
            toast.success(`Saved as version ${saved.version}`)
            navigate(`/${agentId}/functions/${encodeURIComponent(saved.tool?.name ?? name)}`, { replace: true })
        } catch (e) {
            if (isOutOfDate(e)) setStale(true)
            else setProblems(problemsOf(e))
        } finally {
            setBusy(false)
        }
    }

    const categoryOptions = [...(categories.includes(fn?.category ?? '') || !fn ? categories : [fn.category, ...categories]).map((c) => ({ value: c, label: c })), { value: NEW_CATEGORY, label: 'New category…' }]

    return (
        <PageFrame
            fill
            back={{ href: back, label: fn?.call_as ?? routeName }}
            title={`Edit ${fn?.call_as ?? routeName}`}
            meta={fn ? `version ${fn.current_version} · saved as version ${fn.current_version + 1}` : undefined}
            actions={
                <>
                    <Button variant="ghost" onClick={() => navigate(back)} disabled={busy}>
                        Cancel
                    </Button>
                    <Button type="submit" form={FORM} variant="primary" loading={busy} disabled={!valid}>
                        Save
                    </Button>
                </>
            }
        >
            {!fn ? (
                <Skeleton lines={10} className="m-24" />
            ) : (
                <form
                    id={FORM}
                    className="flex min-h-0 flex-1 flex-col gap-20 overflow-y-auto px-16 py-20 sm:px-24 sm:py-24"
                    onSubmit={(e) => {
                        e.preventDefault()
                        void submit()
                    }}
                >
                    {stale && (
                        <Notice tone="failed" title="This function changed since you opened it" actions={<Button size="sm" variant="secondary" onClick={() => void load()}>Reload</Button>}>
                            Reload to see the change, then edit again. Reloading drops your edit.
                        </Notice>
                    )}
                    {problems.length > 0 && (
                        <Notice tone="failed" title="Couldn't save the function">
                            <ul className="list-disc pl-20">
                                {problems.map((p, i) => (
                                    <li key={i}>{problemText(p)}</li>
                                ))}
                            </ul>
                        </Notice>
                    )}
                    <div className="grid grid-cols-1 items-start gap-16 md:grid-cols-2">
                        <TextField
                            label="Name"
                            mono
                            value={name}
                            autoComplete="off"
                            spellCheck={false}
                            helper={nameOk ? 'The agent calls it by this name; it can’t be the name of another tool the agent has.' : 'Lower-case letters, digits and _, starting with a letter.'}
                            invalid={!nameOk}
                            onChange={(e) => setName(e.target.value)}
                        />
                        <Select label="Category" value={category} onChange={setCategory} options={categoryOptions} />
                        <TextField label="Description" value={description} helper="One line: what it does and when the agent should call it." onChange={(e) => setDescription(e.target.value)} />
                        <TextField label="What it returns" value={output} helper="Optional." onChange={(e) => setOutput(e.target.value)} />
                        {creating && (
                            <>
                                <TextField
                                    label="New category name"
                                    mono
                                    value={newCategory}
                                    invalid={newCategory.length > 0 && !NAME_PATTERN.test(newCategory)}
                                    helper="Lower-case letters, digits and _, starting with a letter."
                                    onChange={(e) => setNewCategory(e.target.value)}
                                />
                                <TextField
                                    label="New category description"
                                    value={newCategoryDescription}
                                    helper="Required for a new category: one line saying what it is for."
                                    onChange={(e) => setNewCategoryDescription(e.target.value)}
                                />
                            </>
                        )}
                    </div>
                    <div className="grid grid-cols-1 items-start gap-16 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
                        <CodeEditor
                            label="Script"
                            lineNumbers
                            rows={18}
                            value={script}
                            tokens={scriptTokens}
                            helper="The arguments are the dict input; the last expression is the result."
                            onChange={setScript}
                        />
                        <CodeEditor label="Input schema (JSON)" lineNumbers rows={18} value={schema} tokens={schemaTokens} onChange={setSchema} />
                    </div>
                    <TextField label="Why" value={reason} helper="Optional, kept with the version." onChange={(e) => setReason(e.target.value)} />
                </form>
            )}
        </PageFrame>
    )
}
