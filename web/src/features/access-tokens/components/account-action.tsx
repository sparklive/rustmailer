import { Row } from '@tanstack/react-table'
import { Button } from '@/components/ui/button'
import { useAccessTokensContext } from '../context'
import { AccessToken } from '../data/schema'

interface DataTableRowActionsProps {
  row: Row<AccessToken>
}

export function AccountCellAction({ row }: DataTableRowActionsProps) {
  const { setOpen, setCurrentRow } = useAccessTokensContext()

  const accounts = row.original.accounts;
  return (
    <Button variant='ghost' onClick={() => {
      setCurrentRow(row.original)
      setOpen('account-detail')
    }}>
      <span>{accounts.length}</span>
    </Button>
  )
}
