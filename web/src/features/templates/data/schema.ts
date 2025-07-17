interface AccountInfo {
  id: number;
  email: string;
}

// Define the EmailTemplate interface
export interface EmailTemplate {
  id: number;
  description?: string; // Optional field
  account?: AccountInfo; // Optional field
  subject: string;
  html: string;
  text?: string;
  format: string;
  preview?: string; // Optional field
  created_at: number;
  updated_at: number;
  last_access_at: number;
}