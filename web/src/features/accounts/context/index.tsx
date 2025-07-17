import React from 'react'
import { AccountEntity } from '../data/schema'

export type AccountDialogType = 'add' | 'edit' | 'delete' | 'detail' | 'oauth2' | 'running-state' | 'sync-folders'

interface AccountContextType {
  open: AccountDialogType | null
  setOpen: (str: AccountDialogType | null) => void
  currentRow: AccountEntity | null
  setCurrentRow: React.Dispatch<React.SetStateAction<AccountEntity | null>>
}

const AccountContext = React.createContext<AccountContextType | null>(null)

interface Props {
  children: React.ReactNode
  value: AccountContextType
}

export default function AccountProvider({ children, value }: Props) {
  return <AccountContext.Provider value={value}>{children}</AccountContext.Provider>
}

export const useAccountContext = () => {
  const accountContext = React.useContext(AccountContext)

  if (!accountContext) {
    throw new Error(
      'useAccountContext has to be used within <AccountContext.Provider>'
    )
  }

  return accountContext
}
