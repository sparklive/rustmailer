import ProxyManagerPage from '@/features/settings/proxy'
import { createLazyFileRoute } from '@tanstack/react-router'

export const Route = createLazyFileRoute('/_authenticated/settings/proxy')({
  component: ProxyManagerPage,
})
