/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

export interface PaginatedResponse<S> {
    current_page: number | null;
    page_size: number | null;
    total_items: number;
    items: S[];
    total_pages: number | null;
}