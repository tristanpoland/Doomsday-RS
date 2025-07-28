'use client';

import { useState, useMemo } from 'react';
import {
  CertificateWithStatus,
  CertStatus,
} from '@/types';
import {
  getStatusColor,
  getStatusIcon,
  formatDateTime,
  formatRelativeTime,
} from '@/lib/utils';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Search, Calendar, MapPin, Filter } from 'lucide-react';

interface CertificateTableProps {
  certificates: CertificateWithStatus[];
  loading?: boolean;
}

export function CertificateTable({ certificates, loading = false }: CertificateTableProps) {
  const [searchTerm, setSearchTerm] = useState('');
  const [statusFilter, setStatusFilter] = useState<CertStatus | 'all'>('all');
  const [sortBy, setSortBy] = useState<'subject' | 'expiry' | 'status'>('expiry');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc');

  const filteredAndSortedCertificates = useMemo(() => {
    let filtered = certificates;

    // Apply search filter
    if (searchTerm) {
      filtered = filtered.filter(cert =>
        cert.subject.toLowerCase().includes(searchTerm.toLowerCase()) ||
        cert.paths.some(path => 
          path.backend.toLowerCase().includes(searchTerm.toLowerCase()) ||
          path.path.toLowerCase().includes(searchTerm.toLowerCase())
        )
      );
    }

    // Apply status filter
    if (statusFilter !== 'all') {
      filtered = filtered.filter(cert => cert.status === statusFilter);
    }

    // Apply sorting
    filtered.sort((a, b) => {
      let comparison = 0;
      
      switch (sortBy) {
        case 'subject':
          comparison = a.subject.localeCompare(b.subject);
          break;
        case 'expiry':
          comparison = new Date(a.not_after).getTime() - new Date(b.not_after).getTime();
          break;
        case 'status':
          const statusOrder = { [CertStatus.EXPIRED]: 0, [CertStatus.EXPIRING_SOON]: 1, [CertStatus.OK]: 2 };
          comparison = statusOrder[a.status] - statusOrder[b.status];
          break;
      }
      
      return sortOrder === 'asc' ? comparison : -comparison;
    });

    return filtered;
  }, [certificates, searchTerm, statusFilter, sortBy, sortOrder]);

  const handleSort = (column: 'subject' | 'expiry' | 'status') => {
    if (sortBy === column) {
      setSortOrder(sortOrder === 'asc' ? 'desc' : 'asc');
    } else {
      setSortBy(column);
      setSortOrder('asc');
    }
  };

  if (loading) {
    return (
      <div className="space-y-4">
        <div className="h-8 bg-gray-200 rounded animate-pulse"></div>
        <div className="space-y-2">
          {[...Array(5)].map((_, i) => (
            <div key={i} className="h-16 bg-gray-100 rounded animate-pulse"></div>
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Filters */}
      <div className="flex flex-col sm:flex-row gap-4">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 text-gray-400 h-4 w-4" />
          <Input
            placeholder="Search certificates, backends, or paths..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            className="pl-10"
          />
        </div>
        
        <div className="flex gap-2">
          <select
            value={statusFilter}
            onChange={(e) => setStatusFilter(e.target.value as CertStatus | 'all')}
            className="px-3 py-2 border rounded-md text-sm"
          >
            <option value="all">All Status</option>
            <option value={CertStatus.EXPIRED}>Expired</option>
            <option value={CertStatus.EXPIRING_SOON}>Expiring Soon</option>
            <option value={CertStatus.OK}>OK</option>
          </select>
        </div>
      </div>

      {/* Results count */}
      <div className="text-sm text-gray-600">
        Showing {filteredAndSortedCertificates.length} of {certificates.length} certificates
      </div>

      {/* Table */}
      <div className="overflow-hidden shadow ring-1 ring-black ring-opacity-5 md:rounded-lg">
        <table className="min-w-full divide-y divide-gray-300">
          <thead className="bg-gray-50">
            <tr>
              <th
                scope="col"
                className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider cursor-pointer hover:bg-gray-100"
                onClick={() => handleSort('status')}
              >
                Status
                {sortBy === 'status' && (
                  <span className="ml-1">{sortOrder === 'asc' ? '↑' : '↓'}</span>
                )}
              </th>
              <th
                scope="col"
                className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider cursor-pointer hover:bg-gray-100"
                onClick={() => handleSort('subject')}
              >
                Subject
                {sortBy === 'subject' && (
                  <span className="ml-1">{sortOrder === 'asc' ? '↑' : '↓'}</span>
                )}
              </th>
              <th
                scope="col"
                className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider cursor-pointer hover:bg-gray-100"
                onClick={() => handleSort('expiry')}
              >
                Expiry
                {sortBy === 'expiry' && (
                  <span className="ml-1">{sortOrder === 'asc' ? '↑' : '↓'}</span>
                )}
              </th>
              <th
                scope="col"
                className="px-6 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider"
              >
                Paths
              </th>
            </tr>
          </thead>
          <tbody className="bg-white divide-y divide-gray-200">
            {filteredAndSortedCertificates.map((cert, index) => (
              <tr key={`${cert.subject}-${index}`} className="hover:bg-gray-50">
                <td className="px-6 py-4 whitespace-nowrap">
                  <Badge
                    variant={
                      cert.status === CertStatus.OK
                        ? 'success'
                        : cert.status === CertStatus.EXPIRING_SOON
                        ? 'warning'
                        : 'error'
                    }
                    className="flex items-center gap-1"
                  >
                    <span>{getStatusIcon(cert.status)}</span>
                    <span className="capitalize">
                      {cert.status === CertStatus.EXPIRING_SOON ? 'Expiring Soon' : cert.status}
                    </span>
                  </Badge>
                </td>
                <td className="px-6 py-4">
                  <div className="text-sm font-medium text-gray-900">
                    {cert.subject}
                  </div>
                </td>
                <td className="px-6 py-4">
                  <div className="text-sm text-gray-900">
                    <div className="flex items-center gap-1">
                      <Calendar className="h-4 w-4 text-gray-400" />
                      {formatDateTime(cert.not_after)}
                    </div>
                    <div className="text-xs text-gray-500">
                      {formatRelativeTime(cert.not_after)}
                    </div>
                  </div>
                </td>
                <td className="px-6 py-4">
                  <div className="space-y-1">
                    {cert.paths.map((path, pathIndex) => (
                      <div
                        key={pathIndex}
                        className="flex items-center gap-1 text-xs text-gray-600"
                      >
                        <MapPin className="h-3 w-3 text-gray-400" />
                        <span className="font-medium">{path.backend}:</span>
                        <span className="font-mono">{path.path}</span>
                      </div>
                    ))}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {filteredAndSortedCertificates.length === 0 && (
        <div className="text-center py-12">
          <div className="text-gray-500">
            {certificates.length === 0
              ? 'No certificates found'
              : 'No certificates match your filters'}
          </div>
          {searchTerm && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setSearchTerm('')}
              className="mt-2"
            >
              Clear search
            </Button>
          )}
        </div>
      )}
    </div>
  );
}