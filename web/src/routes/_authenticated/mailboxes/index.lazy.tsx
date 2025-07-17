import { createLazyFileRoute } from '@tanstack/react-router'
import Mailboxes from '@/features/mailbox'

export const Route = createLazyFileRoute('/_authenticated/mailboxes/')({
  component: Mailboxes,
})
