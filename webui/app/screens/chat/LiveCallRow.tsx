import { CallRow, type CallRowProps } from '@vizier/design'
import { useCallLevels } from './useLiveVoice'

/** The call's row, reading its own waveform: the levels change every 60 ms, and only this redraws. */
export function LiveCallRow(props: Omit<CallRowProps, 'levels'>) {
    const levels = useCallLevels((s) => s.levels)
    return <CallRow {...props} levels={levels} />
}
