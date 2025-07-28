'use client';

import { useEffect, useState } from 'react';
import useSWR from 'swr';
import { api, ApiError } from '@/lib/api';
import { addStatusToCertificates, groupCertificatesByStatus } from '@/lib/utils';
import { CertificateWithStatus, InfoResponse, CacheStats } from '@/types';
import { Header } from '@/components/layout/Header';
import { StatsCards } from '@/components/dashboard/StatsCards';
import { CertificateTable } from '@/components/dashboard/CertificateTable';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { AlertTriangle } from 'lucide-react';

export default function Dashboard() {
  const [authRequired, setAuthRequired] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  // Fetch server info to determine if auth is required
  const { data: serverInfo, error: infoError } = useSWR<InfoResponse>(
    'server-info',
    () => api.getInfo(),
    { 
      refreshInterval: 30000,
      onSuccess: (data) => setAuthRequired(data.auth_required),
    }
  );

  // Fetch certificates
  const { 
    data: certificates = [], 
    error: certificatesError, 
    mutate: mutateCertificates,
    isLoading: certificatesLoading 
  } = useSWR(
    'certificates',
    () => api.getCertificates(),
    { 
      refreshInterval: 60000,
      errorRetryCount: 3,
      errorRetryInterval: 5000,
    }
  );

  const certificatesWithStatus = addStatusToCertificates(certificates);
  const groupedCertificates = groupCertificatesByStatus(certificatesWithStatus);

  const stats: CacheStats = {
    total: certificatesWithStatus.length,
    expired: groupedCertificates.expired.length,
    expiring_soon: groupedCertificates.expiring_soon.length,
    ok: groupedCertificates.ok.length,
  };

  const handleRefresh = async () => {
    setRefreshing(true);
    try {
      await api.refreshCache();
      await mutateCertificates();
    } catch (error) {
      console.error('Failed to refresh:', error);
    } finally {
      setRefreshing(false);
    }
  };

  const hasError = infoError || certificatesError;
  const isAuthError = hasError && (hasError as ApiError).status === 401;

  return (
    <div className="min-h-screen bg-gray-50">
      <Header
        onRefresh={handleRefresh}
        refreshing={refreshing}
        authRequired={authRequired}
      />

      <main className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 py-8">
        <div className="space-y-8">
          {/* Error Messages */}
          {hasError && !isAuthError && (
            <Alert className="border-red-200 bg-red-50">
              <AlertTriangle className="h-4 w-4 text-red-600" />
              <AlertDescription className="text-red-800">
                {hasError instanceof ApiError 
                  ? `Error ${hasError.status}: ${hasError.message}`
                  : 'Failed to load data. Please try refreshing the page.'}
              </AlertDescription>
            </Alert>
          )}

          {isAuthError && (
            <Alert className="border-yellow-200 bg-yellow-50">
              <AlertTriangle className="h-4 w-4 text-yellow-600" />
              <AlertDescription className="text-yellow-800">
                Authentication required. Please log in to view certificates.
              </AlertDescription>
            </Alert>
          )}

          {/* Stats Cards */}
          <StatsCards stats={stats} loading={certificatesLoading} />

          {/* Certificates Table */}
          <div className="bg-white shadow rounded-lg">
            <div className="px-6 py-4 border-b border-gray-200">
              <h2 className="text-lg font-medium text-gray-900">
                Certificate Details
              </h2>
              <p className="text-sm text-gray-500">
                Monitor certificate expiration status across all backends
              </p>
            </div>
            <div className="p-6">
              <CertificateTable
                certificates={certificatesWithStatus}
                loading={certificatesLoading}
              />
            </div>
          </div>

          {/* Server Info */}
          {serverInfo && (
            <div className="bg-white shadow rounded-lg p-6">
              <h3 className="text-lg font-medium text-gray-900 mb-4">
                Server Information
              </h3>
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-sm">
                <div>
                  <span className="font-medium text-gray-700">Version:</span>
                  <span className="ml-2 text-gray-600">{serverInfo.version}</span>
                </div>
                <div>
                  <span className="font-medium text-gray-700">Authentication:</span>
                  <span className="ml-2 text-gray-600">
                    {serverInfo.auth_required ? 'Required' : 'Not Required'}
                  </span>
                </div>
              </div>
            </div>
          )}
        </div>
      </main>
    </div>
  );
}