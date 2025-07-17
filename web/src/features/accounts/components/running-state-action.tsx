import { Row } from '@tanstack/react-table'
import { Button } from '@/components/ui/button'
import { AccountEntity } from '../data/schema';
import { useAccountContext } from '../context';

interface Props {
  row: Row<AccountEntity>
}

export function RunningStateCellAction({ row }: Props) {
  const { setOpen, setCurrentRow } = useAccountContext()
  return (
    <Button variant='ghost' className="h-auto p-1" onClick={() => {
      setCurrentRow(row.original)
      setOpen('running-state')
    }}>
      <span className="text-xs text-blue-500 cursor-pointer underline hover:text-blue-700">view details</span>
    </Button>
  )
}
