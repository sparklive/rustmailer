/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

import { useState } from 'react'
import useDialogState from '@/hooks/use-dialog-state'
import { Button } from '@/components/ui/button'
import { Main } from '@/components/layout/main'
import { AccountActionDialog } from './components/action-dialog'
import { columns } from './components/columns'
import { AccountDeleteDialog } from './components/delete-dialog'
import { AccountTable } from './components/table'
import AccountProvider, {
  type AccountDialogType,
} from './context'
import { Plus } from 'lucide-react'
import Logo from '@/assets/logo.svg'
import { AccountEntity, MailerType } from './data/schema'
import { AccountDetailDrawer } from './components/account-detail'
import { list_accounts } from '@/api/account/api'
import { TableSkeleton } from '@/components/table-skeleton'
import { useQuery } from '@tanstack/react-query'
import { OAuth2TokensDialog } from './components/oauth2-tokens'
import { RunningStateDialog } from './components/running-state-dialog'
import { FixedHeader } from '@/components/layout/fixed-header'
import { SyncFoldersDialog } from './components/sync-folders'

export default function Accounts() {
  // Dialog states
  const [currentRow, setCurrentRow] = useState<AccountEntity | null>(null)
  const [open, setOpen] = useDialogState<AccountDialogType>(null)

  const { data: accountList, isLoading } = useQuery({
    queryKey: ['account-list'],
    queryFn: list_accounts,
  })

  const hasAccounts = accountList != null && accountList.items.length > 0;

  return (
    <AccountProvider value={{ open, setOpen, currentRow, setCurrentRow }}>
      {/* ===== Top Heading ===== */}
      <FixedHeader />

      <Main>
        <div className='mb-2 flex items-center justify-between space-y-2 flex-wrap gap-x-4'>
          <div>
            <h2 className='text-2xl font-bold tracking-tight'>Email Accounts</h2>
            <p className='text-muted-foreground'>
              Manage and configure your email accounts, integrating with various service providers for seamless communication.
            </p>
          </div>
          <div className='flex gap-2'>
            <Button className='space-x-1' onClick={() => setOpen('add')}>
              <span>Add</span> <Plus size={18} />
            </Button>
          </div>
        </div>
        <div className='-mx-4 flex-1 overflow-auto px-4 py-1 flex-row lg:space-x-12 space-y-0'>
          {isLoading ? <TableSkeleton columns={columns.length} rows={10} /> : hasAccounts ? (
            <AccountTable data={accountList.items} columns={columns} />
          ) : (
            <div className="flex h-[450px] shrink-0 items-center justify-center rounded-md border border-dashed">
              <div className="mx-auto flex max-w-[420px] flex-col items-center justify-center text-center">
                <img
                  src={Logo}
                  className="max-h-[100px] w-auto opacity-20 saturate-0 transition-all duration-300 hover:opacity-100 hover:saturate-100 object-contain"
                  alt="RustMailer Logo"
                />
                <h3 className="mt-4 text-lg font-semibold">No Account Configurations</h3>
                <p className="mb-4 mt-2 text-sm text-muted-foreground">
                  You haven't added any Account configurations yet. Add one to start using Account features.
                </p>
                <Button onClick={() => setOpen('add')}>
                  Add Configuration
                </Button>
              </div>
            </div>
          )}
        </div>
      </Main>

      <AccountActionDialog
        key='account-add'
        open={open === 'add'}
        onOpenChange={() => setOpen('add')}
      />

      {currentRow && (
        <>
          <AccountActionDialog
            key={`account-edit-${currentRow.id}`}
            open={open === 'edit'}
            onOpenChange={() => {
              setOpen('edit')
              setTimeout(() => {
                setCurrentRow(null)
              }, 500)
            }}
            currentRow={currentRow}
          />

          <RunningStateDialog
            key='running-state'
            open={open === 'running-state'}
            onOpenChange={() => {
              setOpen('running-state')
              setTimeout(() => {
                setCurrentRow(null)
              }, 500)
            }}
            currentRow={currentRow}
          />
          <AccountDeleteDialog
            key={`account-delete-${currentRow.id}`}
            open={open === 'delete'}
            onOpenChange={() => {
              setOpen('delete')
              setTimeout(() => {
                setCurrentRow(null)
              }, 500)
            }}
            currentRow={currentRow}
          />
          <SyncFoldersDialog
            key={`sync-folders-${currentRow.id}`}
            open={open === 'sync-folders'}
            onOpenChange={() => {
              setOpen('sync-folders')
              setTimeout(() => {
                setCurrentRow(null)
              }, 500)
            }}
            currentRow={currentRow}
          />

          <AccountDetailDrawer
            open={open === 'detail'}
            onOpenChange={() => setOpen('detail')}
            currentRow={currentRow}
          />
          {(
            (currentRow.mailer_type === MailerType.ImapSmtp &&
              currentRow.imap?.auth.auth_type === 'OAuth2') ||
            currentRow.mailer_type === MailerType.GmailApi
          ) && <OAuth2TokensDialog open={open === 'oauth2'}
            onOpenChange={() => setOpen('oauth2')}
            currentRow={currentRow}
            />}
        </>
      )}
    </AccountProvider>
  )
}
