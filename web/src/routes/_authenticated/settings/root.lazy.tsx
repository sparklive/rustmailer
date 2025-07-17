import { createLazyFileRoute } from '@tanstack/react-router'
import RootAccessToken from '@/features/settings/root'

export const Route = createLazyFileRoute(
  '/_authenticated/settings/root',
)({
  component: RootAccessToken,
})
