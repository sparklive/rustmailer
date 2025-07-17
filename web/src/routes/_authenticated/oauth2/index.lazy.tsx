import { createLazyFileRoute } from '@tanstack/react-router'
import OAuth2 from '@/features/oauth2'

export const Route = createLazyFileRoute('/_authenticated/oauth2/')({
  component: OAuth2,
})
