import { createLazyFileRoute } from '@tanstack/react-router'
import Accounts from '@/features/accounts'

export const Route = createLazyFileRoute('/_authenticated/accounts/')({
  component: Accounts,
})
