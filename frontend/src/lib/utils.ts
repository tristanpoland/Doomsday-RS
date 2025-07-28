import { type ClassValue, clsx } from 'clsx';
import { CacheItem, CertStatus, CertificateWithStatus } from '@/types';

export function cn(...inputs: ClassValue[]) {
  return clsx(inputs);
}

export function formatDuration(durationStr: string): string {
  // Parse duration from Rust format (e.g., "1y2d3h4m5s") to human readable
  const matches = durationStr.match(/(\d+)([yMwdhms])/g);
  if (!matches) return durationStr;

  const parts: string[] = [];
  
  for (const match of matches) {
    const [, num, unit] = match.match(/(\d+)([yMwdhms])/) || [];
    if (!num || !unit) continue;
    
    const number = parseInt(num);
    let unitName = '';
    
    switch (unit) {
      case 'y': unitName = number === 1 ? 'year' : 'years'; break;
      case 'M': unitName = number === 1 ? 'month' : 'months'; break;
      case 'w': unitName = number === 1 ? 'week' : 'weeks'; break;
      case 'd': unitName = number === 1 ? 'day' : 'days'; break;
      case 'h': unitName = number === 1 ? 'hour' : 'hours'; break;
      case 'm': unitName = number === 1 ? 'minute' : 'minutes'; break;
      case 's': unitName = number === 1 ? 'second' : 'seconds'; break;
      default: continue;
    }
    
    parts.push(`${number} ${unitName}`);
  }
  
  if (parts.length === 0) return durationStr;
  if (parts.length === 1) return parts[0];
  if (parts.length === 2) return parts.join(' and ');
  
  return parts.slice(0, -1).join(', ') + ', and ' + parts[parts.length - 1];
}

export function getDaysUntilExpiry(dateString: string): number {
  const expiryDate = new Date(dateString);
  const now = new Date();
  const diffTime = expiryDate.getTime() - now.getTime();
  return Math.ceil(diffTime / (1000 * 60 * 60 * 24));
}

export function getCertificateStatus(daysUntilExpiry: number): CertStatus {
  if (daysUntilExpiry < 0) {
    return CertStatus.EXPIRED;
  } else if (daysUntilExpiry <= 30) {
    return CertStatus.EXPIRING_SOON;
  } else {
    return CertStatus.OK;
  }
}

export function addStatusToCertificates(certificates: CacheItem[]): CertificateWithStatus[] {
  return certificates.map(cert => {
    const daysUntilExpiry = getDaysUntilExpiry(cert.not_after);
    const status = getCertificateStatus(daysUntilExpiry);
    
    return {
      ...cert,
      status,
      days_until_expiry: daysUntilExpiry,
    };
  });
}

export function getStatusColor(status: CertStatus): string {
  switch (status) {
    case CertStatus.OK:
      return 'text-green-700 bg-green-50 border-green-200';
    case CertStatus.EXPIRING_SOON:
      return 'text-orange-700 bg-orange-50 border-orange-200';
    case CertStatus.EXPIRED:
      return 'text-red-700 bg-red-50 border-red-200';
    default:
      return 'text-gray-700 bg-gray-50 border-gray-200';
  }
}

export function getStatusIcon(status: CertStatus): string {
  switch (status) {
    case CertStatus.OK:
      return '✅';
    case CertStatus.EXPIRING_SOON:
      return '⏰';
    case CertStatus.EXPIRED:
      return '⚠️';
    default:
      return '❓';
  }
}

export function formatRelativeTime(dateString: string): string {
  const date = new Date(dateString);
  const now = new Date();
  const diffMs = date.getTime() - now.getTime();
  const diffDays = Math.ceil(diffMs / (1000 * 60 * 60 * 24));
  
  if (diffDays < 0) {
    const pastDays = Math.abs(diffDays);
    if (pastDays === 1) return 'expired yesterday';
    if (pastDays < 30) return `expired ${pastDays} days ago`;
    if (pastDays < 365) return `expired ${Math.floor(pastDays / 30)} months ago`;
    return `expired ${Math.floor(pastDays / 365)} years ago`;
  }
  
  if (diffDays === 0) return 'expires today';
  if (diffDays === 1) return 'expires tomorrow';
  if (diffDays < 30) return `expires in ${diffDays} days`;
  if (diffDays < 365) return `expires in ${Math.floor(diffDays / 30)} months`;
  return `expires in ${Math.floor(diffDays / 365)} years`;
}

export function formatDateTime(dateString: string): string {
  const date = new Date(dateString);
  return date.toLocaleString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    timeZoneName: 'short',
  });
}

export function groupCertificatesByStatus(certificates: CertificateWithStatus[]) {
  const groups = {
    expired: certificates.filter(cert => cert.status === CertStatus.EXPIRED),
    expiring_soon: certificates.filter(cert => cert.status === CertStatus.EXPIRING_SOON),
    ok: certificates.filter(cert => cert.status === CertStatus.OK),
  };

  return groups;
}