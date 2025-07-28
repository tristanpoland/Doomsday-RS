'use client';

import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { auth, api } from '@/lib/api';
import { Shield, LogOut, RefreshCw, User, Settings } from 'lucide-react';

interface HeaderProps {
  onRefresh: () => void;
  refreshing: boolean;
  authRequired: boolean;
}

export function Header({ onRefresh, refreshing, authRequired }: HeaderProps) {
  const [showLogin, setShowLogin] = useState(false);
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loggingIn, setLoggingIn] = useState(false);
  const [error, setError] = useState('');
  
  const isAuthenticated = auth.isAuthenticated();

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoggingIn(true);
    setError('');

    try {
      const response = await api.authenticate({ username, password });
      auth.setToken(response.token, response.expires_at);
      setShowLogin(false);
      setUsername('');
      setPassword('');
      window.location.reload();
    } catch (err) {
      setError('Invalid credentials');
    } finally {
      setLoggingIn(false);
    }
  };

  const handleLogout = () => {
    auth.clearToken();
    window.location.reload();
  };

  return (
    <header className="bg-white shadow-sm border-b">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex justify-between items-center h-16">
          <div className="flex items-center">
            <Shield className="h-8 w-8 text-blue-600" />
            <h1 className="ml-2 text-xl font-bold text-gray-900">
              Doomsday Certificate Monitor
            </h1>
          </div>

          <div className="flex items-center space-x-4">
            <Button
              onClick={onRefresh}
              disabled={refreshing}
              variant="outline"
              size="sm"
              className="flex items-center gap-2"
            >
              <RefreshCw className={`h-4 w-4 ${refreshing ? 'animate-spin' : ''}`} />
              Refresh
            </Button>

            {authRequired && (
              <div className="flex items-center space-x-2">
                {isAuthenticated ? (
                  <div className="flex items-center space-x-2">
                    <div className="flex items-center space-x-1 text-sm text-gray-600">
                      <User className="h-4 w-4" />
                      <span>Authenticated</span>
                    </div>
                    <Button
                      onClick={handleLogout}
                      variant="outline"
                      size="sm"
                      className="flex items-center gap-2"
                    >
                      <LogOut className="h-4 w-4" />
                      Logout
                    </Button>
                  </div>
                ) : (
                  <Button
                    onClick={() => setShowLogin(!showLogin)}
                    variant="outline"
                    size="sm"
                    className="flex items-center gap-2"
                  >
                    <User className="h-4 w-4" />
                    Login
                  </Button>
                )}
              </div>
            )}
          </div>
        </div>

        {/* Login Form */}
        {showLogin && !isAuthenticated && (
          <div className="border-t bg-gray-50 p-4">
            <form onSubmit={handleLogin} className="max-w-md mx-auto space-y-4">
              <div>
                <Input
                  type="text"
                  placeholder="Username"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  required
                />
              </div>
              <div>
                <Input
                  type="password"
                  placeholder="Password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  required
                />
              </div>
              {error && (
                <div className="text-red-600 text-sm text-center">{error}</div>
              )}
              <div className="flex space-x-2">
                <Button
                  type="submit"
                  disabled={loggingIn}
                  className="flex-1"
                >
                  {loggingIn ? 'Logging in...' : 'Login'}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={() => setShowLogin(false)}
                >
                  Cancel
                </Button>
              </div>
            </form>
          </div>
        )}
      </div>
    </header>
  );
}