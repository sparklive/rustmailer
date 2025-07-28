/*
 * Copyright © 2025 rustmailer.com
 * Licensed under RustMailer License Agreement v1.0
 * Unauthorized use or distribution is prohibited.
 */

type Encryption = 'Ssl' | 'StartTls' | 'None';
type AuthType = 'Password' | 'OAuth2';
type Unit = 'Days' | 'Months' | 'Years';

// Interface definitions
interface AuthConfig {
  auth_type: AuthType;
  password?: string;
}

interface SmtpConfig {
  host: string;
  port: number; // integer, 0-65535
  encryption: Encryption;
  auth: AuthConfig;
  use_proxy?: number;
}

interface ImapConfig {
  host: string;
  port: number; // integer, 0-65535
  encryption: Encryption;
  auth: AuthConfig;
  use_proxy?: number;
}

interface RelativeDate {
  unit: Unit;
  value: number; // integer, minimum 1
}

interface DateSelection {
  fixed?: string; // format: "YYYY-MM-DD"
  relative?: RelativeDate;
}

export interface AccountEntity {
  id: number;
  imap: ImapConfig;
  smtp: SmtpConfig;
  enabled: boolean;
  deleted: boolean;
  name?: string,
  email: string;
  minimal_sync: boolean;
  capabilities: string[];
  date_since?: DateSelection;
  sync_folders?: string[];
  full_sync_interval_min: number;
  incremental_sync_interval_sec: number;
  created_at: number;
  updated_at: number;
}