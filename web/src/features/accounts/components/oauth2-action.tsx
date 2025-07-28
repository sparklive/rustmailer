/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

import { Row } from '@tanstack/react-table'
import { Button } from '@/components/ui/button'
import { useAccountContext } from '../context'
import { AccountEntity } from '../data/schema'

interface DataTableRowActionsProps {
  row: Row<AccountEntity>
}

export function OAuth2Action({ row }: DataTableRowActionsProps) {
  const { setOpen, setCurrentRow } = useAccountContext()

  return row.original.imap.auth.auth_type === 'OAuth2' ? (
    <Button variant='ghost' onClick={() => {
      setCurrentRow(row.original)
      setOpen('oauth2')
    }}><span className="text-xs text-blue-500 cursor-pointer underline hover:text-blue-700">{row.original.imap.auth.auth_type}</span></Button>
  ) : (
    <span className="text-xs cursor-pointer">{row.original.imap.auth.auth_type}</span>
  )
}
