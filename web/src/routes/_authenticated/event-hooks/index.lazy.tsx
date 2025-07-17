import { createLazyFileRoute } from '@tanstack/react-router'
import EventHooks from '@/features/event-hooks'

export const Route = createLazyFileRoute('/_authenticated/event-hooks/')({
  component: EventHooks,
})
