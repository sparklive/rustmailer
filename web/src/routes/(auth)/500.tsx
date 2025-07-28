/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

import { createFileRoute } from '@tanstack/react-router'
import GeneralError from '@/features/errors/general-error'

export const Route = createFileRoute('/(auth)/500')({
  component: GeneralError,
})
