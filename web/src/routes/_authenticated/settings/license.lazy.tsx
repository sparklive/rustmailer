import { createLazyFileRoute } from '@tanstack/react-router'
import License from '@/features/settings/license'

export const Route = createLazyFileRoute(
  '/_authenticated/settings/license'
)({
  component: License,
})
