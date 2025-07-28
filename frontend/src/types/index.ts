export interface CacheItem {
  subject: string;
  not_after: string;
  paths: PathObject[];
}

export interface PathObject {
  backend: string;
  path: string;
}

export interface PopulateStats {
  num_certs: number;
  num_paths: number;
  duration_ms: number;
}

export interface InfoResponse {
  version: string;
  auth_required: boolean;
}

export interface AuthRequest {
  username: string;
  password: string;
}

export interface AuthResponse {
  token: string;
  expires_at: string;
}

export interface SchedulerInfo {
  workers: number;
  pending_tasks: number;
  running_tasks: number;
}

export interface RefreshRequest {
  backends?: string[];
}

export interface CacheStats {
  total: number;
  ok: number;
  expiring_soon: number;
  expired: number;
}

export enum CertStatus {
  OK = 'ok',
  EXPIRING_SOON = 'expiring_soon', 
  EXPIRED = 'expired',
}

export interface CertificateWithStatus extends CacheItem {
  status: CertStatus;
  days_until_expiry: number;
}