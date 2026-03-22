import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // The below is required for static export.
  // See https://nextjs.org/docs/app/guides/static-exports.
  output: "export",
  images: {
    remotePatterns: [
      {
        protocol: "https",
        hostname: "**",
      },
    ],
  },
};

export default nextConfig;
