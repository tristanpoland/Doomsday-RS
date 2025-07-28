/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  swcMinify: true,
  output: 'standalone',
  async rewrites() {
    return [
      {
        source: '/api/:path*',
        destination: process.env.BACKEND_URL || 'http://localhost:8111/v1/:path*',
      },
    ];
  },
  env: {
    BACKEND_URL: process.env.BACKEND_URL || 'http://localhost:8111',
  },
};

module.exports = nextConfig;