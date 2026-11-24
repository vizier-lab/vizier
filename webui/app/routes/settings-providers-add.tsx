import { Button, Dialog, EmptyState, Skeleton } from '@vizier/design'
import { useEffect, useMemo, useState } from 'react'
import { useNavigate } from 'react-router'
import { allProviders, type ProviderSummary } from '../api/endpoints/catalogue'
import { connectedProviders, customProviders } from '../api/endpoints/providers'
import { invalidateCatalogue } from '../lib/catalogue'
import { AddProvider } from '../screens/providers/AddProvider'
import { CustomProviderForm } from '../screens/providers/CustomProviderForm'
import { PageFrame } from '../screens/shared/PageFrame'
import { toast } from '../stores/toasts'

/** Connecting a provider, on its own page: the whole catalogue to search, each row opening its form. */
export default function SettingsProvidersAdd() {
    const navigate = useNavigate()
    const [available, setAvailable] = useState<ProviderSummary[]>()
    const [error, setError] = useState<string>()
    const [custom, setCustom] = useState<{ name?: string } | null>(null)

    useEffect(() => {
        invalidateCatalogue()
        Promise.all([allProviders(), connectedProviders(), customProviders()])
            .then(([all, connected, cu]) => {
                const taken = new Set([...(connected ?? []), ...(cu ?? [])].map((p) => p.id))
                setAvailable((all ?? []).filter((p) => !taken.has(p.id) && p.origin !== 'custom' && p.fields.length > 0))
            })
            .catch((e) => setError(e instanceof Error ? e.message : 'The catalogue could not be loaded.'))
    }, [])

    const count = useMemo(() => available?.length ?? 0, [available])
    const done = () => navigate('/settings/providers')

    return (
        <PageFrame
            fill
            back={{ href: '/settings/providers', label: 'Providers' }}
            title="Add a provider"
            meta={available ? `${count} in the catalogue · keys are checked when you save` : undefined}
            actions={
                <Button icon="plus" onClick={() => setCustom({})}>
                    Add custom provider
                </Button>
            }
        >
            <div className="flex min-h-0 flex-1 flex-col px-16 py-20 sm:px-24 sm:py-24">
                {error ? (
                    <EmptyState kind="error" title="Couldn’t load the catalogue" message={error} />
                ) : !available ? (
                    <Skeleton lines={8} />
                ) : (
                    <AddProvider
                        page
                        providers={available}
                        onConnected={(p) => {
                            toast.success(`${p.name} connected`)
                            done()
                        }}
                        onCustom={(name) => setCustom({ name })}
                    />
                )}
            </div>

            <Dialog open={custom !== null} title="Add custom provider" confirmLabel="" onConfirm={() => {}} onCancel={() => setCustom(null)}>
                {custom !== null && (
                    <CustomProviderForm
                        initialName={custom.name}
                        onCancel={() => setCustom(null)}
                        onDone={() => {
                            setCustom(null)
                            done()
                        }}
                    />
                )}
            </Dialog>
        </PageFrame>
    )
}
