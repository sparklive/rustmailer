import { createLazyFileRoute } from '@tanstack/react-router'
import MTA from '@/features/mta'

export const Route = createLazyFileRoute('/_authenticated/mta/')({
  component: MTA,
})
