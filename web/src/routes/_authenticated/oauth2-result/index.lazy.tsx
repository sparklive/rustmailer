import { createLazyFileRoute } from '@tanstack/react-router'
import OAuth2Result from '@/features/oauth2-result'

export const Route = createLazyFileRoute('/_authenticated/oauth2-result/')({
  component: OAuth2Result,
})
