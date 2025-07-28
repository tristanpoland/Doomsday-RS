import { 
  CacheItem, 
  InfoResponse, 
  AuthRequest, 
  AuthResponse, 
  SchedulerInfo, 
  RefreshRequest, 
  PopulateStats 
} from '@/types';

const API_BASE = process.env.NEXT_PUBLIC_API_URL || '/api';

class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public statusText: string
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

async function apiRequest<T>(
  endpoint: string,
  options: RequestInit = {}
): Promise<T> {
  const token = typeof window !== 'undefined' 
    ? localStorage.getItem('doomsday-token') 
    : null;

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string>),
  };

  if (token) {
    headers['X-Doomsday-Token'] = token;
  }

  const response = await fetch(`${API_BASE}${endpoint}`, {
    ...options,
    headers,
  });

  if (!response.ok) {
    throw new ApiError(
      `API request failed: ${response.statusText}`,
      response.status,
      response.statusText
    );
  }

  return response.json();
}

export const api = {
  // Info endpoint
  getInfo: (): Promise<InfoResponse> =>
    apiRequest('/info'),

  // Authentication
  authenticate: (credentials: AuthRequest): Promise<AuthResponse> =>
    apiRequest('/auth', {
      method: 'POST',
      body: JSON.stringify(credentials),
    }),

  // Cache endpoints
  getCertificates: (params?: {
    beyond?: string;
    within?: string;
  }): Promise<CacheItem[]> => {
    const searchParams = new URLSearchParams();
    if (params?.beyond) searchParams.set('beyond', params.beyond);
    if (params?.within) searchParams.set('within', params.within);
    
    const query = searchParams.toString();
    return apiRequest(`/cache${query ? `?${query}` : ''}`);
  },

  refreshCache: (request?: RefreshRequest): Promise<PopulateStats> =>
    apiRequest('/cache/refresh', {
      method: 'POST',
      body: JSON.stringify(request || {}),
    }),

  // Scheduler endpoint
  getSchedulerInfo: (): Promise<SchedulerInfo> =>
    apiRequest('/scheduler'),
};

export { ApiError };

// Auth utilities
export const auth = {
  setToken: (token: string, expiresAt: string) => {
    if (typeof window !== 'undefined') {
      localStorage.setItem('doomsday-token', token);
      localStorage.setItem('doomsday-token-expires', expiresAt);
    }
  },

  getToken: (): string | null => {
    if (typeof window !== 'undefined') {
      return localStorage.getItem('doomsday-token');
    }
    return null;
  },

  clearToken: () => {
    if (typeof window !== 'undefined') {
      localStorage.removeItem('doomsday-token');
      localStorage.removeItem('doomsday-token-expires');
    }
  },

  isTokenExpired: (): boolean => {
    if (typeof window !== 'undefined') {
      const expiresAt = localStorage.getItem('doomsday-token-expires');
      if (!expiresAt) return true;
      
      return new Date(expiresAt) <= new Date();
    }
    return true;
  },

  isAuthenticated: (): boolean => {
    return !!auth.getToken() && !auth.isTokenExpired();
  },
};