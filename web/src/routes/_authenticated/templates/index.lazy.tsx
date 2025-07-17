import { createLazyFileRoute } from '@tanstack/react-router'
import EmailTemplates from '@/features/templates'

export const Route = createLazyFileRoute('/_authenticated/templates/')({
  component: EmailTemplates,
})
