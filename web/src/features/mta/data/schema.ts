interface SmtpCredentials {
  username: string;
  password: string;
}

interface SmtpServerConfig {
  host: string;
  port: number;
  encryption: 'StartTls' | 'None' | 'Ssl';
}

export interface MTARecord {
  id: number;
  description?: string;
  credentials: SmtpCredentials;
  server: SmtpServerConfig;
  dsn_capable: boolean;
  created_at: number;
  updated_at: number;
  last_access_at: number;
  use_proxy?: number;
}
