import { createLazyFileRoute } from '@tanstack/react-router'
import AccessTokens from '@/features/access-tokens'

export const Route = createLazyFileRoute('/_authenticated/access-tokens/')({
  component: AccessTokens,
})
