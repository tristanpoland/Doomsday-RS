import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { CacheStats } from '@/types';
import { Shield, AlertTriangle, Clock, CheckCircle } from 'lucide-react';

interface StatsCardsProps {
  stats: CacheStats;
  loading?: boolean;
}

export function StatsCards({ stats, loading = false }: StatsCardsProps) {
  if (loading) {
    return (
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
        {[...Array(4)].map((_, i) => (
          <Card key={i} className="animate-pulse">
            <CardHeader className="pb-2">
              <div className="h-4 bg-gray-200 rounded w-3/4"></div>
            </CardHeader>
            <CardContent>
              <div className="h-8 bg-gray-200 rounded w-1/2"></div>
            </CardContent>
          </Card>
        ))}
      </div>
    );
  }

  const cards = [
    {
      title: 'Total Certificates',
      value: stats.total,
      icon: Shield,
      color: 'text-blue-600',
      bgColor: 'bg-blue-50',
    },
    {
      title: 'Expired',
      value: stats.expired,
      icon: AlertTriangle,
      color: 'text-red-600',
      bgColor: 'bg-red-50',
    },
    {
      title: 'Expiring Soon',
      value: stats.expiring_soon,
      icon: Clock,
      color: 'text-orange-600',
      bgColor: 'bg-orange-50',
    },
    {
      title: 'OK',
      value: stats.ok,
      icon: CheckCircle,
      color: 'text-green-600',
      bgColor: 'bg-green-50',
    },
  ];

  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
      {cards.map((card) => {
        const Icon = card.icon;
        return (
          <Card key={card.title}>
            <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
              <CardTitle className="text-sm font-medium text-gray-600">
                {card.title}
              </CardTitle>
              <div className={`p-2 rounded-lg ${card.bgColor}`}>
                <Icon className={`h-4 w-4 ${card.color}`} />
              </div>
            </CardHeader>
            <CardContent>
              <div className="text-2xl font-bold">{card.value.toLocaleString()}</div>
              <div className="text-xs text-gray-500 mt-1">
                {stats.total > 0
                  ? `${Math.round((card.value / stats.total) * 100)}% of total`
                  : 'No data'}
              </div>
            </CardContent>
          </Card>
        );
      })}
    </div>
  );
}