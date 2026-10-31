import { ActivityGrid, Button, Segment, TextLink } from '@vizier/design'
import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import {
    getActivity,
    type Activity,
    type Rank,
} from '../../api/endpoints/overview'
import { count } from '../../lib/format'
import { gridMeta, gridRows, summaryLine } from '../../lib/overview'
import { useCan } from '../../lib/access'

const RANKS: { value: Rank; label: string }[] = [
    { value: 'most_active', label: 'Most active' },
    { value: 'mine', label: 'Mine' },
    { value: 'live', label: 'Running' },
    { value: 'failed', label: 'Failed' },
]

const REFRESH_MS = 30_000

/** Home's "Last 24 hours": every agent's turns per hour, at most seven rows whatever the court's size. */
export function ActivityPanel({ compact }: { compact?: boolean }) {
    const navigate = useNavigate()
    const canCreate = useCan('all_agents:create')
    const [rank, setRank] = useState<Rank>('most_active')
    const [activity, setActivity] = useState<Activity>()
    const [failed, setFailed] = useState(false)

    useEffect(() => {
        let live = true
        const load = () =>
            getActivity(rank).then(
                (a) => live && (setActivity(a), setFailed(false)),
                () => live && setFailed(true)
            )
        void load()
        const id = setInterval(
            () => document.visibilityState === 'visible' && void load(),
            REFRESH_MS
        )
        return () => {
            live = false
            clearInterval(id)
        }
    }, [rank])

    // The panel keeps its seven rows of height while loading, so Home doesn't jump.
    if (!activity) {
        return (
            <ActivityGrid
                aria-busy={!failed}
                compact={compact}
                rows={[]}
                empty={
                    failed
                        ? {
                              title: 'Activity could not be loaded',
                              message: 'It will try again in a moment.',
                          }
                        : undefined
                }
            />
        )
    }

    const rows = gridRows(activity)
    const hidden = Math.max(0, activity.agent_total - activity.rows.length)
    return (
        <ActivityGrid
            compact={compact}
            rows={rows}
            meta={compact ? undefined : gridMeta(activity)}
            summary={summaryLine(activity)}
            empty={{
                title: 'No activity yet',
                message:
                    'This fills in once an agent takes its first turn: a chat, a task, or a dream.',
                action: canCreate ? (
                    <Button
                        variant="primary"
                        size="sm"
                        icon="plus"
                        onClick={() => navigate('/?new=1')}
                    >
                        New agent
                    </Button>
                ) : undefined,
            }}
            footer={
                activity.agent_total > 1 ? (
                    <>
                        <div
                            role="group"
                            aria-label="Show"
                            className="inline-flex rounded-md border border-border-strong bg-surface p-2"
                        >
                            {RANKS.map((r) => (
                                <Segment
                                    key={r.value}
                                    selected={rank === r.value}
                                    onClick={() => setRank(r.value)}
                                >
                                    {r.label}
                                </Segment>
                            ))}
                        </div>
                        {hidden > 0 && (
                            <TextLink
                                size="ui"
                                href="/"
                                arrow="forward"
                                onClick={(e) => (
                                    e.preventDefault(),
                                    document
                                        .getElementById('the-court')
                                        ?.scrollIntoView({ behavior: 'smooth' })
                                )}
                            >
                                {count(hidden)} more below
                            </TextLink>
                        )}
                    </>
                ) : undefined
            }
        />
    )
}
