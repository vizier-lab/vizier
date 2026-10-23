import { EmptyState, FilterTabs, Pagination, SearchField, SortSelect, Table, TableRow } from '@vizier/design'
import type { ComponentProps, ReactNode } from 'react'
import type { usePagedList } from '../../lib/usePagedList'
import { count } from '../../lib/format'

type List<T> = ReturnType<typeof usePagedList<T>>

type Props<T> = {
    label: string
    list: List<T>
    columns: ComponentProps<typeof Table>['columns']
    /** Cells for one row. */
    row?: (item: T) => ReactNode[]
    /** A whole row, for components that are rows themselves (ServerRow). Used instead of `row`. */
    renderRow?: (item: T) => ReactNode
    rowKey: (item: T) => string
    onSelect?: (item: T) => void
    selectedKey?: string
    endAligned?: number[]
    /** Filter tabs, `all` first; counts come from the server. */
    filters?: { value: string; label: ReactNode }[]
    sorts?: { value: string; label: string }[]
    searchLabel: string
    /** Shown when there is nothing at all (not merely nothing matching). */
    empty: { icon?: ComponentProps<typeof EmptyState>['icon']; title: ReactNode; message?: ReactNode; action?: ReactNode }
    toolbar?: ReactNode
}

/** SearchField + FilterTabs + SortSelect + Table + Pagination over a `usePagedList`. */
export function PagedTable<T>({
    label, list, columns, row, renderRow, rowKey, onSelect, selectedKey, endAligned,
    filters, sorts, searchLabel, empty, toolbar,
}: Props<T>) {
    const { data, state, error, loading } = list
    const total = data?.total ?? 0
    const all = data?.counts.all ?? 0
    const searching = state.q.trim() !== '' || state.filter !== 'all'

    return (
        <div className="flex flex-col gap-16">
            <div className="flex flex-wrap items-center gap-12">
                <div className="min-w-240 flex-1">
                    <SearchField
                        aria-label={searchLabel}
                        placeholder={searchLabel}
                        value={list.search}
                        onValueChange={list.setSearch}
                        count={searching && data ? `${count(total)} of ${count(all)}` : data ? count(all) : undefined}
                    />
                </div>
                {sorts && <SortSelect label="Sort" value={state.sort || sorts[0].value} options={sorts} onChange={list.setSort} />}
                {toolbar}
            </div>
            {filters && (
                <FilterTabs
                    aria-label={`Filter ${label}`}
                    value={state.filter}
                    onChange={list.setFilter}
                    options={filters.map((f) => ({ ...f, count: data ? count(data.counts[f.value] ?? 0) : undefined }))}
                />
            )}
            {error ? (
                <EmptyState kind="error" title="Couldn't load this list" message={error.message} />
            ) : data && all === 0 && !searching ? (
                <EmptyState kind="empty" {...empty} />
            ) : data && total === 0 ? (
                <EmptyState kind="no-results" title="Nothing matched" message="Try a different search or filter." />
            ) : (
                <div aria-busy={loading}>
                    <Table
                        label={label}
                        columns={columns}
                        footer={
                            <Pagination
                                page={state.page}
                                pageCount={list.pageCount}
                                total={total}
                                perPage={list.perPage}
                                onPageChange={list.setPage}
                            />
                        }
                    >
                        {(data?.items ?? []).map((item) =>
                            renderRow ? (
                                renderRow(item)
                            ) : (
                                <TableRow
                                    key={rowKey(item)}
                                    cells={row?.(item) ?? []}
                                    endAligned={endAligned}
                                    selected={selectedKey === rowKey(item)}
                                    onSelect={onSelect ? () => onSelect(item) : undefined}
                                />
                            )
                        )}
                    </Table>
                </div>
            )}
        </div>
    )
}
