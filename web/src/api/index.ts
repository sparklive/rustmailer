export interface PaginatedResponse<S> {
    current_page: number | null;
    page_size: number | null;
    total_items: number;
    items: S[];
    total_pages: number | null;
}