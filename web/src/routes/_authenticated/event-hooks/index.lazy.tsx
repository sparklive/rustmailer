/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

import { createLazyFileRoute } from '@tanstack/react-router'
import EventHooks from '@/features/event-hooks'

export const Route = createLazyFileRoute('/_authenticated/event-hooks/')({
  component: EventHooks,
})
