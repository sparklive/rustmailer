import { createLazyFileRoute } from '@tanstack/react-router'
import APIDocs from '@/components/api-docs'

export const Route = createLazyFileRoute('/_authenticated/api-docs/')({
  component: APIDocs,
})
